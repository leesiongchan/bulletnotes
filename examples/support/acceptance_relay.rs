//! Example-only allowlist boundary; never used by the production application.
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, Response},
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone)]
pub struct Relay(Arc<Inner>);
struct Inner {
    upstream: url::Url,
    client: reqwest::Client,
    paths: Mutex<BTreeMap<String, Vec<String>>>,
    journal: PathBuf,
    pub offline: AtomicBool,
    gate: Mutex<Option<String>>,
    pub waiting: AtomicBool,
    release: tokio::sync::Notify,
    pub rejected: AtomicUsize,
}
impl Relay {
    pub async fn start(
        upstream: url::Url,
        journal: PathBuf,
        secure: bool,
    ) -> Result<(Self, url::Url, tokio::task::JoinHandle<()>)> {
        if upstream.query().is_some()
            || upstream.fragment().is_some()
            || !upstream.username().is_empty()
            || upstream.password().is_some()
            || secure && upstream.scheme() != "https"
            || !secure && upstream.host_str() != Some("127.0.0.1")
        {
            return Err("invalid acceptance upstream".into());
        }
        let client = reqwest::Client::builder()
            .https_only(secure)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .retry(reqwest::retry::never())
            .build()?;
        let relay = Self(Arc::new(Inner {
            upstream,
            client,
            paths: Mutex::new(BTreeMap::new()),
            journal,
            offline: AtomicBool::new(false),
            gate: Mutex::new(None),
            waiting: AtomicBool::new(false),
            release: tokio::sync::Notify::new(),
            rejected: AtomicUsize::new(0),
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let base = url::Url::parse(&format!("http://{}/", listener.local_addr()?))?;
        let router = Router::new().fallback(handle).with_state(relay.clone());
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok((relay, base, task))
    }
    fn endpoint(&self, path: &str) -> url::Url {
        let mut url = self.0.upstream.clone();
        url.path_segments_mut()
            .unwrap()
            .pop_if_empty()
            .push(".fs")
            .extend(path.split('/'));
        url
    }
    pub fn register(&self, path: &str, sources: &[&str]) -> Result<()> {
        bulletnotes::domain::page::PagePath::parse(path)?;
        let mut paths = self.0.paths.lock().unwrap();
        let mut next = paths.clone();
        let registered = next.entry(path.into()).or_default();
        for source in sources {
            if !registered.iter().any(|value| value == source) {
                registered.push((*source).into());
            }
        }
        std::fs::write(&self.0.journal, serde_json::to_vec_pretty(&next)?)?;
        *paths = next;
        Ok(())
    }
    pub fn offline(&self, value: bool) {
        self.0.offline.store(value, Ordering::SeqCst);
    }
    pub fn arm(&self, path: &str) {
        self.0.waiting.store(false, Ordering::SeqCst);
        *self.0.gate.lock().unwrap() = Some(path.into());
    }
    pub fn waiting(&self) -> bool {
        self.0.waiting.load(Ordering::SeqCst)
    }
    pub fn release(&self) {
        self.0.release.notify_one();
    }
    pub fn rejected(&self) -> usize {
        self.0.rejected.load(Ordering::SeqCst)
    }
    pub fn paths(&self) -> BTreeMap<String, Vec<String>> {
        self.0.paths.lock().unwrap().clone()
    }
}
fn reply(status: u16, body: impl Into<Body>) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.into())
        .unwrap()
}
async fn handle(State(relay): State<Relay>, request: Request<Body>) -> Response<Body> {
    match process(&relay, request).await {
        Ok(response) => response,
        Err(_) => reply(502, ""),
    }
}
async fn process(relay: &Relay, request: Request<Body>) -> Result<Response<Body>> {
    let (parts, body) = request.into_parts();
    if parts.uri.query().is_some() {
        return Ok(reply(403, ""));
    }
    let catalogue = parts.uri.path() == "/.fs" && parts.method == Method::GET;
    // Hold a new destination briefly so the supervisor can journal its exact local event.
    // Nothing is forwarded until registration; unknown requests still fail closed.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    let (paths, path) = loop {
        let paths = relay.paths();
        let path = paths
            .keys()
            .find(|path| relay.endpoint(path).path() == parts.uri.path())
            .cloned();
        if catalogue || path.is_some() || tokio::time::Instant::now() >= deadline {
            break (paths, path);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    if !catalogue && path.is_none() {
        relay.0.rejected.fetch_add(1, Ordering::SeqCst);
        return Ok(reply(403, ""));
    }
    if relay.0.offline.load(Ordering::SeqCst) {
        return Ok(reply(503, ""));
    }
    let body = to_bytes(body, 3 * 1024 * 1024).await?;
    if let Some(path) = &path {
        if !matches!(
            parts.method,
            Method::GET | Method::HEAD | Method::PUT | Method::DELETE
        ) {
            return Ok(reply(405, ""));
        }
        if parts.method == Method::PUT
            && !paths[path]
                .iter()
                .any(|source| source.as_bytes() == body.as_ref())
        {
            relay.0.rejected.fetch_add(1, Ordering::SeqCst);
            return Ok(reply(403, ""));
        }
        if matches!(parts.method, Method::PUT | Method::DELETE)
            && !parts.headers.contains_key("if-match")
            && !(parts.method == Method::PUT
                && parts
                    .headers
                    .get("if-none-match")
                    .is_some_and(|value| value == "*"))
        {
            return Ok(reply(428, ""));
        }
    }
    let endpoint = if let Some(path) = &path {
        relay.endpoint(path)
    } else {
        let mut url = relay.0.upstream.clone();
        url.path_segments_mut().unwrap().pop_if_empty().push(".fs");
        url
    };
    let mut upstream = relay
        .0
        .client
        .request(parts.method.clone(), endpoint)
        .header("x-sync-mode", "true")
        .header("accept-encoding", "identity");
    for name in ["if-match", "if-none-match", "content-type", "accept"] {
        if let Some(value) = parts.headers.get(name) {
            upstream = upstream.header(name, value);
        }
    }
    let response = upstream.body(body).send().await?;
    let status = response.status();
    let headers = response.headers().clone();
    let limit = if catalogue {
        16 * 1024 * 1024
    } else {
        3 * 1024 * 1024
    };
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > limit {
            return Ok(reply(502, ""));
        }
        bytes.extend_from_slice(&chunk);
    }
    if catalogue && status.is_success() {
        let entries: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
        bytes = serde_json::to_vec(
            &entries
                .into_iter()
                .filter(|entry| {
                    entry["name"]
                        .as_str()
                        .is_some_and(|name| paths.contains_key(name))
                })
                .collect::<Vec<_>>(),
        )?;
    }
    let gated = path.as_ref().is_some_and(|path| {
        matches!(parts.method, Method::GET | Method::HEAD)
            && relay.0.gate.lock().unwrap().as_deref() == Some(path)
    });
    if gated {
        *relay.0.gate.lock().unwrap() = None;
        relay.0.waiting.store(true, Ordering::SeqCst);
        tokio::time::timeout(Duration::from_secs(8), relay.0.release.notified()).await?;
    }
    let mut output = Response::builder().status(status);
    for name in [
        "content-type",
        "etag",
        "x-silverbullet-revision",
        "x-permission",
        "last-modified",
    ] {
        if let Some(value) = headers.get(name) {
            output = output.header(name, value);
        }
    }
    Ok(output.body(Body::from(bytes))?)
}
