//! Stateful loopback SilverBullet contract fixture; all content is generated.
#![expect(
    dead_code,
    reason = "individual integration targets use different fixture faults and request fields"
)]
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, Method, Request, Response, StatusCode},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Clone, Default)]
pub enum Fault {
    #[default]
    None,
    Status(u16),
    Html,
    InvalidUtf8,
    Encoded,
    MissingRevision,
    MissingPermission,
    ProxyRevision,
    InvalidDedicated,
    Catalogue(serde_json::Value),
    Redirect(String),
    TimeoutAfterWrite,
    IgnorePreconditions,
}
#[derive(Clone)]
pub struct Seen {
    pub method: Method,
    pub uri: String,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
pub struct Data {
    pub body: Option<Vec<u8>>,
    pub revision: u64,
    pub fault: Fault,
    pub seen: Vec<Seen>,
}
pub struct Fixture {
    pub base: url::Url,
    pub data: Arc<Mutex<Data>>,
    task: tokio::task::JoinHandle<()>,
}
impl Fixture {
    pub async fn start(body: Option<&[u8]>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = url::Url::parse(&format!(
            "http://{}/prefix/",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let data = Arc::new(Mutex::new(Data {
            body: body.map(Vec::from),
            revision: 1,
            fault: Fault::None,
            seen: Vec::new(),
        }));
        let router = Router::new().fallback(handle).with_state(data.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { base, data, task }
    }
    pub fn fault(&self, fault: Fault) {
        self.data.lock().unwrap().fault = fault;
    }
    pub fn body(&self) -> Option<Vec<u8>> {
        self.data.lock().unwrap().body.clone()
    }
    pub fn seen(&self) -> Vec<Seen> {
        self.data.lock().unwrap().seen.clone()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn response(status: u16, body: Vec<u8>, headers: &[(&str, String)]) -> Response<Body> {
    let mut result = Response::builder().status(status);
    for (key, value) in headers {
        result = result.header(*key, value);
    }
    result.body(Body::from(body)).unwrap()
}
async fn handle(State(state): State<Arc<Mutex<Data>>>, request: Request<Body>) -> Response<Body> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 16 * 1024 * 1024).await.unwrap().to_vec();
    let (reply, slow) = {
        let mut data = state.lock().unwrap();
        data.seen.push(Seen {
            method: parts.method.clone(),
            uri: parts.uri.to_string(),
            headers: parts.headers.clone(),
            body: body.clone(),
        });
        let fault = data.fault.clone();
        if let Fault::Status(status) = fault {
            return response(status, Vec::new(), &[]);
        }
        if let Fault::Redirect(location) = fault {
            return response(302, Vec::new(), &[("location", location)]);
        }
        if matches!(fault, Fault::Html) {
            return response(
                200,
                b"<html>login</html>".to_vec(),
                &[
                    ("content-type", "text/html".into()),
                    ("etag", "\"login\"".into()),
                ],
            );
        }
        if parts
            .headers
            .get("x-sync-mode")
            .and_then(|v| v.to_str().ok())
            != Some("true")
        {
            return response(400, Vec::new(), &[]);
        }
        if parts.method == Method::GET && parts.uri.path().ends_with("/.fs") {
            let catalogue = match fault {
                Fault::Catalogue(value) => value,
                _ => {
                    serde_json::json!([{"name":"sample.md","lastModified":100,"size":data.body.as_ref().map_or(0,Vec::len),"perm":"rw","etag":format!("\"fixture-{}\"",data.revision)}])
                }
            };
            return response(
                200,
                serde_json::to_vec(&catalogue).unwrap(),
                &[("content-type", "application/json".into())],
            );
        }
        let revision = format!("\"fixture-{}\"", data.revision);
        let reply = match parts.method {
            Method::GET => {
                let Some(body) = &data.body else {
                    return response(404, Vec::new(), &[]);
                };
                let mut headers = vec![
                    ("content-type", "text/markdown; charset=utf-8".into()),
                    ("x-last-modified", "100".into()),
                ];
                if matches!(fault, Fault::Encoded) {
                    headers.push(("content-encoding", "gzip".into()));
                }
                if !matches!(fault, Fault::MissingPermission) {
                    headers.push(("x-permission", "rw".into()));
                }
                if !matches!(fault, Fault::MissingRevision) {
                    headers.push(("etag", revision));
                }
                if matches!(fault, Fault::ProxyRevision) {
                    headers.retain(|(k, _)| *k != "etag");
                    headers.push(("etag", "W/\"proxy\"".into()));
                    headers.push(("x-silverbullet-revision", "\"fixture-1\"".into()));
                }
                if matches!(fault, Fault::InvalidDedicated) {
                    headers.push(("x-silverbullet-revision", "W/\"invalid\"".into()));
                }
                if matches!(fault, Fault::Html) {
                    headers[0].1 = "text/html".into();
                    response(200, b"<html>login</html>".to_vec(), &headers)
                } else if matches!(fault, Fault::InvalidUtf8) {
                    response(200, vec![0xff], &headers)
                } else {
                    response(200, body.clone(), &headers)
                }
            }
            Method::PUT | Method::DELETE => {
                let create = parts
                    .headers
                    .get("if-none-match")
                    .and_then(|v| v.to_str().ok())
                    == Some("*");
                let expected = parts.headers.get("if-match").and_then(|v| v.to_str().ok());
                if !matches!(fault, Fault::IgnorePreconditions)
                    && !(create && parts.method == Method::PUT && data.body.is_none()
                        || !create && data.body.is_some() && expected == Some(revision.as_str()))
                {
                    return response(412, Vec::new(), &[]);
                }
                data.body = (parts.method == Method::PUT).then_some(body);
                data.revision += 1;
                response(
                    StatusCode::OK.as_u16(),
                    b"OK".to_vec(),
                    &[
                        ("content-type", "text/plain".into()),
                        ("etag", format!("\"fixture-{}\"", data.revision)),
                    ],
                )
            }
            _ => response(405, Vec::new(), &[]),
        };
        (
            reply,
            matches!(fault, Fault::TimeoutAfterWrite) && parts.method != Method::GET,
        )
    };
    if slow {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    reply
}
