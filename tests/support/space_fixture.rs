//! Generated, independent files with atomic guards, used for recovery races.
#![expect(
    dead_code,
    reason = "shared by storage/network and native recovery contracts"
)]
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, Response},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Clone)]
pub struct Entry {
    pub body: String,
    pub revision: u64,
}
#[derive(Default)]
pub struct Data {
    pub files: BTreeMap<String, Entry>,
    pub revision: u64,
    pub content_addressed: bool,
    pub lost_delete: Option<String>,
    pub deleted: bool,
    pub delay_delete: Option<String>,
    pub replace_before_delete: Option<(String, String)>,
    pub delete_requests: Vec<String>,
    pub delete_received: bool,
    pub lost_put: Option<String>,
    pub denied_put: Option<String>,
    pub read_only: Option<String>,
    pub delay_put: Option<String>,
    pub puts: Vec<(String, String, bool)>,
    pub release: Arc<tokio::sync::Notify>,
    pub deletes: Vec<(String, String)>,
}
pub struct SpaceFixture {
    pub base: url::Url,
    pub data: Arc<Mutex<Data>>,
    task: tokio::task::JoinHandle<()>,
}
impl SpaceFixture {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = url::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let data = Arc::new(Mutex::new(Data::default()));
        let router = Router::new().fallback(handle).with_state(data.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { base, data, task }
    }
    pub fn put(&self, path: &str, body: &str) {
        let mut data = self.data.lock().unwrap();
        data.revision += 1;
        let revision = if data.content_addressed {
            content_revision(body)
        } else {
            data.revision
        };
        data.files.insert(
            path.into(),
            Entry {
                body: body.into(),
                revision,
            },
        );
    }
    pub fn body(&self, path: &str) -> Option<String> {
        self.data
            .lock()
            .unwrap()
            .files
            .get(path)
            .map(|entry| entry.body.clone())
    }
}
impl Drop for SpaceFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn content_revision(body: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    // Content identity, deliberately not an incarnation counter. The algorithm
    // is immaterial to the equality contract exercised by the fixture.
    let mut hash = std::hash::DefaultHasher::new();
    body.hash(&mut hash);
    hash.finish()
}
fn reply(status: u16, body: impl Into<Body>, revision: Option<u64>) -> Response<Body> {
    let mut response = Response::builder()
        .status(status)
        .header("content-type", "text/markdown")
        .header("x-permission", "rw");
    if let Some(revision) = revision {
        response = response.header("etag", format!("\"space-{revision}\""));
    }
    response.body(body.into()).unwrap()
}
fn decode(path: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut input = path.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = (input.next()? as char).to_digit(16)?;
            let low = (input.next()? as char).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}
async fn handle(state: State<Arc<Mutex<Data>>>, request: Request<Body>) -> Response<Body> {
    // Accepted server work survives the client dropping its HTTP connection.
    tokio::spawn(process(state, request)).await.unwrap()
}
async fn process(State(state): State<Arc<Mutex<Data>>>, request: Request<Body>) -> Response<Body> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 64 * 1024 * 1024).await.unwrap();
    let Some(path) = decode(parts.uri.path()) else {
        return reply(400, "", None);
    };
    if path == "/.fs" && parts.method == Method::GET {
        let data = state.lock().unwrap();
        let files:Vec<_>=data.files.iter().map(|(name,file)|serde_json::json!({"name":name,"size":file.body.len(),"lastModified":1,"perm":if data.read_only.as_deref()==Some(name.as_str()) {"ro"}else{"rw"},"etag":format!("\"space-{}\"",file.revision)})).collect();
        return Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&files).unwrap()))
            .unwrap();
    }
    let Some(path) = path.strip_prefix("/.fs/") else {
        return reply(404, "", None);
    };
    let delay = {
        let mut data = state.lock().unwrap();
        if parts.method == Method::DELETE && data.delay_delete.as_deref() == Some(path) {
            data.delay_delete = None;
            data.delete_received = true;
            Some(data.release.clone())
        } else if parts.method == Method::PUT && data.delay_put.as_deref() == Some(path) {
            data.delay_put = None;
            Some(data.release.clone())
        } else {
            None
        }
    };
    if let Some(delay) = delay {
        delay.notified().await;
    }
    let (response, wait) = {
        let mut data = state.lock().unwrap();
        if parts.method == Method::DELETE {
            data.delete_requests.push(path.into());
            if data
                .replace_before_delete
                .as_ref()
                .is_some_and(|(name, _)| name == path)
            {
                let (_, body) = data.replace_before_delete.take().unwrap();
                data.revision += 1;
                let revision = data.revision;
                data.files.insert(path.into(), Entry { body, revision });
            }
        }
        let existing = data.files.get(path).cloned();
        match parts.method {
            Method::GET | Method::HEAD => {
                return match existing {
                    Some(file) => {
                        let mut response = reply(
                            200,
                            if parts.method == Method::HEAD {
                                String::new()
                            } else {
                                file.body
                            },
                            Some(file.revision),
                        );
                        if data.read_only.as_deref() == Some(path) {
                            response
                                .headers_mut()
                                .insert("x-permission", "ro".parse().unwrap());
                        }
                        response
                    }
                    None => reply(404, "", None),
                };
            }
            Method::PUT | Method::DELETE => {
                let expected = parts.headers.get("if-match").and_then(|v| v.to_str().ok());
                let create = parts.headers.get("if-none-match").is_some_and(|v| v == "*");
                if parts.method == Method::PUT {
                    data.puts.push((
                        path.into(),
                        String::from_utf8(body.to_vec()).unwrap(),
                        create,
                    ));
                    if data.denied_put.as_deref() == Some(path) {
                        return reply(403, "", None);
                    }
                }
                let permitted = if create {
                    parts.method == Method::PUT && existing.is_none()
                } else {
                    existing.as_ref().is_some_and(|entry| {
                        expected == Some(format!("\"space-{}\"", entry.revision).as_str())
                    })
                };
                if !permitted {
                    return reply(412, "", None);
                }
                data.revision += 1;
                let mut revision = data.revision;
                if parts.method == Method::PUT {
                    let Ok(body) = String::from_utf8(body.to_vec()) else {
                        return reply(400, "", None);
                    };
                    revision = if data.content_addressed {
                        content_revision(&body)
                    } else {
                        revision
                    };
                    data.files.insert(path.into(), Entry { body, revision });
                } else {
                    data.files.remove(path);
                    data.deletes.push((path.into(), expected.unwrap().into()));
                }
                let wait = if parts.method == Method::DELETE
                    && data.lost_delete.as_deref() == Some(path)
                {
                    data.lost_delete = None;
                    data.deleted = true;
                    Some(data.release.clone())
                } else if parts.method == Method::PUT && data.lost_put.as_deref() == Some(path) {
                    data.lost_put = None;
                    Some(data.release.clone())
                } else {
                    None
                };
                (reply(200, "OK", Some(revision)), wait)
            }
            _ => return reply(405, "", None),
        }
    };
    if let Some(wait) = wait {
        wait.notified().await;
        tokio::time::sleep(Duration::from_millis(150)).await;
        return reply(503, "", None);
    }
    response
}
