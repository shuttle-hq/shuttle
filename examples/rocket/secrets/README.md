## How to use
The secrets resource requires a `Secrets.toml` file to be present in your crate. Each like in this file
should be a key-value pair that you can access using `SecretStore::get(&self, key)`.

Rename `Secrets.toml.example` to `Secrets.toml` to use this example.
