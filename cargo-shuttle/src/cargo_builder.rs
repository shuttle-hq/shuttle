use anyhow::Result;
use crates_index::Index;
use std::collections::BTreeMap;
use toml_edit::{value, Document, Value};

pub enum CargoSection {
    Dependency(Dependency),
    Package,
}

#[derive(Debug, Default)]
pub struct CargoBuilder {
    packages: BTreeMap<String, Value>,
    dependencies: BTreeMap<String, BTreeMap<String, Value>>,
}

#[derive(Clone)]
pub struct Dependency {
    name: String,
    version: Option<String>,
}

impl Dependency {
    pub fn new(name: String, version: Option<String>) -> Self {
        Dependency { name, version }
    }

    pub fn get_version(&self) -> &Option<String> {
        &self.version
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_latest_version(&self) -> String {
        match &self.version {
            Some(x) => x.to_owned(),
            None => {
                let index = Index::new_cargo_default().unwrap();
                let crate_ver = index
                    .crate_(&self.name)
                    .expect(&format!("Could not find package {} in registry", self.name));

                crate_ver
                    .highest_normal_version()
                    .unwrap()
                    .version()
                    .to_string()
            }
        }
    }
}

impl CargoBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds an inline value attribute representing a `CargoSection` to be built via `combine` or
    /// `get_document` methods, duplicate values are overwritten. Any `CargoSection::Dependency()`
    /// values without a version will be calculated automatically
    pub fn add_var(
        &mut self,
        section: CargoSection,
        attribute_name: String,
        dep_value: Value,
    ) -> &mut Self {
        match section {
            CargoSection::Dependency(x) => {
                match self.dependencies.get_mut(&x.name) {
                    Some(y) => {
                        y.insert(attribute_name, dep_value);
                    }
                    None => {
                        self.dependencies.insert(
                            x.name.to_owned(),
                            BTreeMap::from([(attribute_name, dep_value)]),
                        );
                    }
                };

                self.dependencies
                    .get_mut(&x.name)
                    .unwrap()
                    .insert("version".to_owned(), Value::from(x.get_latest_version()));
            }
            CargoSection::Package => {
                self.packages.entry(attribute_name).or_insert(dep_value);
            }
        }
        self
    }

    // Convenience function for inserting dependency with `String` only without the use of `CargoSection`
    pub fn add_dependency_var(
        &mut self,
        dependency: Dependency,
        attribute_name: String,
        dep_value: Value,
    ) -> &mut Self {
        self.add_var(
            CargoSection::Dependency(dependency),
            attribute_name,
            dep_value,
        )
    }

    // Convenience function for inserting a package with name / values without the need for `CargoSection`
    pub fn add_package(&mut self, package_name: &str, package_value: &str) -> &mut Self {
        self.add_var(
            CargoSection::Package,
            package_name.to_owned(),
            Value::from(package_value),
        )
    }

    /// Add a main dependency and calculates the current version. This is a convenience function for adding
    /// and calculating the version attributes of a dependency
    pub fn add_dependency(&mut self, dependency: Dependency) -> &mut Self {
        let version = dependency.get_latest_version();
        self.add_var(
            CargoSection::Dependency(dependency),
            "version".to_owned(),
            Value::from(version),
        )
    }

    /// Returns the toml_edit `Document` for the current settings
    pub fn get_document(self) -> Document {
        let blank_doc = Document::new();
        self.combine(blank_doc).unwrap()
    }

    /// Combines both provided toml `Document` with the settings of the `CargoBuilder` struct.
    /// Duplicate values will be overwritten by the `CargoBuilder` settings
    pub fn combine(self, mut cargo_doc: Document) -> Result<Document> {
        for (name, dep_attribute) in self.dependencies {
            if dep_attribute.len() == 1 && dep_attribute.contains_key("version") {
                let dep_value = dep_attribute.get("version").unwrap();
                cargo_doc["dependencies"][name.to_owned()] = value(dep_value);
            } else {
                for (dep_type, dep_value) in dep_attribute {
                    cargo_doc["dependencies"][name.to_owned()][dep_type] = value(dep_value);
                }
            }
        }

        for (name, dep_value) in self.packages {
            cargo_doc["packages"][name.to_owned()] = value(dep_value);
        }

        Ok(cargo_doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml_edit::Array;

    fn get_mock_dependency(name: &str, version: Option<String>) -> Dependency {
        Dependency {
            name: name.to_owned(),
            version,
        }
    }

    #[test]
    fn add_dependency_new() {
        let dependency = get_mock_dependency("test-dep", Some("1.2.3".to_owned()));

        let mut builder = CargoBuilder::new();
        builder.add_dependency(dependency);
        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = \"1.2.3\" }\n"
        );
    }

    #[test]
    fn add_dependency_overwrite() {
        let dependency1 = get_mock_dependency("test-dep", Some("1.1.1".to_owned()));
        let dependency2 = get_mock_dependency("test-dep", Some("1.2.2".to_owned()));

        let mut builder = CargoBuilder::new();
        builder.add_dependency(dependency1);
        builder.add_dependency(dependency2);
        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = \"1.2.2\" }\n"
        );
    }

    #[test]
    fn add_dependency_additional() {
        let dependency1 = get_mock_dependency("test-dep", Some("1.1.1".to_owned()));

        let mut builder = CargoBuilder::new();
        let features = Array::from_iter(vec!["axum-web"]);

        builder.add_dependency(dependency1.to_owned());
        builder.add_var(
            CargoSection::Dependency(dependency1.to_owned()),
            "features".to_owned(),
            Value::from(features),
        );

        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = { features = [\"axum-web\"], version = \"1.1.1\" } }\n"
        );
    }

    #[test]
    fn add_dependency_var_no_version() {
        let dependency = get_mock_dependency("test-dep", Some("1.1.1".to_owned()));
        let mut builder = CargoBuilder::new();

        builder.add_dependency_var(
            dependency.to_owned(),
            "features".to_owned(),
            Value::from("afeature"),
        );

        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = { features = \"afeature\", version = \"1.1.1\" } }\n"
        );
    }

    #[test]
    fn add_var_new() {
        let dependency = get_mock_dependency("test-dep", Some("1.1.1".to_owned()));
        let mut builder = CargoBuilder::new();
        let features = Array::from_iter(vec!["axum-web"]);
        builder.add_var(
            CargoSection::Dependency(dependency),
            "features".to_owned(),
            Value::from(features),
        );
        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = { features = [\"axum-web\"], version = \"1.1.1\" } }\n"
        );
    }

    #[test]
    fn add_var_overwrite() {
        let existing_toml_doc = Document::new();
        let dependency = get_mock_dependency("test-dep", Some("1.1.1".to_owned()));
        let overwrite_dependency = get_mock_dependency("test-dep", Some("1.2.2".to_owned()));

        let mut builder = CargoBuilder::new();
        let features = Array::from_iter(vec!["dsfdsf"]);
        builder.add_var(
            CargoSection::Dependency(dependency),
            "path".to_owned(),
            Value::from("initial/path"),
        );
        builder.add_var(
            CargoSection::Dependency(overwrite_dependency),
            "path".to_owned(),
            Value::from("overwrite/path"),
        );
        let toml_document = builder.combine(existing_toml_doc).unwrap();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = { path = \"overwrite/path\", version = \"1.2.2\" } }\n"
        );
    }

    #[test]
    fn combine_overwrite() {
        let mut doc_to_overwrite = Document::new();
        doc_to_overwrite["dependencies"]["test_dep"]["features"] = value("initial value");

        let mock_dependencies = BTreeMap::from([(
            "test_dep".to_owned(),
            BTreeMap::from([("features".to_owned(), Value::from("overwrite value"))]),
        )]);

        let builder = CargoBuilder {
            dependencies: mock_dependencies,
            ..Default::default()
        };

        let toml_document = builder.combine(doc_to_overwrite).unwrap();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test_dep = { features = \"overwrite value\" } }\n"
        );
    }

    #[test]
    fn add_package_new() {
        let mut builder = CargoBuilder::new();
        builder.add_package("description", "test description");
        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "packages = { description = \"test description\" }\n"
        );
    }
}
