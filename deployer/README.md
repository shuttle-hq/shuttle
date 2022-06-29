# Deployment System

Service that manages the building, loading, and deployment of the Shuttle service(s) that make up a user's Shuttle project.

## Endpoints

* `GET /services` - Returns an array containing information objects for each service managed by the deployer.
  * If there are no services: `[]`
  * If there are some services: `[{"name": "service-one", ...}, {"name": "service-two", ...}, ...]`
* `GET /services/:name` - Returns information on the specified service.
  * If a service with the given name does not exist: `null`
  * If a service with that name exists: `{"name": "some-service", "state": "building"}`
  * See `src/deployment/info.rs` for up-to-date look at what the returned JSON object will look like.
* `POST /services/:name` (where the body is `.tar.gz`-encoded Cargo crate data) - Add a new service to the build, load, deploy pipeline.
  * A lack of or invalid data in the request body will of course result in the build process failing.
* `DELETE /services/:name` - Stops the execution of the specified service and removes it from the list of services.
* `GET /services/:name/build-logs` - Get all Cargo build logs produced so far for the specified deployment.
  * If there is no service with the specified name or it has not yet been built: `null`
  * If the specified service has produced build logs: `["log line 1", "log line 2", ...]`
* `GET /services/:name/build-logs-subscribe` - Establish a WebSocket connection over which Cargo build logs are streamed as they are produced. Build log lines produced before the request is sent and a connection established are not sent over.
  * Clients are not expected to send any data - they should continually receive and will be sent each log line (a string without an ending newline character) as individual message.
