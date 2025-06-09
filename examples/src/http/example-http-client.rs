use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use volo_http::{
    body::BodyConversion,
    client::{get, ClientBuilder},
    error::BoxError,
};

#[derive(Deserialize, Serialize, Debug)]
struct Person {
    name: String,
    age: u8,
    phones: Vec<String>,
}

#[volo::main]
async fn main() -> Result<(), BoxError> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // simple `get` function with dns resolve
    println!(" ==== http://httpbin.org/get ====");
    println!(
        "{}",
        get("http://httpbin.org/get").await?.into_string().await?
    );

    // HTTPS `get`
    #[cfg(feature = "__tls")]
    {
        println!(" ==== https://httpbin.org/get ====");
        println!(
            "{}",
            get("https://httpbin.org/get").await?.into_string().await?
        );
    }

    // create client by builder
    let example_client = {
        let mut builder = ClientBuilder::new();
        builder
            .user_agent("example.http.client")
            .default_host("example.http.server")
            // set default target address
            .target_address("127.0.0.1:8080".parse::<SocketAddr>().unwrap())
            .header("Test", "Test");
        builder.build()?
    };

    println!(" ==== http://127.0.0.1:8080/ ====");
    println!(
        "{}",
        example_client
            .get("http://127.0.0.1:8080/")
            .send()
            .await?
            .into_string()
            .await?
    );
    println!(" ==== http://127.0.0.1:8080/ ====");
    println!(
        "{}",
        example_client.get("/").send().await?.into_string().await?
    );

    let httpbin_client = {
        let mut builder = ClientBuilder::new();
        builder.target_domain("httpbin.org");
        builder.build()?
    };

    // set host and override the default one
    println!(" ==== http://httpbin.org/get ====");
    println!(
        "{}",
        httpbin_client
            .request_builder()
            .host("httpbin.org")
            .uri("/get")
            .send()
            .await?
            .into_string()
            .await?
    );

    // an empty client
    let empty_client = ClientBuilder::new().build()?;
    println!(" ==== http://127.0.0.1:8080/ ====");
    println!(
        "{}",
        empty_client
            .get("http://127.0.0.1:8080/")
            .send()
            .await?
            .into_string()
            .await?
    );
    println!(" ==== http://127.0.0.1:8080/user/json_get ====");
    println!(
        "{:?}",
        httpbin_client
            .request_builder()
            .uri("http://127.0.0.1:8080/user/json_get")
            .send()
            .await?
            .into_json::<Person>()
            .await?
    );
    // invalid request because there is no target address
    println!(" ==== invalid url, error is expected ====");
    println!(
        "{:?}",
        empty_client
            .get("/")
            .send()
            .await
            .expect_err("this request should fail"),
    );

    Ok(())
}
