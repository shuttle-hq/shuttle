# Persist Example

An example app to show what you can do with shuttle.

## How to deploy the example

To deploy the examples, check out the repository locally

```bash
$ git clone https://github.com/shuttle-hq/shuttle.git
```

navigate to the Persist root folder

```bash
$ cd examples/rocket/persist
```

Pick a project name that is something unique - in shuttle,
projects are globally unique. Then run

```bash
$ cargo shuttle project new --name=$PROJECT_NAME
$ cargo shuttle deploy --name=$PROJECT_NAME
```

Once deployed you can post to the endpoint the following values:
```bash
curl -X POST -H "Content-Type: application/json" -d '{"date":"2020-12-22", "temp_high":5, "temp_low":5, "precipitation": 5}' {$PROJECT_NAME}.shuttleapp.rs
```

The json data will then persist within Shuttle it can be queried with the following curl request

```bash
curl {$PROJECT_NAME}.shuttleapp.rs/2020-12-22
```
