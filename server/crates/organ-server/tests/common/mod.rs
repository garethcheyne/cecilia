//! A real server on a spare localhost port, with its own data folder, no
//! sound and (optionally) the simulator wired as both MIDI in and out.

#![allow(dead_code)]

use std::time::{Duration, Instant};

use organ_server::{build, Config};
use reqwest::Method;
use serde_json::{json, Value};

pub struct TestServer {
    pub base: String,
    pub http: reqwest::Client,
    /// The data folder (removed when the server is dropped, if we own it).
    pub dir: std::path::PathBuf,
    _owned: Option<tempfile::TempDir>,
}

impl TestServer {
    pub async fn start() -> Self {
        let data = tempfile::tempdir().expect("temp dir");
        Self::start_in(data).await
    }

    pub async fn start_in(data: tempfile::TempDir) -> Self {
        let mut s = Self::start_at(data.path().to_path_buf()).await;
        s._owned = Some(data);
        s
    }

    /// A server on an existing data folder (not removed afterwards).
    pub async fn start_at(dir: std::path::PathBuf) -> Self {
        Self::start_with(Config::for_tests(dir)).await
    }

    pub async fn start_with(config: Config) -> Self {
        let dir = config.data_dir.clone();
        let app = build(config).await.expect("server builds");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app.router).await.unwrap() });
        let http = reqwest::Client::builder().cookie_store(true).build().unwrap();
        Self { base: format!("http://{addr}"), http, dir, _owned: None }
    }

    /// Start, then route the simulator as Organ in and Organ out.
    pub async fn with_simulator() -> Self {
        let s = Self::start().await;
        s.route_simulator().await;
        s
    }

    pub async fn route_simulator(&self) {
        self.put(
            "/midi/config",
            json!({ "inputs": ["Registra Simulator"], "output": "Registra Simulator" }),
        )
        .await;
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.base)
    }

    pub async fn request(&self, method: Method, path: &str, body: Option<Value>) -> (u16, Value) {
        let mut req = self.http.request(method, self.url(path));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.unwrap();
        let code = r.status().as_u16();
        let text = r.text().await.unwrap_or_default();
        (code, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    async fn ok(&self, method: Method, path: &str, body: Option<Value>) -> Value {
        let (code, v) = self.request(method.clone(), path, body).await;
        assert!((200..300).contains(&code), "{method} {path}: {code} {v}");
        v
    }

    pub async fn get(&self, path: &str) -> Value {
        self.ok(Method::GET, path, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Value {
        self.ok(Method::POST, path, Some(body)).await
    }

    pub async fn put(&self, path: &str, body: Value) -> Value {
        self.ok(Method::PUT, path, Some(body)).await
    }

    pub async fn patch(&self, path: &str, body: Value) -> Value {
        self.ok(Method::PATCH, path, Some(body)).await
    }

    pub async fn delete(&self, path: &str) -> u16 {
        self.request(Method::DELETE, path, None).await.0
    }

    /// Do something at the simulated console.
    pub async fn act(&self, event: Value) {
        self.post("/simulator/act", event).await;
    }

    pub async fn note(&self, division: &str, key: u8, velocity: u8) {
        self.act(json!({ "type": "note", "division": division, "key": key, "velocity": velocity }))
            .await;
    }

    pub async fn draw(&self, stop: &str, on: bool) {
        self.act(json!({ "type": "stop", "stop": stop, "on": on })).await;
    }

    pub async fn sim(&self) -> Value {
        self.get("/simulator").await
    }

    /// Poll `path` until `pred` holds (or panic after `secs`).
    pub async fn wait_for(&self, path: &str, secs: u64, pred: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let v = self.get(path).await;
            if pred(&v) {
                return v;
            }
            assert!(Instant::now() < deadline, "timed out waiting on {path}; last: {v}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Drawn stop ids in a simulator snapshot.
pub fn stops(sim: &Value) -> Vec<String> {
    let mut v: Vec<String> = sim["console"]["stops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    v.sort();
    v
}
