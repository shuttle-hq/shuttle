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
}

impl From<(&String, &toml::Value)> for User {
    fn from((key, value): (&String, &toml::Value)) -> User {
        let name = match value {
            toml::Value::Table(table) => table
                .get("name")
                .expect("user to have a name")
                .as_str()
                .expect("name to be a string")
                .to_string(),
            other => panic!("unexpected '{other}' at user level"),
        };

        Self {
            key: key.to_string(),
            name,
        }
    }
}

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "INSERT INTO accounts(account_name, key, super_user) VALUES('{}', '{}', 0);",
            self.name, self.key
        )
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
            },
            User {
                key: "key2".to_string(),
                name: "name2".to_string(),
            },
        ];

        assert_eq!(actual, expected);
    }
}
