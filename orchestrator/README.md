# Orchestrator example

This is a base crate that documents how to setup an orchestrator that will
provide the necessary APIs to start containers in a kubernetes cluster. Ideally
it will provide the necessary APIs to be used to manage shuttle-runtimes in a
Kubernetes cluster, but designing them is still under scoping.

## Setting up Kubernetes locally for development speed

There are various guides on internet on how to set this up. A straight-forward
alternative is to install Docker Desktop and `Enable Kubernetes` as described in
[this guide](https://docs.docker.com/desktop/kubernetes/) and make sure you set up
your kubectl to use the Docker Desktop K8s cluster. Other alternatives can be found
at [tilt.dev](https://docs.tilt.dev/choosing_clusters.html) for example.

## Creating pods against the cluster for experimentation

The `src/main.rs` file contains a sample code imported from [kube-rs/examples/pod-api](https://github.com/kube-rs/kube/blob/main/examples/pod_api.rs). To experiment based on the `orchestrator` example you should run the crate
as a binary with: `RUST_LOG=TRACE cargo run -p shuttle-orchestrator`.
