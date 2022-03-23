# Examples

Some example apps to show what you can do with shuttle.

## How to deploy the examples

To deploy the examples:

1. Check out the repository locally

```bash
git clone https://github.com/getsynth/shuttle.git
cd shuttle
```

2. Install `cargo-shuttle` with either
```bash
cargo install cargo-shuttle
```
or
```bash
cargo install --path cargo-shuttle
```

3. Navigate to an example root folder

```bash
cd examples/rocket/hello-world
```

4. Open up the `Shuttle.toml` file and change the project name to something 
unique - in shuttle, projects are globally unique. Then run

5. Get your API key by running
```bash
cargo shuttle login
```
That will open a browser window and prompt you to connect using your GitHub account.
Copy-paste the key into the terminal and **store it in a safe place for future use** as a CLI param (`--api_key [your key]`).

6. Deploy the service with:

```bash
cargo shuttle deploy --allow-dirty
```