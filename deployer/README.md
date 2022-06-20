# Deployment System

Service that manages the building, loading, and deployment of the Shuttle service(s) that make up a user's Shuttle project.

## Checklist

* [ ] Implement building and loading of incoming services.
* [ ] Implement the execution/deployment of built and loaded services.
* [x] Re-deploy already built services (read active deployments from database and add directly to the run queue).
* [ ] Populate logs table.
* [ ] Send build logs over WebSockets.
* [ ] Cache `crates.io`.
* [ ] Server-side pre-deployment testing of services.
* [ ] End-to-end/integration testing.
* [ ] Unit testing.
