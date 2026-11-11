//! Browsing and adding from the shared library (a local copy of it).

mod common;

use common::*;
use organ_server::Config;
use reqwest::Method;
use serde_json::json;

/// A tiny shared library: one hymn and the Makin template.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music/hymns");
    let organs = dir.path().join("organs/makin");
    std::fs::create_dir_all(&music).unwrap();
    std::fs::create_dir_all(&organs).unwrap();
    let text = include_str!("../../organ-core/samples/crimond.score.json");
    let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
    v["licence"] = "public-domain".into();
    v["tags"] = json!(["hymn", "psalm"]);
    std::fs::write(music.join("crimond.score.json"), v.to_string()).unwrap();
    std::fs::write(organs.join("makin-2m.organ.json"), include_str!("../../organ-core/organs/makin-2m.organ.json"))
        .unwrap();
    std::fs::write(
        organs.join("makin-2m.meta.json"),
        r#"{"maker":"Makin","model":"Westmorland","confidence":"learned","notes":["Set MIDI to mode B"],"licence":"CC0-1.0"}"#,
    )
    .unwrap();
    dir
}

#[tokio::test]
async fn browse_and_add_from_the_shared_library() {
    let lib = fixture();
    let data = tempfile::tempdir().unwrap();
    let s = TestServer::start_with(Config {
        library_url: lib.path().to_string_lossy().into(),
        ..Config::for_tests(data.path().to_path_buf())
    })
    .await;

    let cat = s.get("/community").await;
    assert_eq!(cat["music"][0]["id"], "crimond");
    assert_eq!(cat["organs"][0]["meta"]["confidence"], "learned");

    let t = s.post("/community/music/crimond/add", json!(null)).await;
    assert_eq!(t["id"], "lib-crimond");
    assert_eq!(t["source"], "imported");
    let again = s.post("/community/music/crimond/add", json!(null)).await;
    assert_eq!(again["id"], "lib-crimond", "adding twice doesn't make a copy");
    assert!(s.get("/library").await.as_array().unwrap().iter().any(|t| t["id"] == "lib-crimond"));

    let preview = s.get("/community/organs/makin-2m").await;
    assert!(preview["stops"].as_array().unwrap().len() > 10, "the full stop list, to check before adding");

    let o = s.post("/community/organs/makin-2m/add", json!(null)).await;
    assert_eq!(o["name"], "Makin Westmorland");
    assert_eq!(o["builtin"], false, "an organ you can edit");
    let organ = s.get(&format!("/organs/{}", o["id"].as_str().unwrap())).await;
    assert!(organ["connection"]["notes"].as_array().unwrap().iter().any(|n| n == "Set MIDI to mode B"));

    let (code, _) = s.request(Method::POST, "/community/music/nope/add", None).await;
    assert_eq!(code, 404);
}

#[tokio::test]
async fn no_library_is_a_clear_error() {
    let data = tempfile::tempdir().unwrap();
    let s = TestServer::start_with(Config {
        library_url: data.path().join("missing").to_string_lossy().into(),
        ..Config::for_tests(data.path().to_path_buf())
    })
    .await;
    // An empty folder is an empty library, not a crash.
    let cat = s.get("/community").await;
    assert_eq!(cat["music"].as_array().unwrap().len(), 0);
}
