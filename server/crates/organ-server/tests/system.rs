//! Settings, backup and restore, the password, and scanned music.

mod common;

use common::*;
use organ_server::Config;
use reqwest::Method;
use serde_json::json;

#[tokio::test]
async fn settings_apply_and_stay_in_range() {
    let s = TestServer::start().await;
    let st = s.get("/settings").await;
    assert_eq!(st["recording"]["silence_ms"], 8000);
    let out = s
        .put("/settings", json!({ "recording": { "silence_ms": 500, "margin_start_ms": 1000, "margin_end_ms": 99000 }, "playback": { "fade_ms": 8000, "settle_ms": 300 } }))
        .await;
    assert_eq!(out["recording"]["silence_ms"], 2000, "at least 2 s");
    assert_eq!(out["recording"]["margin_end_ms"], 15000, "at most 15 s");
    assert_eq!(s.get("/settings").await["playback"]["fade_ms"], 8000);
}

#[tokio::test]
async fn a_backup_restores_onto_another_machine() {
    let a = TestServer::start().await;
    let mut p = a.post("/playlists", json!({ "name": "Easter Day" })).await;
    p["rows"] = json!([{ "type": "track", "track_id": "sample-crimond" }]);
    a.put("/playlists/easter-day", p).await;
    a.patch("/library/sample-crimond", json!({ "performer": "Dad" })).await;
    let zip = a.http.get(a.url("/export/backup.zip")).send().await.unwrap().bytes().await.unwrap();
    assert_eq!(&zip[..2], b"PK");

    let b = TestServer::start().await;
    let r = b.http.post(b.url("/import/backup")).body(zip.to_vec()).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let counts: serde_json::Value = r.json().await.unwrap();
    assert!(counts["tracks"].as_u64().unwrap() >= 11);
    assert_eq!(counts["playlists"], 1);
    assert_eq!(b.get("/playlists/easter-day").await["name"], "Easter Day");
    assert_eq!(b.get("/library/sample-crimond").await["info"]["performer"], "Dad");

    let r = b.http.post(b.url("/import/backup")).body(b"not a zip".to_vec()).send().await.unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn a_password_protects_the_api() {
    let s = TestServer::start().await;
    assert_eq!(s.get("/auth").await["required"], false);
    let (code, _) = s.request(Method::PUT, "/auth/password", Some(json!({ "password": "abc" }))).await;
    assert_eq!(code, 400, "too short");
    s.put("/auth/password", json!({ "password": "magnificat" })).await;
    // This client stays signed in; a new one is locked out.
    assert!(!s.get("/library").await.as_array().unwrap().is_empty());
    let stranger = reqwest::Client::new();
    assert_eq!(stranger.get(s.url("/library")).send().await.unwrap().status(), 401);
    assert_eq!(stranger.get(s.url("/health")).send().await.unwrap().status(), 200, "health stays open");

    let other = reqwest::Client::builder().cookie_store(true).build().unwrap();
    let bad = other.post(s.url("/auth/login")).json(&json!({ "password": "nunc dimittis" })).send().await.unwrap();
    assert_eq!(bad.status(), 401);
    let good = other.post(s.url("/auth/login")).json(&json!({ "password": "magnificat" })).send().await.unwrap();
    assert_eq!(good.status(), 204);
    assert_eq!(other.get(s.url("/library")).send().await.unwrap().status(), 200);

    s.put("/auth/password", json!({ "password": null })).await;
    assert_eq!(stranger.get(s.url("/library")).send().await.unwrap().status(), 200, "removed again");
}

#[tokio::test]
async fn scanned_music_goes_through_the_omr_engine() {
    let s = TestServer::start().await;
    let (code, _) = s.request(Method::POST, "/omr?name=hymn.png", None).await;
    assert_eq!(code, 501, "no engine installed");

    // A stand-in engine that "reads" a scan by copying a MusicXML file.
    let dir = tempfile::tempdir().unwrap();
    let fixture = dir.path().join("fixture.musicxml");
    std::fs::write(&fixture, r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Organ</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration></note></measure></part></score-partwise>"#).unwrap();
    let command = if cfg!(windows) {
        format!("cmd /C copy /Y {} {{output}}\\scan.musicxml", fixture.display())
    } else {
        format!("cp {} {{output}}/scan.musicxml", fixture.display())
    };
    let data = tempfile::tempdir().unwrap();
    let s =
        TestServer::start_with(Config { omr_command: Some(command), ..Config::for_tests(data.path().to_path_buf()) })
            .await;
    let r = s.http.post(format!("{}?name=hymn.png", s.url("/omr"))).body(vec![0u8; 16]).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let xml = r.text().await.unwrap();
    assert!(xml.contains("score-partwise"));
    let (code, _) = s.request(Method::POST, "/omr?name=hymn.docx", None).await;
    assert_eq!(code, 400);
}

#[test]
fn every_api_operation_has_its_own_name() {
    // The web app's types are keyed by these names: two the same and one
    // endpoint's types silently replace the other's.
    use utoipa::OpenApi;
    let doc = serde_json::to_value(organ_server::api::ApiDoc::openapi()).unwrap();
    let mut seen = std::collections::HashSet::new();
    for item in doc["paths"].as_object().unwrap().values() {
        for op in item.as_object().unwrap().values() {
            if let Some(id) = op["operationId"].as_str() {
                assert!(seen.insert(id.to_string()), "two API operations are both called {id}");
            }
        }
    }
    assert!(seen.len() > 50);
}
