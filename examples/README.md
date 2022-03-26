# Examples

Some example apps to show what you can do with shuttle.

## How to deploy the examples

To deploy the examples, check out the repository locally

```bash
git clone https://github.com/getsynth/shuttle.git
```

navigate to an example root folder

```bash
cd shuttle/examples/rocket/hello-world
```

open up the `Shuttle.toml` file and change the project name to something 
unique - in shuttle, projects are globally unique. Then run

```bash
cargo shuttle deploy
```