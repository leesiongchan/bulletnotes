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
    HtmlCaps,
    FakeAck,
    InvalidUtf8,
    Encoded,
    MissingRevision,
    MissingPermission,
    ProxyRevision,
    InvalidDedicated,
    Catalogue(serde_json::Value),
    Redirect(String),
    TimeoutAfterWrite,
    DelayedCreate,
    IgnorePreconditions,
    SlowReads,
    BlockOneRead,
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
    pub catalogue_override: Option<serde_json::Value>,
    pub active_reads: usize,
    pub max_active_reads: usize,
    pub release: Arc<tokio::sync::Notify>,
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
            catalogue_override: None,
            active_reads: 0,
            max_active_reads: 0,
            release: Arc::new(tokio::sync::Notify::new()),
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
    let (reply, slow, reading, blocked) = {
        let mut data = state.lock().unwrap();
        data.seen.push(Seen {
            method: parts.method.clone(),
            uri: parts.uri.to_string(),
            headers: parts.headers.clone(),
            body: body.clone(),
        });
        let fault = data.fault.clone();
        if matches!(fault, Fault::DelayedCreate)
            && parts.method == Method::PUT
            && parts.headers.get("if-none-match").is_some_and(|v| v == "*")
        {
            let state = state.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let mut data = state.lock().unwrap();
                if data.body.is_none() {
                    data.body = Some(body);
                    data.revision += 1;
                }
            });
            return response(503, Vec::new(), &[]);
        }
        if let Fault::Status(status) = fault {
            return response(status, Vec::new(), &[]);
        }
        if let Fault::Redirect(location) = fault {
            return response(302, Vec::new(), &[("location", location)]);
        }
        if matches!(fault, Fault::FakeAck) {
            return response(
                200,
                b"please sign in".to_vec(),
                &[
                    ("content-type", "text/plain".into()),
                    ("etag", "\"login\"".into()),
                ],
            );
        }
        if matches!(fault, Fault::Html | Fault::HtmlCaps) {
            return response(
                200,
                b"<html>login</html>".to_vec(),
                &[
                    (
                        "content-type",
                        if matches!(fault, Fault::HtmlCaps) {
                            "Text/HTML".into()
                        } else {
                            "text/html".into()
                        },
                    ),
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
                _ if data.catalogue_override.is_some() => data.catalogue_override.clone().unwrap(),
                _ => {
                    if data.body.is_some() {
                        serde_json::json!([{"name":"sample.md","lastModified":100,"size":data.body.as_ref().map_or(0,Vec::len),"perm":"rw","etag":format!("\"fixture-{}\"",data.revision)}])
                    } else {
                        serde_json::json!([])
                    }
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
            Method::GET | Method::HEAD => {
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
                if parts.headers.get("if-none-match").is_some_and(|value| {
                    value.as_bytes() == format!("\"fixture-{}\"", data.revision).as_bytes()
                }) {
                    let validators: Vec<_> = headers
                        .iter()
                        .filter(|(name, _)| matches!(*name, "etag" | "x-silverbullet-revision"))
                        .cloned()
                        .collect();
                    return response(304, Vec::new(), &validators);
                }
                if parts.method == Method::HEAD {
                    return response(200, Vec::new(), &headers);
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
        let reading = parts.method == Method::GET;
        if reading {
            data.active_reads += 1;
            data.max_active_reads = data.max_active_reads.max(data.active_reads);
        }
        let blocked = (reading
            && matches!(fault, Fault::BlockOneRead)
            && parts.uri.path().ends_with("/blocked.md"))
        .then(|| data.release.clone());
        (
            reply,
            matches!(fault, Fault::TimeoutAfterWrite) && parts.method != Method::GET
                || reading && matches!(fault, Fault::SlowReads),
            reading,
            blocked,
        )
    };
    if let Some(blocked) = blocked {
        blocked.notified().await;
    }
    if slow {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    if reading {
        state.lock().unwrap().active_reads -= 1;
    }
    reply
}
