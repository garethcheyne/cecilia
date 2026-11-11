//! The MCP endpoint: handshake and a tool end to end.

mod common;

use common::*;
use serde_json::{json, Value};

async fn rpc(s: &TestServer, method: &str, params: Value) -> Value {
    let r = s
        .http
        .post(format!("{}/mcp", s.base))
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.json().await.unwrap()
}

#[tokio::test]
async fn initialize_reports_the_assistant_name() {
    let s = TestServer::start().await;
    let v = rpc(
        &s,
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }),
    )
    .await;
    assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
    assert!(v["result"]["serverInfo"]["title"].as_str().unwrap().starts_with("Cecilia"));
}

#[tokio::test]
async fn draws_stops_by_spoken_name() {
    let s = TestServer::with_simulator().await;
    let v = rpc(
        &s,
        "tools/call",
        json!({ "name": "draw_stops", "arguments": { "stops": ["open diapason", "swell to great"] } }),
    )
    .await;
    let reply = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(reply.contains("Open Diapason"), "{reply}");
    sleep_ms(50).await;
    assert_eq!(stops(&s.sim().await), ["gt-opendiap", "gt-sw"]);
}
