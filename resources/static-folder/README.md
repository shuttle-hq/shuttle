# Shuttle Static Folder
This plugin allows services to get the path to a static folder at runtime

## Usage
Add `shuttle-static-folder` to the dependencies for your service. This resource can be using by the `shuttle_static_folder::StaticFolder` attribute to get a `PathBuf` with the location of the static folder.

An example using the Axum framework can be found on [GitHub](https://github.com/shuttle-hq/examples/tree/main/axum/websocket)
