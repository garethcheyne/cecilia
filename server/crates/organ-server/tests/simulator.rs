//! The simulator and MIDI plumbing, driven over HTTP the way the web UI does.

mod common;

use common::*;
use reqwest::Method;
use serde_json::json;

#[tokio::test]
async fn health_and_organs() {
    let s = TestServer::start().await;
    assert_eq!(s.get("/health").await["name"], "registra");
    let organs = s.get("/organs").await;
    let ids: Vec<_> = organs.as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"demo") && ids.contains(&"makin-2m"), "{ids:?}");
    assert_eq!(s.request(Method::GET, "/organs/nope", None).await.0, 404);
}

#[tokio::test]
async fn simulator_ports_are_listed_and_selectable() {
    let s = TestServer::with_simulator().await;
    let ports = s.get("/midi/ports").await;
    assert!(ports["inputs"].as_array().unwrap().iter().any(|p| p == "Registra Simulator"));
    let status = s.get("/midi/status").await;
    assert_eq!(status["output"], "Registra Simulator");
    assert_eq!(status["inputs"], json!(["Registra Simulator"]));
    assert_eq!(s.get("/midi/config").await["output"], "Registra Simulator", "selection is saved");
}

#[tokio::test]
async fn console_actions_change_state_and_cancel_clears_it() {
    let s = TestServer::start().await;
    s.draw("gt-opendiap", true).await;
    s.note("great", 60, 90).await;
    let sim = s.sim().await;
    assert_eq!(stops(&sim), ["gt-opendiap"]);
    assert_eq!(sim["console"]["notes"]["great"], json!([60]));

    s.post("/simulator/reset", json!(null)).await;
    let sim = s.sim().await;
    assert!(stops(&sim).is_empty());
    assert!(sim["console"]["notes"].as_object().unwrap().is_empty());
}

#[tokio::test]
async fn switching_organ_starts_a_fresh_console() {
    let s = TestServer::start().await;
    s.draw("gt-opendiap", true).await;
    let snap = s.put("/simulator/organ", json!({ "organ_id": "makin-2m" })).await;
    assert_eq!(snap["organ_id"], "makin-2m");
    assert!(stops(&snap).is_empty());
    let (code, _) = s
        .request(Method::PUT, "/simulator/organ", Some(json!({ "organ_id": "nope" })))
        .await;
    assert_eq!(code, 404);
}

#[tokio::test]
async fn raw_test_recorder_round_trips_through_the_simulator() {
    let s = TestServer::with_simulator().await;
    s.post("/midi/test/record", json!(null)).await;
    s.draw("gt-principal", true).await;
    s.note("great", 64, 80).await;
    sleep_ms(60).await;
    s.note("great", 64, 0).await;
    let status = s.post("/midi/test/stop", json!(null)).await;
    assert_eq!(status["test"]["events"], 3);

    // Clear the console; the playback must draw the stop again.
    s.post("/simulator/reset", json!(null)).await;
    s.post("/midi/test/play", json!(null)).await;
    s.wait_for("/midi/status", 3, |v| v["test"]["state"] == "idle").await;
    assert_eq!(stops(&s.sim().await), ["gt-principal"]);
}

#[tokio::test]
async fn makin_toggles_are_never_sent_twice() {
    let s = TestServer::with_simulator().await;
    s.put("/simulator/organ", json!({ "organ_id": "makin-2m" })).await;
    s.post("/midi/test/record", json!(null)).await;
    s.draw("gt-opendiap", true).await;
    // Already drawn: a toggle organ must not be sent the toggle again.
    s.draw("gt-opendiap", true).await;
    let status = s.post("/midi/test/stop", json!(null)).await;
    assert_eq!(status["test"]["events"], 1);
    assert_eq!(stops(&s.sim().await), ["gt-opendiap"]);
}

#[tokio::test]
async fn foreign_websocket_origins_are_refused() {
    let s = TestServer::start().await;
    let r = s
        .http
        .get(s.url("/events"))
        .header("Origin", "http://evil.example")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
}
