//! Recording into the library and playing back, end to end through the
//! simulator, as the web UI and an organist would use them.

mod common;

use common::*;
use reqwest::Method;
use serde_json::{json, Value};

fn ids(list: &Value) -> Vec<String> {
    list.as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn sample_music_is_in_a_new_library() {
    let s = TestServer::start().await;
    let list = s.get("/library").await;
    assert!(ids(&list).contains(&"sample-old-hundredth".to_string()), "{list}");
    let t = s.get("/library/sample-old-hundredth").await;
    assert_eq!(t["info"]["melody"], "OLD 100TH");
    assert_eq!(t["verses"].as_array().unwrap().len(), 3, "four verses");
    assert!(t["meter"]["bpm"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn deleted_samples_stay_deleted_after_a_restart() {
    let data = tempfile::tempdir().unwrap();
    let first = TestServer::start_at(data.path().to_path_buf()).await;
    assert_eq!(first.delete("/library/sample-old-hundredth").await, 204);
    assert_eq!(first.delete("/library/sample-old-hundredth").await, 404);
    // A second server on the same data folder, as after a restart.
    let again = TestServer::start_at(data.path().to_path_buf()).await;
    assert!(!ids(&again.get("/library").await).contains(&"sample-old-hundredth".to_string()));
}

#[tokio::test]
async fn recording_needs_an_input_and_playing_needs_an_output() {
    let s = TestServer::start().await;
    let (code, body) = s.request(Method::POST, "/recorder/start", None).await;
    assert_eq!(code, 409, "{body}");
    let (code, body) = s
        .request(Method::POST, "/player/load", Some(json!({ "track_id": "sample-old-hundredth" })))
        .await;
    assert_eq!(code, 409, "{body}");
    let (code, _) = s.request(Method::POST, "/player/load", Some(json!({ "track_id": "nope" }))).await;
    assert_eq!(code, 404);
}

#[tokio::test]
async fn a_piece_played_on_the_simulator_is_recorded_and_plays_back() {
    let s = TestServer::with_simulator().await;
    s.draw("gt-opendiap", true).await;
    let r = s.post("/recorder/start", json!(null)).await;
    assert_eq!(r["state"], "waiting");

    s.draw("gt-principal", true).await;
    for key in [60u8, 64, 67] {
        s.note("great", key, 90).await;
        sleep_ms(40).await;
        s.note("great", key, 0).await;
    }
    s.draw("gt-trumpet", true).await;
    s.note("pedal", 36, 90).await;
    sleep_ms(40).await;
    s.note("pedal", 36, 0).await;
    sleep_ms(50).await;
    assert_eq!(s.get("/recorder").await["state"], "recording");
    let r = s.post("/recorder/stop", json!(null)).await;
    assert_eq!(r["state"], "off");
    assert_eq!(r["saved"], 1);

    let list = s.get("/library").await;
    let rec = list.as_array().unwrap().iter().find(|t| t["source"] == "recorded").expect("a recording");
    assert_eq!(rec["notes"], 4);
    let id = rec["id"].as_str().unwrap().to_string();
    let track = s.get(&format!("/library/{id}")).await;
    assert_eq!(track["initial"]["stops"].as_array().unwrap().len(), 2, "Open Diapason and Principal");
    assert!(track["events"].as_array().unwrap().iter().any(|e| e["type"] == "stop" && e["on"] == true));

    // Clear the console and play it back: the simulator ends up as it was.
    s.post("/simulator/reset", json!(null)).await;
    let p = s.post("/player/load", json!({ "track_id": id })).await;
    assert_eq!(p["track_id"], id);
    s.wait_for("/player", 10, |p| p["state"] == "stopped").await;
    assert_eq!(stops(&s.sim().await), ["gt-opendiap", "gt-principal", "gt-trumpet"]);
}

#[tokio::test]
async fn a_hymn_plays_with_verse_control_and_tempo() {
    let s = TestServer::with_simulator().await;
    // One verse, very fast: enough to hear it all and finish quickly.
    let p = s
        .post(
            "/player/load",
            json!({ "track_id": "sample-old-hundredth", "options": { "verses": [4], "tempo": 400 } }),
        )
        .await;
    assert_eq!(p["verses"], 1);
    assert_eq!(p["plan"], json!([4]));
    s.wait_for("/player", 5, |p| p["state"] == "playing").await;
    // Verse 4 adds the mixture: the simulator shows it.
    s.wait_for("/simulator", 5, |sim| stops(sim).contains(&"gt-mixture".to_string())).await;
    s.wait_for("/player", 30, |p| p["state"] == "stopped").await;
    let sim = s.sim().await;
    assert!(sim["console"]["notes"].as_object().unwrap().is_empty(), "no stuck notes");
}

#[tokio::test]
async fn a_saved_speed_is_used_unless_another_is_asked_for() {
    let s = TestServer::with_simulator().await;
    s.patch("/library/sample-crimond", json!({ "tempo": 90 })).await;
    let p = s.post("/player/load", json!({ "track_id": "sample-crimond", "play": false })).await;
    assert_eq!(p["tempo"], 90);
    let p = s
        .post("/player/load", json!({ "track_id": "sample-crimond", "play": false, "options": { "tempo": 110 } }))
        .await;
    assert_eq!(p["tempo"], 110, "a playlist's tempo wins for that performance");
}

#[tokio::test]
async fn transport_pause_seek_tempo_and_stop() {
    let s = TestServer::with_simulator().await;
    s.post("/player/load", json!({ "track_id": "sample-old-hundredth" })).await;
    s.wait_for("/player", 5, |p| p["state"] == "playing" && p["position_ms"].as_u64().unwrap() > 1500).await;

    let p = s.post("/player/pause", json!(null)).await;
    assert_eq!(p["state"], "paused");
    let pos = p["position_ms"].as_u64().unwrap();
    sleep_ms(200).await;
    assert_eq!(s.get("/player").await["position_ms"].as_u64().unwrap(), pos, "paused means paused");
    assert!(s.sim().await["console"]["notes"].as_object().unwrap().is_empty(), "keys released");

    let p = s.post("/player/seek", json!({ "ms": 40_000 })).await;
    assert!(p["position_ms"].as_u64().unwrap().abs_diff(40_000) < 100);
    assert_eq!(p["verse"], 2, "40 s is in the second verse (each is about 31 s)");

    let p = s.post("/player/tempo", json!({ "percent": 150 })).await;
    assert_eq!(p["tempo"], 150);
    let p = s.post("/player/play", json!(null)).await;
    assert_eq!(p["state"], "playing");

    let p = s.post("/player/verses", json!({ "skip_to_last": true })).await;
    assert_eq!(p["skip_to_last"], true);

    let p = s.post("/player/stop", json!(null)).await;
    assert_eq!(p["state"], "stopped");
    assert!(s.sim().await["console"]["notes"].as_object().unwrap().is_empty());
}

#[tokio::test]
async fn library_edits() {
    let s = TestServer::start().await;
    let id = "sample-old-hundredth";
    let t = s
        .patch(&format!("/library/{id}"), json!({ "title": "  Old 100th  ", "volume": 80, "performer": "Dad" }))
        .await;
    assert_eq!(t["info"]["title"], "Old 100th");
    assert_eq!(t["info"]["volume"], 80);
    let (code, _) = s.request(Method::PATCH, &format!("/library/{id}"), Some(json!({ "volume": 0 }))).await;
    assert_eq!(code, 400, "volume is 1-100");
    let t = s.patch(&format!("/library/{id}"), json!({ "tempo": 90 })).await;
    assert_eq!(t["info"]["tempo"], 90, "a saved speed");
    let (code, _) = s.request(Method::PATCH, &format!("/library/{id}"), Some(json!({ "title": " " }))).await;
    assert_eq!(code, 400, "a title is required");

    let copy = s.post(&format!("/library/{id}/duplicate"), json!(null)).await;
    assert_ne!(copy["id"], id);
    assert!(copy["info"]["title"].as_str().unwrap().ends_with("(copy)"));

    // Full save round-trip, as the editor does.
    let mut track = s.get(&format!("/library/{id}")).await;
    track["info"]["composer"] = json!("Bourgeois");
    let saved = s.put(&format!("/library/{id}"), track.clone()).await;
    assert_eq!(saved["info"]["composer"], "Bourgeois");
    let (code, _) = s.request(Method::PUT, "/library/other-id", Some(track)).await;
    assert_eq!(code, 400, "id must match");

    // The history folder keeps old versions.
    let history = s.dir.join("history").join(id);
    assert!(std::fs::read_dir(history).unwrap().count() >= 2);
}

#[tokio::test]
async fn the_virtual_organist_explains_its_choices() {
    let s = TestServer::start().await;
    s.put("/simulator/organ", json!({ "organ_id": "makin-2m" })).await;
    let m = s.get("/library/sample-old-hundredth/mapping").await;
    let rows = m.as_array().unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r["recorded"]["division"].is_string()));
    assert!(rows.iter().filter(|r| r["stop"].is_string()).count() * 2 > rows.len());
}
