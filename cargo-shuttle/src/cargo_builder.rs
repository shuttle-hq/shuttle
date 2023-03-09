use anyhow::Result;
use std::array;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};
use toml_edit::{value, Array, Document, Item, Table, Value};

// todo - See if we can accept a str instead of a Sring throughout.
// todo - See if we can use Path instead of PathBuff
// todo - fix version call
// todo - Add package settings to combine method
// todo - we should be able to add vec! directly instead of having to pass in `Array::from_iter(vec!["dsfdsf"]);`
// todo - make the combine functional on all types and not just the dependencies field

#[derive(Debug, Default)]
pub struct CargoBuilder {
    packages: HashMap<String, String>,
    dependencies: HashMap<String, HashMap<String, Value>>,
}

pub struct Dependency {
    name: String,
    version: Option<String>,
}

impl Dependency {
    pub fn new(package_name: String) -> Self {
        Dependency {
            name: package_name,
            version: None,
        }
    }

    pub fn get_latest_version(&self) -> String {
        match &self.version {
            Some(x) => x.to_owned(),
            None => "1.1.1.1".to_owned(),
        }
    }
}

impl CargoBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds an inline dependency attribute to a current dependency. Creates a new dependency line
    /// if it doesn't already exist
    pub fn add_dependency_var(
        &mut self,
        dependency: Dependency,
        attribute_name: String,
        dep_value: Value,
    ) -> &mut Self {
        match self.dependencies.get_mut(&dependency.name) {
            Some(x) => {
                x.entry(attribute_name).or_insert(dep_value);
            }
            None => {
                self.dependencies.insert(
                    dependency.name,
                    HashMap::from([(attribute_name, dep_value)]),
                );
            }
        }
        self
    }

    /// Add a main dependency and calculates the current version. Convenience function for adding
    /// and calculating the version attributes of a dependency
    pub fn add_dependency(&mut self, dependency: Dependency) -> &mut Self {
        let version = dependency.get_latest_version();
        self.add_dependency_var(dependency, "version".to_owned(), Value::from(version))
    }

    /// Saves the `CargoBuilder` values to the `path` provided, overwriting any existing matching
    /// values
    pub fn save_overwrite(self, path: PathBuf) -> Result<()> {
        let mut cargo_doc = read_to_string(path)?.parse::<Document>()?;
        let toml_document = self.combine(cargo_doc)?;
        Ok(())
    }

    /// Returns the toml_edit `Document` of the current settings
    pub fn get_document(self) -> Document {
        let blank_doc = Document::new();
        self.combine(blank_doc).unwrap()
    }

    /// Combines both provided toml `path` file with the settings of the `CargoBuilder` struct.
    /// Duplicate settings will be overwritten by the `CargoBuilder` settings
    pub fn combine(self, mut cargo_doc: Document) -> Result<Document> {
        // Loop over main dependency name - `axum`,`actix` etc.
        for (name, dep_attribute) in self.dependencies {
            // Loop over child values 'version' / 'features' etc.
            if dep_attribute.len() == 1 && dep_attribute.contains_key("version") {
                let dep_value = dep_attribute.get("version").unwrap();
                cargo_doc["dependencies"][name.to_owned()] = value(dep_value);
            } else {
                for (dep_type, dep_value) in dep_attribute {
                    cargo_doc["dependencies"][name.to_owned()][dep_type] = value(dep_value);
                }
            }
        }

        Ok(cargo_doc)
    }
}

#[cfg(test)]
mod cargo_builder_tests {
    use super::*;

    fn get_mock_dependency(name: &str, version: Option<String>) -> Dependency {
        Dependency {
            name: name.to_owned(),
            version,
        }
    }

    // Adding a new dependency
    #[test]
    fn test_add_dependency_new() {
        let dependency = get_mock_dependency("test-dep", Some("1.2.3".to_owned()));

        let mut builder = CargoBuilder::new();
        builder.add_dependency(dependency);
        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = \"1.2.3\" }\n"
        );
    }

    // Adding one dependency of the same name over another
    #[test]
    fn test_add_dependency_additional() {
        let dependency1 = get_mock_dependency("test-dep", Some("1.1".to_owned()));
        let dependency2 = get_mock_dependency("test-dep", Some("1.2".to_owned()));

        let mut builder = CargoBuilder::new();
        builder.add_dependency(dependency1);
        builder.add_dependency(dependency2);
        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = \"1.2\" }\n"
        );
    }

    #[test]
    fn test_add_dependency_var_new() {
        let dependency = get_mock_dependency("test-dep", Some("1.1".to_owned()));

        let mut builder = CargoBuilder::new();
        let features = Array::from_iter(vec!["axum-web"]);
        builder.add_dependency_var(dependency, "features".to_owned(), features);
        let toml_document = builder.get_document();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = { features = [\"axum-web\"] } }\n"
        );
    }

    #[test]
    fn test_add_dependency_var_additional() {
        let existing_toml_doc = Document::new();
        let dependency1 = get_mock_dependency("test-dep", Some("1.1".to_owned()));
        let dependency2 = get_mock_dependency("test-dep", Some("1.2".to_owned()));

        let mut builder = CargoBuilder::new();
        let features = Array::from_iter(vec!["dsfdsf"]);
        builder.add_dependency_var(dependency1, "path".to_owned(), "initial/path");
        builder.add_dependency_var(dependency2, "path".to_owned(), "overwrite/path");
        let toml_document = builder.combine(existing_toml_doc).unwrap();

        assert_eq!(
            toml_document.to_string(),
            "dependencies = { test-dep = { path = \"overwrite/path\" } }\n"
        );
    }

    #[test]
    fn test_combine_overwrite() {
        let mut doc_to_overwrite = Document::new();
        doc_to_overwrite["dependencies"]["test_dep"]["features"] = value("initial value");

        let mock_dependencies = HashMap::from([(
            "test_dep".to_owned(),
            HashMap::from([("features".to_owned(), value("overwrite value"))]),
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
}
