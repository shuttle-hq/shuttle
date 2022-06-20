# Deployment System

Service that manages the building, loading, and deployment of the Shuttle service(s) that make up a user's Shuttle project.

## Checklist

* [ ] Implement building of incoming services.
  * `deployment/queue.rs`
* [ ] Implement the loading/execution/deployment of built services.
  * `deployment/run.rs`
* [ ] Populate logs table.
  * `persistence.rs`
  * `deployment/queue.rs`
  * `deployment/run.rs`
* [ ] Send build logs over WebSockets.
* [ ] Properly cache `crates.io`.
* [ ] Server-side pre-deployment testing of services.
  * `deployment/run.rs`
* [ ] Integrate with gateway (i.e., start instances of deployer from gateway with correct project secret specified).
  * Depends on gateway being complete/merged.
* [ ] End-to-end/integration testing.
  * Fairly independent of the rest of the code base.
