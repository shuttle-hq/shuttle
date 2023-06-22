use serde_json::json;
use tracing::field::Visit;

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
    /// Get log fields from the `log` crate
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
