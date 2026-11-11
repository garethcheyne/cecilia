//! Playlists and running a service, end to end through the simulator.

mod common;

use common::*;
use reqwest::Method;
use serde_json::{json, Value};

/// Record a very short piece on the simulator and return its id.
async fn record_short_piece(s: &TestServer) -> String {
    s.post("/recorder/start", json!(null)).await;
    s.draw("gt-stoppeddiap", true).await;
    for key in [60u8, 62] {
        s.note("great", key, 90).await;
        sleep_ms(30).await;
        s.note("great", key, 0).await;
    }
    s.post("/recorder/stop", json!(null)).await;
    let list = s.get("/library").await;
    list.as_array().unwrap().iter().find(|t| t["source"] == "recorded").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn track_row(id: &str, delay: Value) -> Value {
    json!({ "type": "track", "track_id": id, "delay": delay, "tempo": 400 })
}

#[tokio::test]
async fn playlists_are_created_saved_and_deleted() {
    let s = TestServer::start().await;
    let p = s.post("/playlists", json!({ "name": "Sunday 16 November" })).await;
    assert_eq!(p["id"], "sunday-16-november");
    let again = s.post("/playlists", json!({ "name": "Sunday 16 November" })).await;
    assert_eq!(again["id"], "sunday-16-november-2", "names can repeat, ids can't");

    let mut p = p;
    p["rows"] = json!([
        { "type": "text", "text": "Voluntary" },
        { "type": "track", "track_id": "sample-old-hundredth", "verses": 3, "note": "Hymn 1" }
    ]);
    let saved = s.put("/playlists/sunday-16-november", p).await;
    assert!(saved["rows"][1]["track_modified"].is_u64(), "tracks are marked as checked");
    assert_eq!(s.get("/playlists").await.as_array().unwrap().len(), 2);

    let copy = s.post("/playlists/sunday-16-november/duplicate", json!(null)).await;
    assert_eq!(copy["rows"].as_array().unwrap().len(), 2);

    let parsed = s.post("/verses/parse", json!({ "text": "12357" })).await;
    assert_eq!(parsed["verses"], json!([1, 2, 3, 5, 7]));
    let (code, _) = s.request(Method::POST, "/verses/parse", Some(json!({ "text": "a" }))).await;
    assert_eq!(code, 400);

    assert_eq!(s.delete("/playlists/sunday-16-november").await, 204);
    assert_eq!(s.delete("/playlists/sunday-16-november").await, 404);
}

#[tokio::test]
async fn a_service_runs_through_its_rows() {
    let s = TestServer::with_simulator().await;
    let rec = record_short_piece(&s).await;
    let mut p = s.post("/playlists", json!({ "name": "Evensong" })).await;
    p["rows"] = json!([
        { "type": "text", "text": "Before the service" },
        track_row(&rec, json!({ "type": "wait" })),
        track_row(&rec, json!({ "type": "after", "seconds": 1 })),
        { "type": "track", "track_id": "sample-old-hundredth", "delay": { "type": "cue" } }
    ]);
    s.put("/playlists/evensong", p).await;

    // Start without playing: the first track waits for Play.
    let st = s.post("/service/start", json!({ "playlist_id": "evensong" })).await;
    assert_eq!(st["state"], "ready");
    assert_eq!(st["row"], 1, "text rows are skipped");
    assert_eq!(st["next_row"], 2);

    s.post("/player/play", json!(null)).await;
    // When it ends, the next row counts down one second, then plays.
    s.wait_for("/service", 10, |v| v["row"] == 2 && v["state"] == "waiting").await;
    s.wait_for("/service", 10, |v| v["row"] == 2 && v["state"] == "playing").await;
    // Then the cued hymn: ready at its first note, not playing.
    let st = s.wait_for("/service", 10, |v| v["row"] == 3).await;
    assert_eq!(st["state"], "ready");
    let player = s.get("/player").await;
    assert_eq!(player["track_id"], "sample-old-hundredth");
    assert_eq!(player["state"], "stopped");
    assert_eq!(player["position_ms"], 850, "cued 150 ms before the first note");

    // Next on the last row finishes the service.
    let st = s.post("/service/next", json!(null)).await;
    assert_eq!(st["state"], "finished");
    let st = s.post("/service/previous", json!(null)).await;
    assert_eq!(st["row"], 2);
    s.post("/player/stop", json!(null)).await;

    let st = s.post("/service/select", json!({ "row": 3 })).await;
    assert_eq!(st["row"], 3);
    assert_eq!(st["state"], "ready");
    let (code, _) = s.request(Method::POST, "/service/select", Some(json!({ "row": 0 }))).await;
    assert_eq!(code, 409, "a text row can't be played");

    let st = s.post("/service/end", json!(null)).await;
    assert_eq!(st["state"], "idle");
}

#[tokio::test]
async fn a_service_needs_an_organ() {
    let s = TestServer::start().await;
    s.post("/playlists", json!({ "name": "Matins" })).await;
    let (code, body) = s
        .request(Method::POST, "/service/start", Some(json!({ "playlist_id": "matins" })))
        .await;
    assert_eq!(code, 409, "{body}");
    let (code, _) = s.request(Method::POST, "/service/next", None).await;
    assert_eq!(code, 409);
}
