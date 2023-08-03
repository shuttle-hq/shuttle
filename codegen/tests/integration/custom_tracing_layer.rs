use reqwest::Client;
use serde_json::Value as JsonValue;
use shuttle_common_tests::cargo_shuttle::cargo_shuttle_run;
use tokio::time::Duration;

#[tokio::test]
async fn custom_tracing_layer() {
    // Spin up the example
    let path = "../examples/tracing/axum-logs-endpoint";
    let url = cargo_shuttle_run(path, false).await;

    // Prepare URLs
    let get_url = format!("{url}/logs/3");
    let post_url1 = format!("{url}/message/hello");
    let post_url2 = format!("{url}/message/world");
    let post_url3 = format!("{url}/message/how%20are%20you%3F");

    let client1 = Client::new();
    let client2 = client1.clone();

    // Send the initial GET request
    let get = tokio::spawn(async move {
        client1
            .get(get_url)
            .send()
            .await
            .unwrap()
            .json::<Vec<JsonValue>>()
            .await
            .unwrap()
    });

    // Wait for the request to send
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send some messages
    tokio::spawn(async move {
        client2.post(post_url1).send().await.unwrap();
        client2.post(post_url2).send().await.unwrap();
        client2.post(post_url3).send().await.unwrap();
    });

    // Receive messages and validate them
    let result = get.await.unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(
        result[0]["fields"]["message"].as_str().unwrap(),
        "\"hello\""
    );
    assert_eq!(
        result[1]["fields"]["message"].as_str().unwrap(),
        "\"world\""
    );
    assert_eq!(
        result[2]["fields"]["message"].as_str().unwrap(),
        "\"how are you?\""
    );
}
