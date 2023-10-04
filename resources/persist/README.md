# Shuttle Persist

This plugin allows persisting struct that implement `serde::Serialize` and loading them again using `serde::Deserialize`.

## Usage

Add `shuttle-persist` to the dependencies for your service. You can get this resource using the `shuttle-persist::Persist` attribute to get a `PersistInstance`. Objects can now be managed with the following six methods:

- `clear()`: removes the keys within the `PersistInstance`
- `list()`: returns a vector of strings containing all the keys associated with a `PersistInstance`
- `load()`: loads the contents of the `PersistInstance`
- `new()`: constructs a new `PersistInstance` along with its associated storage folder
- `save()`: saves a key-value pair into the `PersistInstance`
- `remove()`: deletes a key from the `PersistInstance`

An example using the Rocket framework can be found on [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/rocket/persist)
