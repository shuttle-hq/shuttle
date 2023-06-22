use chrono::Utc;
use opentelemetry_proto::tonic::logs::v1::{LogRecord, SeverityNumber};
use serde_json::json;
use tracing::{field::Visit, Level, Metadata};

pub const MESSAGE_KEY: &str = "message";
pub const FILEPATH_KEY: &str = "code.filepath";
pub const LINENO_KEY: &str = "code.lineno";
pub const NAMESPACE_KEY: &str = "code.namespace";
pub const TARGET_KEY: &str = "target";

// Boilerplate for extracting the fields from the event
#[derive(Default)]
pub struct JsonVisitor {
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub target: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl JsonVisitor {
    /// Ignores log metadata as it is included in the other LogItem fields (target, file, line...)
    fn filter_insert(&mut self, field: &tracing::field::Field, value: serde_json::Value) {
        match field.name() {
            "log.line" => self.line = value.as_u64().map(|u| u as u32),
            "log.target" => self.target = value.as_str().map(ToOwned::to_owned),
            "log.file" => self.file = value.as_str().map(ToOwned::to_owned),
            "log.module_path" => {}
            name => {
                self.fields.insert(name.to_string(), json!(value));
            }
        }
    }

    /// Use metadata to turn self into a [LogRecord]
    pub fn into_log_record(mut self, metadata: &Metadata) -> LogRecord {
        let body = self.get_body();
        let severity_number = get_severity_number(metadata);
        let attributes = self.enrich_with_metadata(metadata);
        let attributes = serde_json_map_to_key_value_list(attributes);

        LogRecord {
            time_unix_nano: Utc::now().timestamp_nanos() as u64,
            severity_number: severity_number.into(),
            severity_text: metadata.level().to_string(),
            body: serde_json_value_to_any_value(body),
            attributes,
            dropped_attributes_count: 0,
            ..Default::default()
        }
    }

    /// Get the body from a visitor
    fn get_body(&mut self) -> serde_json::Value {
        self.fields.remove(MESSAGE_KEY).unwrap_or_default()
    }

    /// Add metadata information to own fields and return those fields
    fn enrich_with_metadata(
        self,
        metadata: &Metadata,
    ) -> serde_json::Map<String, serde_json::Value> {
        let Self {
            mut fields,
            target,
            file,
            line,
        } = self;

        fields.insert(
            TARGET_KEY.to_string(),
            serde_json::Value::String(target.unwrap_or(metadata.target().to_string())),
        );

        if let Some(filepath) = file.or(metadata.file().map(ToString::to_string)) {
            fields.insert(
                FILEPATH_KEY.to_string(),
                serde_json::Value::String(filepath),
            );
        }

        if let Some(lineno) = line.or(metadata.line()) {
            fields.insert(
                LINENO_KEY.to_string(),
                serde_json::Value::Number(lineno.into()),
            );
        }

        if let Some(namespace) = metadata.module_path() {
            fields.insert(
                NAMESPACE_KEY.to_string(),
                serde_json::Value::String(namespace.to_string()),
            );
        }

        fields
    }
}

impl Visit for JsonVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.filter_insert(field, json!(value));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.filter_insert(field, json!(value));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.filter_insert(field, json!(value));
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.filter_insert(field, json!(value));
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.filter_insert(field, json!(value));
    }
    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.filter_insert(field, json!(value.to_string()));
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.filter_insert(field, json!(format!("{value:?}")));
    }
}

fn get_severity_number(metadata: &Metadata) -> SeverityNumber {
    match *metadata.level() {
        Level::TRACE => SeverityNumber::Trace,
        Level::DEBUG => SeverityNumber::Debug,
        Level::INFO => SeverityNumber::Info,
        Level::WARN => SeverityNumber::Warn,
        Level::ERROR => SeverityNumber::Error,
    }
}

fn serde_json_value_to_any_value(
    value: serde_json::Value,
) -> Option<opentelemetry_proto::tonic::common::v1::AnyValue> {
    use opentelemetry_proto::tonic::common::v1::any_value::Value;

    let value = match value {
        serde_json::Value::Null => return None,
        serde_json::Value::Bool(b) => Value::BoolValue(b),
        serde_json::Value::Number(n) => {
            if n.is_f64() {
                Value::DoubleValue(n.as_f64().unwrap()) // Safe to unwrap as we just checked if it is a f64
            } else {
                Value::IntValue(n.as_i64().unwrap()) // Safe to unwrap since we know it is not a f64
            }
        }
        serde_json::Value::String(s) => Value::StringValue(s),
        serde_json::Value::Array(a) => {
            Value::ArrayValue(opentelemetry_proto::tonic::common::v1::ArrayValue {
                values: a
                    .into_iter()
                    .flat_map(serde_json_value_to_any_value)
                    .collect(),
            })
        }
        serde_json::Value::Object(o) => {
            Value::KvlistValue(opentelemetry_proto::tonic::common::v1::KeyValueList {
                values: serde_json_map_to_key_value_list(o),
            })
        }
    };

    Some(opentelemetry_proto::tonic::common::v1::AnyValue { value: Some(value) })
}

pub fn serde_json_map_to_key_value_list(
    map: serde_json::Map<String, serde_json::Value>,
) -> Vec<opentelemetry_proto::tonic::common::v1::KeyValue> {
    map.into_iter()
        .map(
            |(key, value)| opentelemetry_proto::tonic::common::v1::KeyValue {
                key,
                value: serde_json_value_to_any_value(value),
            },
        )
        .collect()
}
