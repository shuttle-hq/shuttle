use rand::distributions::{Alphanumeric, DistString};
use std::{
    env::args,
    fmt::{Display, Formatter},
    fs,
};

fn main() {
    let mut args = args();
    let _ = args.next();
    let file = args
        .next()
        .expect("expected a users.toml file to convert to .sql");
    let data = fs::read_to_string(file).expect("to read data file");
    let toml = toml::from_str(&data).expect("to parse data file");

    let users = parse_value(toml);

    for user in users {
        println!("{user}");
    }
}

#[derive(Eq, PartialEq, Debug)]
struct User {
    key: String,
    name: String,
    projects: Vec<String>,
}

impl From<(&String, &toml::Value)> for User {
    fn from((key, value): (&String, &toml::Value)) -> User {
        let (name, projects) = match value {
            toml::Value::Table(table) => {
                let name = table
                    .get("name")
                    .expect("user to have a name")
                    .as_str()
                    .expect("name to be a string")
                    .to_string();
                let projects = table
                    .get("projects")
                    .expect("user to have projects")
                    .as_array()
                    .expect("projects to be an array")
                    .iter()
                    .map(|value| value.as_str().expect("project to be a string").to_string())
                    .collect();

                (name, projects)
            }
            other => panic!("unexpected '{other}' at user level"),
        };

        Self {
            key: key.to_string(),
            name,
            projects,
        }
    }
}

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "INSERT INTO accounts(account_name, key, super_user) VALUES('{}', '{}', 0);",
            self.name, self.key
        )?;

        for project in self.projects.iter() {
            let initial_key = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
            let state = format!("{{\"creating\": {{\"project_name\": \"{project}\", \"initial_key\": \"{initial_key}\"}}}}");

            write!(
                f,
                "\nINSERT INTO projects(project_name, account_name, project_state, initial_key) VALUES('{}', '{}', '{}', '{}');",
                project,
                self.name,
                state,
                initial_key,
            )?;
        }

        write!(f, "\n")
    }
}

fn parse_value(value: toml::Value) -> Vec<User> {
    match value {
        toml::Value::Table(table) => table.iter().map(Into::into).collect(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::User;

    fn get_dummy() -> toml::Value {
        r#"
[key1]
name = 'name1'
projects = []

[key2]
name = 'name2'
projects = [
    'project1',
    'project2',
]
"#
        .parse()
        .unwrap()
    }

    #[test]
    fn parse_value() {
        let value = get_dummy();
        let actual = super::parse_value(value);

        let expected = vec![
            User {
                key: "key1".to_string(),
                name: "name1".to_string(),
                projects: vec![],
            },
            User {
                key: "key2".to_string(),
                name: "name2".to_string(),
                projects: vec!["project1".to_string(), "project2".to_string()],
            },
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn display() {
        let input = User {
            key: "key".to_string(),
            name: "name".to_string(),
            projects: vec!["project1".to_string(), "project2".to_string()],
        };

        let actual = input.to_string();

        assert!(
            actual.starts_with(
                "INSERT INTO accounts(account_name, key, super_user) VALUES('name', 'key', 0);"
            ),
            "got: {}",
            actual
        );
        assert!(
            actual.contains(
                "INSERT INTO projects(project_name, account_name, project_state, initial_key) VALUES('project1', 'name', '{\"creating\": {\"project_name\": \"project1\", \"initial_key\": "
            ),
            "got: {}",
            actual
        );
        assert!(
            actual.contains(
                "INSERT INTO projects(project_name, account_name, project_state, initial_key) VALUES('project2', 'name', '{\"creating\": {\"project_name\": \"project2\", \"initial_key\": "
            ),
            "got: {}",
            actual
        );
    }
}
