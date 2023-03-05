use anyhow::Result;
use std::array;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::{collections::HashMap, path::PathBuf};
use toml_edit::{value, Array, Document, Item, Table, Value};

#[derive(Debug, Default)]
pub struct CargoBuilder {
    path: PathBuf,
    packages: HashMap<String, String>,
    dependencies: HashMap<String, Vec<HashMap<String, Item>>>,
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

pub trait IntoTomlVal {}

impl CargoBuilder {
    pub fn new(path: PathBuf) -> Self {
        CargoBuilder {
            path,
            ..Default::default()
        }
    }

    /// Adds an inline dependency attribute to the current dependency ready for building via `self.build()`
    pub fn add_dependency_var<V: Into<Value>>(
        &mut self,
        dependency: Dependency,
        attribute_name: String,
        dep_value: V,
    ) -> &mut Self {
        let attribute = HashMap::from([(attribute_name, value(dep_value))]);
        match self.dependencies.get_mut(&dependency.name) {
            Some(x) => {
                println!("add to current!");
                x.push(attribute);
            }
            None => {
                println!("added new!");
                self.dependencies
                    .entry(dependency.name)
                    .or_insert(vec![attribute]);
            }
        }
        self
    }

    /// Adds a dependency with a calculate version to the current config, ready for
    /// building via `self.build()`
    pub fn add_dependency(&mut self, dependency: Dependency) -> &mut Self {
        eprintln!("Make this function generic over String or Vec<String>");

        let version = dependency.get_latest_version();

        self.add_dependency_var(dependency, "version".to_owned(), version)
    }

    pub fn build(self) -> Result<Document> {
        //panic!("enddd");
        println!("{:?}", self.path);

        let mut cargo_doc = read_to_string(self.path.clone())
            .unwrap()
            .parse::<Document>()
            .unwrap();

        // Loop over main dependency name
        for (name, dep_attribute) in self.dependencies {
            // Loop over child values vector of 'version' / 'features' etc.
            for dep in dep_attribute.into_iter() {
                //
                for (dep_type, dep_value) in dep {
                    cargo_doc["dependencies"][name.to_owned()][dep_type] = dep_value;
                }
            }
        }

        let mut cargo_toml = File::create(self.path).expect("oh I see");

        //let mut cargo_string = cargo_doc.to_string().as_bytes();
        cargo_toml
            .write_all(cargo_doc.to_string().as_bytes())
            .expect("oh no");

        //let mut cargo_toml = File::create(cargo_toml_path)?; println!("Result: {:#?}", self.dependencies);
        //println!("String: {:#?}", cargo_string);
        //panic!("the endd.");
        Ok(cargo_doc)
    }
}

#[cfg(test)]
mod cargo_builder_tests {
    use super::*;

    #[test]
    fn test_add_dependency() {
        let version = "1.2.3";
        let test_dep = "test-dep".to_owned();
        let manifest_path = PathBuf::new();
        let mut builder = CargoBuilder::new(manifest_path.clone());
        let dependency = Dependency {
            name: test_dep.clone(),
            version: Some(version.to_owned()),
        };

        builder.add_dependency(dependency);

        let expected_attribute = HashMap::from([("version".to_owned(), [version.to_owned()])]);
        let expected = CargoBuilder {
            path: manifest_path,
            packages: HashMap::new(),
            dependencies: HashMap::from([(test_dep, vec![expected_attribute])]),
        };

        assert_eq!(builder.dependencies, expected.dependencies);
    }

    #[test]
    fn test_add_dependency_var() {
        let version = "1.2.3";
        let test_dep = "test-dep".to_owned();
        let manifest_path = PathBuf::new();
        let mut builder = CargoBuilder::new(manifest_path.clone());
        let dependency = Dependency {
            name: test_dep.clone(),
            version: Some(version.to_owned()),
        };

        builder.add_dependency_var(dependency, "feature".to_owned(), []);

        let attribute = HashMap::from([("version".to_owned(), [version.to_owned()])]);
        let expected = CargoBuilder {
            path: manifest_path,
            packages: HashMap::new(),
            dependencies: HashMap::from([(test_dep, vec![attribute])]),
        };

        assert_eq!(builder.dependencies, expected.dependencies);
    }

    //#[test]
    //fn test_build() {
    ////assert_eq!(cargo_toml.to_string(), expected);
    //}
}
