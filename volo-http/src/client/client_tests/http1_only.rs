// FIXME:
//
// `httpbin.org` supports h2 (HTTP/2 with tls), but doesn't support h2c (HTTP/2 over Cleartext),
// just disable those test cases.
//
// TODO:
//
// Find a website that support h2c.

use std::{
    any::TypeId,
    collections::HashMap,
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use bytes::Bytes;
use http::{header, status::StatusCode};
use http_body_util::Full;
use motore::service::Service;
use volo::context::Context;

use super::{
    utils::{
        AutoBody, AutoBodyLayer, AutoFull, AutoFullLayer, DropBodyLayer, Nothing,
        RespBodyToFullLayer,
    },
    HttpBinResponse, HTTPBIN_GET, HTTPBIN_POST, USER_AGENT_KEY, USER_AGENT_VAL,
};
use crate::{
    body::{Body, BodyConversion},
    client::{
        dns::DnsResolver,
        get,
        layer::FailOnStatus,
        test_helpers::{DebugLayer, MockTransport},
        CallOpt, Client,
    },
    context::client::Config,
    error::client::{ClientError, ErrorKind},
    response::Response,
    utils::consts::HTTP_DEFAULT_PORT,
};

#[tokio::test]
async fn client_with_generics() {
    fn type_of<T: 'static>(_: &T) -> TypeId {
        TypeId::of::<T>()
    }

    // Override default `ReqBody`, but the `ReqBody` is still implements `http_body::Body`
    {
        let client = Client::builder().build().unwrap();
        assert!(client
            .post(HTTPBIN_POST)
            .body(Full::new(Bytes::new()))
            .send()
            .await
            .is_ok());
        assert_eq!(TypeId::of::<Client<Full<Bytes>>>(), type_of(&client),);
    }
    // Override default `RespBody`, but the `RespBody` is still implements `http_body::Body`
    {
        let client = Client::builder()
            .layer_outer_front(RespBodyToFullLayer)
            .build()
            .unwrap();
        assert!(client.get(HTTPBIN_GET).send().await.is_ok());
        assert_eq!(TypeId::of::<Client<Body, Full<Bytes>>>(), type_of(&client),);
    }
    // Override default `ReqBody` through `Layer`. The `AutoBody` does not implement
    // `http_body::Body`, but the `AutoBodyLayer` will convert it to `volo_http::body::Body` and
    // use it.
    {
        let client = Client::builder()
            .layer_outer_front(AutoBodyLayer)
            .build()
            .unwrap();
        assert!(client
            .post(HTTPBIN_POST)
            .body(AutoBody)
            .send()
            .await
            .is_ok());
        assert_eq!(TypeId::of::<Client<AutoBody>>(), type_of(&client),);
    }
    // Override default `ReqBody` through `Layer`. The `AutoFull` does not implement
    // `http_body::Body`, but the `AutoFullLayer` will convert it to `Full<Bytes>` which implements
    // `http_body::Body` as its `InnerReqBody`.
    {
        let client = Client::builder()
            .layer_outer_front(AutoFullLayer)
            .build()
            .unwrap();
        assert!(client
            .post(HTTPBIN_POST)
            .body(AutoFull)
            .send()
            .await
            .is_ok());
        assert_eq!(TypeId::of::<Client<AutoFull>>(), type_of(&client),);
    }
    // Override default `RespBody` through `Layer`. The `RespBody` does not implement
    // `http_body::Body`, but the `DropBodyLayer` will drop `volo_http::body::Body` and put
    // `Nothing` to `Response`.
    {
        let client = Client::builder()
            .layer_outer_front(DropBodyLayer)
            .build()
            .unwrap();
        assert!(client.get(HTTPBIN_GET).send().await.is_ok());
        assert_eq!(TypeId::of::<Client<Body, Nothing>>(), type_of(&client),);
    }
    // Combine them
    {
        let client = Client::builder()
            .layer_outer_front(AutoFullLayer)
            .layer_outer_front(DropBodyLayer)
            .build()
            .unwrap();
        assert!(client.post(HTTPBIN_GET).body(AutoFull).send().await.is_ok());
        assert_eq!(TypeId::of::<Client<AutoFull, Nothing>>(), type_of(&client),);
    }
}

#[cfg(feature = "json")]
#[tokio::test]
async fn simple_get() {
    let resp = get(HTTPBIN_GET)
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert!(resp.args.is_empty());
    assert_eq!(resp.url, HTTPBIN_GET);
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_builder_with_header() {
    let mut builder = Client::builder().layer_inner(DebugLayer::default());
    builder.header(header::USER_AGENT, USER_AGENT_VAL);
    let client = builder.build().unwrap();

    let resp = client
        .get(HTTPBIN_GET)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert!(resp.args.is_empty());
    assert_eq!(resp.headers.get(USER_AGENT_KEY).unwrap(), USER_AGENT_VAL);
    assert_eq!(resp.url, HTTPBIN_GET);
}

// Test cases:
//
// 1. default target
//   a. have default target
//   b. no default target
// 2. request target
//   a. have request target
//   b. no request target
// 3. default host
//   a. have default host
//   b. use auto host
//
// aaa -> client_builder_addr_override
// aab -> client_builder_host_override
// aba -> client_builder_with_domain_default_host & client_builder_with_address
// abb -> client_builder_with_domain
// baa -> client_builder_with_default_host
// bab -> simple_get & client_builder_with_header
// bba/bbb -> no_target_request

#[cfg(feature = "json")]
#[tokio::test]
async fn client_builder_addr_override() {
    let invalid_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8888);
    let httpbin_addr = DnsResolver::default()
        .resolve("httpbin.org", HTTP_DEFAULT_PORT)
        .await
        .unwrap();

    // default target (valid addr to httpbin)
    // request target (invalid addr)
    // default host
    {
        let mut builder = Client::builder().layer_inner(DebugLayer::default());
        builder
            .default_host("httpbin.org")
            .target_address(httpbin_addr.clone());
        let client = builder.build().unwrap();
        let resp = client
            .get(format!("http://{invalid_addr}/get"))
            .send()
            .await
            .unwrap()
            .into_json::<HttpBinResponse>()
            .await
            .unwrap();
        assert!(resp.args.is_empty());
        // The authority is an IP address and the address will be ignored.
        assert_eq!(resp.url, HTTPBIN_GET);
    }
    // default target (valid addr to httpbin)
    // request target (invalid addr)
    // default host (it's strange but httpbin does not care it)
    {
        let strange_domain = "hpptboom.org";
        let mut builder = Client::builder().layer_inner(DebugLayer::default());
        builder
            .default_host(strange_domain)
            .target_address(httpbin_addr.clone());
        let client = builder.build().unwrap();
        let resp = client
            .get(format!("http://{invalid_addr}/get"))
            .send()
            .await
            .unwrap()
            .into_json::<HttpBinResponse>()
            .await
            .unwrap();
        assert!(resp.args.is_empty());
        // The authority is a domain name and it will be used as header `Host`, but the destination
        // address is still the `target_address`.
        assert_eq!(resp.url, format!("http://{strange_domain}/get"));
    }
    // default target (invalid addr)
    // request target
    // default host
    {
        let mut builder = Client::builder().layer_inner(DebugLayer::default());
        builder
            .default_host("httpbin.org")
            .target_address(invalid_addr);
        let client = builder.build().unwrap();

        let err = client
            .get(format!("http://{httpbin_addr}/get"))
            .send()
            .await
            .unwrap_err();
        // The new address cannot override the `target_address`, so it fails
        assert_eq!(err.kind(), &ErrorKind::Connect);
    }
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_builder_host_override() {
    // default target (valid domain httpbin.org)
    // request target (invalid domain but httpbin does not care it)
    // no host
    {
        let mut builder = Client::builder().layer_inner(DebugLayer::default());
        builder.target_domain("httpbin.org");
        let client = builder.build().unwrap();
        let test_url = "http://hpptboom.org/get";

        let resp = client
            .get(test_url)
            .send()
            .await
            .unwrap()
            .into_json::<HttpBinResponse>()
            .await
            .unwrap();
        assert!(resp.args.is_empty());
        // Well `httpbin.org` will concat scheme+host+uri directly without checking the `Host`, so
        // we can get the strange url as result.
        assert_eq!(resp.url, test_url);
    }
    // default target (invalid domain)
    // request target (valid domain httpbin.org but it does not work)
    // no host
    {
        let mut builder = Client::builder().layer_inner(DebugLayer::default());
        builder.target_domain("this.domain.must.be.invalid");
        let client = builder.build().unwrap();

        let err = client.get(HTTPBIN_GET).send().await.unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::LoadBalance);
    }
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_builder_with_domain_default_host() {
    // default target, no request target, default host
    let strange_host = "hpptboom.org";
    let mut builder = Client::builder().layer_inner(DebugLayer::default());
    builder
        .target_domain("httpbin.org")
        .default_host(strange_host);
    let client = builder.build().unwrap();

    let resp = client
        .get("/get")
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert!(resp.args.is_empty());
    assert_eq!(resp.url, format!("http://{strange_host}/get"));
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_builder_with_address() {
    // default target, no request target, default host
    let addr = DnsResolver::default()
        .resolve("httpbin.org", HTTP_DEFAULT_PORT)
        .await
        .unwrap();
    let mut builder = Client::builder().layer_inner(DebugLayer::default());
    builder.default_host("httpbin.org").target_address(addr);
    let client = builder.build().unwrap();

    let resp = client
        .get("/get")
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert!(resp.args.is_empty());
    assert_eq!(resp.url, HTTPBIN_GET);
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_builder_with_domain() {
    // default target, no request target, auto host
    let mut builder = Client::builder().layer_inner(DebugLayer::default());
    builder.target_domain("httpbin.org");
    let client = builder.build().unwrap();

    let resp = client
        .get("/get")
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert!(resp.args.is_empty());
    assert_eq!(resp.url, HTTPBIN_GET);
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_builder_with_default_host() {
    // no default target, have request target, default host
    let mut builder = Client::builder().layer_inner(DebugLayer::default());
    builder.default_host("hpptboom.org");
    let client = builder.build().unwrap();

    let resp = client
        .get(HTTPBIN_GET)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert!(resp.args.is_empty());
    assert_eq!(resp.url, HTTPBIN_GET);
}

#[tokio::test]
async fn no_target_request() {
    let client = Client::default();
    let err = client.get("/").send().await.unwrap_err();
    // ClientError {
    //     kind: LoadBalance,
    //     source: Some(
    //         Discover(
    //             ClientError { kind: Builder, source: Some(NoAddress), uri: None, addr: None },
    //         ),
    //     ),
    //     uri: None,
    //     addr: None,
    // }
    assert_eq!(err.kind(), &ErrorKind::LoadBalance);
}

#[tokio::test]
async fn client_builder_with_port() {
    let mut builder = Client::builder().layer_inner(DebugLayer::default());
    builder.target_domain("httpbin.org").with_port(443);
    let client = builder.build().unwrap();

    let resp = client.get("/get").send().await.unwrap();
    // Send HTTP request to the HTTPS port (443), `httpbin.org` will response `400 Bad
    // Request`.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

fn test_data() -> HashMap<String, String> {
    HashMap::from([
        ("key1".to_string(), "val1".to_string()),
        ("key2".to_string(), "val2".to_string()),
    ])
}

#[cfg(all(feature = "query", feature = "json"))]
#[tokio::test]
async fn set_query() {
    let data = test_data();

    let client = Client::builder().build().unwrap();
    let resp = client
        .get("http://httpbin.org/get")
        .set_query(&data)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert_eq!(resp.args, data);
}

#[cfg(all(feature = "form", feature = "json"))]
#[tokio::test]
async fn set_form() {
    let data = test_data();

    let client = Client::builder().build().unwrap();
    let resp = client
        .post("http://httpbin.org/post")
        .form(&data)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert_eq!(resp.form, data);
}

#[cfg(feature = "json")]
#[tokio::test]
async fn set_json() {
    let data = test_data();

    let client = Client::builder().build().unwrap();
    let resp = client
        .post("http://httpbin.org/post")
        .json(&data)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert_eq!(resp.json, Some(data));
}

struct GetTimeoutAsSeconds;

impl<Cx, Req> Service<Cx, Req> for GetTimeoutAsSeconds
where
    Cx: Context<Config = Config>,
{
    type Response = Response;
    type Error = ClientError;

    fn call(
        &self,
        cx: &mut Cx,
        _: Req,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send {
        let timeout = cx.rpc_info().config().timeout();
        let resp = match timeout {
            Some(timeout) => {
                let secs = timeout.as_secs();
                Response::new(Body::from(format!("{secs}")))
            }
            None => {
                let mut resp = Response::new(Body::empty());
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                resp
            }
        };
        async { Ok(resp) }
    }
}

#[tokio::test]
async fn callopt_test() {
    let mut builder = Client::builder();
    builder.set_request_timeout(Duration::from_secs(1));
    let client = builder
        .layer_outer_front(FailOnStatus::server_error())
        .mock(MockTransport::service(GetTimeoutAsSeconds))
        .unwrap();
    // default timeout is 1 seconds
    assert_eq!(
        client
            .get("/")
            .send()
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap(),
        "1"
    );
    // callopt set timeout to 5 seconds
    assert_eq!(
        client
            .get("/")
            .with_callopt(CallOpt::new().with_timeout(Duration::from_secs(5)))
            .send()
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap(),
        "5"
    );
}

#[cfg(all(feature = "cookie", feature = "json"))]
#[tokio::test]
async fn cookie_store() {
    let mut builder = Client::builder()
        .layer_inner(DebugLayer::default())
        .layer_inner(crate::client::cookie::CookieLayer::new(Default::default()));

    builder.target_domain("httpbin.org");

    let client = builder.build().unwrap();

    // test server add cookie
    let resp = client
        .get("http://httpbin.org/cookies/set?key=value")
        .send()
        .await
        .unwrap();
    let cookies = resp
        .headers()
        .get_all(http::header::SET_COOKIE)
        .iter()
        .filter_map(|value| {
            std::str::from_utf8(value.as_bytes())
                .ok()
                .and_then(|val| cookie::Cookie::parse(val).map(|c| c.into_owned()).ok())
        })
        .collect::<Vec<_>>();
    assert_eq!(cookies[0].name(), "key");
    assert_eq!(cookies[0].value(), "value");

    #[derive(serde::Deserialize)]
    struct CookieResponse {
        #[serde(default)]
        cookies: HashMap<String, String>,
    }
    let resp = client
        .get("http://httpbin.org/cookies")
        .send()
        .await
        .unwrap();
    let json = resp.into_json::<CookieResponse>().await.unwrap();
    assert_eq!(json.cookies["key"], "value");

    // test server delete cookie
    _ = client
        .get("http://httpbin.org/cookies/delete?key")
        .send()
        .await
        .unwrap();
    let resp = client
        .get("http://httpbin.org/cookies")
        .send()
        .await
        .unwrap();
    let json = resp.into_json::<CookieResponse>().await.unwrap();
    assert_eq!(json.cookies.len(), 0);
}
