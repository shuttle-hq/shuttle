# Shuttle Static Folder
This plugin allows services to get the path to a static folder at runtime

## Usage
Add `shuttle-static-folder` to the dependencies for your service. This resource can be using by the `shuttle_static_folder::StaticFolder` attribute to get a `PathBuf` with the location of the static folder.

An example using the Axum framework can be found on [GitHub](https://github.com/shuttle-hq/examples/tree/main/axum/websocket)

``` rust
#[shuttle_service::main]
async fn main(
    #[shuttle_static_folder::StaticFolder] static_folder: PathBuf,
) -> __ { ... }
```

### Parameters
| Parameter | Type | Default  | Description                                                        |
|-----------|------|----------|--------------------------------------------------------------------|
| folder    | str  | `static` | The folder relative to the crate root to make a static folder for. |

### Example: Using the public folder instead
Since this plugin defaults to the `static` folder, the arguments can be used to use the `public` folder instead.

``` rust
#[shuttle_service::main]
async fn main(
    #[shuttle_static_folder::StaticFolder(folder = "public")] public_folder: PathBuf,
) -> __ { ... }
```
