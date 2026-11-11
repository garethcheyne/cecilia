//! Importing and exporting files over HTTP.

mod common;

use common::*;
use serde_json::{json, Value};

async fn bytes(s: &TestServer, path: &str) -> (String, Vec<u8>) {
    let r = s.http.get(s.url(path)).send().await.unwrap();
    assert!(r.status().is_success(), "GET {path}: {}", r.status());
    let ct = r.headers()["content-type"].to_str().unwrap().to_string();
    (ct, r.bytes().await.unwrap().to_vec())
}

async fn upload(s: &TestServer, path: &str, name: &str, options: Option<Value>, body: Vec<u8>) -> (u16, Value) {
    let mut url = format!("{}?name={}", s.url(path), name);
    if let Some(o) = options {
        url.push_str("&options=");
        url.push_str(&urlencode(&o.to_string()));
    }
    let r = s.http.post(url).body(body).send().await.unwrap();
    let code = r.status().as_u16();
    (code, r.json().await.unwrap_or(Value::Null))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[tokio::test]
async fn organ_midi_round_trips_through_the_library() {
    let s = TestServer::start().await;
    let (ct, mid) = bytes(&s, "/library/sample-old-hundredth/midi").await;
    assert_eq!(ct, "audio/midi");
    assert_eq!(&mid[..4], b"MThd");

    let (code, a) = upload(&s, "/import/analyse", "old-100th.mid", None, mid.clone()).await;
    assert_eq!(code, 200, "{a}");
    assert_eq!(a["kind"], "midi");
    assert!(a["midi"]["organ_match"].as_f64().unwrap() > 0.9, "{a}");

    let (code, saved) = upload(&s, "/import", "old-100th.mid", Some(json!({ "organ_format": true })), mid).await;
    assert_eq!(code, 200, "{saved}");
    let t = &saved[0];
    assert_eq!(t["info"]["title"], "old 100th");
    assert_eq!(t["source"], "imported");
    let original = s.get("/library/sample-old-hundredth").await;
    let notes = |t: &Value| {
        t["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == "note" && e["velocity"].as_u64() > Some(0))
            .count()
    };
    let back = s.get(&format!("/library/{}", t["id"].as_str().unwrap())).await;
    assert_eq!(notes(&back), notes(&original));
}

#[tokio::test]
async fn general_midi_and_bad_files() {
    let s = TestServer::start().await;
    let (_, mid) = bytes(&s, "/library/sample-old-hundredth/midi?format=general_midi").await;
    let (_, a) = upload(&s, "/import/analyse", "hymn.mid", None, mid).await;
    let chans: Vec<u64> =
        a["midi"]["channels"].as_array().unwrap().iter().map(|c| c["channel"].as_u64().unwrap()).collect();
    assert_eq!(chans, [1, 3]);

    let (code, body) = upload(&s, "/import/analyse", "photo.jpg", None, b"\xFF\xD8\xFF".to_vec()).await;
    assert_eq!(code, 400, "{body}");
    let (code, _) = upload(&s, "/import", "broken.mid", None, b"MThd nonsense".to_vec()).await;
    assert_eq!(code, 400);
}

#[tokio::test]
async fn musicxml_imports_as_a_notated_track() {
    let s = TestServer::start().await;
    let xml = r#"<?xml version="1.0"?><score-partwise version="4.0">
      <work><work-title>Little Tune</work-title></work>
      <part-list><score-part id="P1"><part-name>Organ</part-name></score-part></part-list>
      <part id="P1"><measure number="1">
        <attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
        <direction><sound tempo="72"/></direction>
        <note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration></note>
        <note><pitch><step>E</step><octave>5</octave></pitch><duration>1</duration></note>
        <note><pitch><step>G</step><octave>5</octave></pitch><duration>1</duration></note>
      </measure></part></score-partwise>"#;
    let (code, a) = upload(&s, "/import/analyse", "tune.musicxml", None, xml.as_bytes().to_vec()).await;
    assert_eq!(code, 200, "{a}");
    assert_eq!(a["title"], "Little Tune");
    assert_eq!(a["musicxml"]["beats"], 3);
    let (code, saved) =
        upload(&s, "/import", "tune.musicxml", Some(json!({ "verses": 3 })), xml.as_bytes().to_vec()).await;
    assert_eq!(code, 200, "{saved}");
    assert_eq!(saved[0]["verses"], 3);
    let t = s.get(&format!("/library/{}", saved[0]["id"].as_str().unwrap())).await;
    assert_eq!(t["meter"]["bpm"], 72.0);
    assert_eq!(t["meter"]["beats"], 3);
}

#[tokio::test]
async fn library_exports() {
    let s = TestServer::start().await;
    let (ct, csv) = bytes(&s, "/export/library.csv").await;
    assert!(ct.starts_with("text/csv"));
    let csv = String::from_utf8(csv).unwrap();
    assert!(csv.lines().next().unwrap().starts_with("Title,Composer"));
    assert!(csv.contains("All People That on Earth Do Dwell"));

    let (ct, zip) = bytes(&s, "/export/library-midi.zip").await;
    assert_eq!(ct, "application/zip");
    assert_eq!(&zip[..2], b"PK");
}
