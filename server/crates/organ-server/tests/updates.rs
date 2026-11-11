//! Updating, against a pretend GitHub that lists releases.

mod common;

use axum::routing::get;
use axum::{Json, Router};
use common::TestServer;
use reqwest::Method;
use serde_json::{json, Value};

/// A local server answering like GitHub's releases list.
async fn fake_github() -> String {
    let releases = json!([
        {
            "tag_name": "v99.1.0-beta.1", "name": "Preview", "body": "Trying things", "draft": false,
            "prerelease": true, "assets": [{ "name": "cecilia-windows-x64.zip", "browser_download_url": "http://x/a.zip" }]
        },
        {
            "tag_name": "v99.0.0", "name": "Cecilia 99", "body": "Lots of new things", "draft": false,
            "prerelease": false, "published_at": "2026-12-01T00:00:00Z",
            "assets": [{ "name": "cecilia-windows-x64.zip", "browser_download_url": "http://x/b.zip" }]
        },
        { "tag_name": "v100.0.0", "name": "Not finished", "draft": true, "prerelease": false, "assets": [] }
    ]);
    let app = Router::new().route("/releases", get(move || async move { Json(releases.clone()) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/releases")
}

#[tokio::test]
async fn finds_a_new_version_and_follows_the_channel() {
    // A test binary of its own, so these settings don't leak into other tests.
    std::env::set_var("CECILIA_UPDATE_URL", fake_github().await);
    std::env::set_var("CECILIA_UPDATE_KIND", "development");
    let s = TestServer::start().await;

    let before = s.get("/updates").await;
    assert_eq!(before["current"], env!("CARGO_PKG_VERSION"));
    assert_eq!(before["available"], false, "nothing known until it checks");
    assert_eq!(before["settings"]["auto_install"], true, "on by default");
    assert_eq!(before["settings"]["channel"], "stable");

    let checked = s.post("/updates/check", json!(null)).await;
    assert_eq!(checked["latest"]["version"], "99.0.0", "drafts and previews are left out");
    assert_eq!(checked["latest"]["notes"], "Lots of new things");
    assert_eq!(checked["available"], true);
    assert!(checked["last_checked"].as_u64().is_some());
    // A development build says how to update rather than offering to.
    assert!(checked["blocked"].as_str().unwrap().contains("development build"));
    let (code, body) = s.request(Method::POST, "/updates/install", Some(json!(null))).await;
    assert_eq!(code, 409);
    assert!(body["error"].as_str().unwrap().contains("development build"));

    let preview = s.put("/updates/settings", json!({ "channel": "preview", "auto_install": false })).await;
    assert_eq!(preview["latest"]["version"], "99.1.0-beta.1");
    assert_eq!(preview["settings"]["auto_install"], false);

    // Settings survive a restart.
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(s.dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(saved["updates"]["channel"], "preview");

    let (code, _) = s.request(Method::POST, "/updates/rollback", Some(json!(null))).await;
    assert_eq!(code, 409, "nothing to go back to");
}
