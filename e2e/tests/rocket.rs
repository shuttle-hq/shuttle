use colored::Color;

mod helpers;

#[test]
fn hello_world() {
    let client = helpers::Api::new_docker("hello-world", Color::Green);
    client.deploy("../examples/rocket/hello-world");

    let request_text = client
        .get("hello")
        .header("Host", "hello-world-rocket-app.shuttleapp.rs")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");

    // Deploy a project which panics in its build function
    client.deploy("../examples/rocket/hello-world-panic");

    // Hello world should still be responsive
    let request_text = client
        .get("hello")
        .header("Host", "hello-world-rocket-app.shuttleapp.rs")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(request_text, "Hello, world!");
}

#[test]
fn postgres() {
    let client = helpers::Api::new_docker("postgres", Color::Blue);
    client.deploy("../examples/rocket/postgres");

    let add_response = client
        .post("todo")
        .body("{\"note\": \"To the stars\"}")
        .header("Host", "postgres-rocket-app.shuttleapp.rs")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(add_response, "{\"id\":1,\"note\":\"To the stars\"}");

    let fetch_response: String = client
        .get("todo/1")
        .header("Host", "postgres-rocket-app.shuttleapp.rs")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"id\":1,\"note\":\"To the stars\"}");

    // Deploy a project which panics in its state function
    client.deploy("../examples/rocket/postgres-panic");

    // Todo app should still be responsive
    let fetch_response: String = client
        .get("todo/1")
        .header("Host", "postgres-rocket-app.shuttleapp.rs")
        .send()
        .unwrap()
        .text()
        .unwrap();

    assert_eq!(fetch_response, "{\"id\":1,\"note\":\"To the stars\"}");
}
