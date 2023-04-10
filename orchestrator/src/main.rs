use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{DeleteParams, ListParams, Patch, PatchParams, PostParams},
    client::ConfigExt,
    runtime::wait::{await_condition, conditions::is_pod_running},
    Api, Client, Config, ResourceExt,
};
use serde_json::json;
use std::io;
use tracing::info;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> io::Result<()> {
    tracing_subscriber::fmt::init();

    // Starts a kube cluster client based on the local kubernetes configuration
    // (from ~/.kube/config or from KUBECONFIG env).
    let config = Config::infer().await.unwrap();

    // Uses HTTPs for the Kubernetes client.
    let https = config.rustls_https_connector().unwrap();
    let service = tower::ServiceBuilder::new()
        .layer(config.base_uri_layer())
        .option_layer(config.auth_layer().unwrap())
        .service(hyper::Client::builder().build(https));
    let client = Client::new(service, config.default_namespace);

    // Manage pods.
    let pods: Api<Pod> = Api::default_namespaced(client);

    // Create Pod blog spec. By default, the images are retrieved from DockerHub.
    // To specify a different registry we must use an image name as described here:
    // https://kubernetes.io/docs/concepts/containers/images/#image-names.
    info!("Creating Pod instance blog");
    let p: Pod = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "blog" },
        "spec": {
            "containers": [{
              "name": "blog",
              "image": "clux/blog:0.1.0"
            }],
        }
    }))?;

    // Create the pod.
    let pp = PostParams::default();
    match pods.create(&pp, &p).await {
        Ok(o) => {
            let name = o.name_any();
            assert_eq!(p.name_any(), name);
            info!("Created {}", name);
        }
        Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
        Err(_e) => panic!("error"),                            // any other case is probably bad
    }

    // Watch it phase for a few seconds.
    let establish = await_condition(pods.clone(), "blog", is_pod_running());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(15), establish).await?;

    // Verify we can get it.
    info!("Get Pod blog");
    let p1cpy = pods.get("blog").await.unwrap();
    if let Some(spec) = &p1cpy.spec {
        info!("Got blog pod with containers: {:?}", spec.containers);
        assert_eq!(spec.containers[0].name, "blog");
    }

    // Replace its spec.
    info!("Patch Pod blog");
    let patch = json!({
        "metadata": {
            "resourceVersion": p1cpy.resource_version(),
        },
        "spec": {
            "activeDeadlineSeconds": 5
        }
    });
    let patchparams = PatchParams::default();
    let p_patched = pods
        .patch("blog", &patchparams, &Patch::Merge(&patch))
        .await
        .unwrap();
    assert_eq!(p_patched.spec.unwrap().active_deadline_seconds, Some(5));

    // List the pods.
    let lp = ListParams::default().fields(&format!("metadata.name={}", "blog")); // only want results for our pod
    for p in pods.list(&lp).await.unwrap() {
        info!("Found Pod: {}", p.name_any());
    }

    // Delete it.
    let dp = DeleteParams::default();
    pods.delete("blog", &dp).await.unwrap().map_left(|pdel| {
        assert_eq!(pdel.name_any(), "blog");
        info!("Deleting blog pod started: {:?}", pdel);
    });

    Ok(())
}
