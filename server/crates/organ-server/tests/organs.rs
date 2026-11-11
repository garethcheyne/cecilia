//! Making and editing organ definitions.

mod common;

use common::*;
use reqwest::Method;
use serde_json::json;

#[tokio::test]
async fn copy_edit_use_and_delete_an_organ() {
    let s = TestServer::with_simulator().await;
    let o = s.post("/organs/copy", json!({ "from": "makin-2m", "name": "St Mary's Makin" })).await;
    assert_eq!(o["id"], "st-mary-s-makin");
    let id = "st-mary-s-makin";

    // Built-ins are read-only.
    let (code, _) = s.request(Method::PUT, "/organs/makin-2m/save", Some(s.get("/organs/makin-2m").await)).await;
    assert_eq!(code, 409);

    // A bad edit is refused with the reason.
    let mut bad = o.clone();
    bad["divisions"][0]["channel"] = json!(99);
    let (code, body) = s.request(Method::PUT, &format!("/organs/{id}/save"), Some(bad)).await;
    assert_eq!(code, 400);
    assert!(body["error"].as_str().unwrap().contains("channel"), "{body}");

    // Use it, then relearn the Open Diapason as program change 0x40.
    s.put("/simulator/organ", json!({ "organ_id": id })).await;
    let mut edited = o.clone();
    let stops = edited["stops"].as_array_mut().unwrap();
    let od = stops.iter_mut().find(|st| st["id"] == "gt-opendiap").unwrap();
    od["midi"] = json!([{ "type": "toggle", "msg": "CB 40" }]);
    s.put(&format!("/organs/{id}/save"), edited).await;
    s.post("/midi/test/record", json!(null)).await;
    s.draw("gt-opendiap", true).await;
    let status = s.post("/midi/test/stop", json!(null)).await;
    assert_eq!(status["test"]["events"], 1, "the simulator now sends the learned message");
    assert!(s.get("/organs").await.as_array().unwrap().iter().any(|o| o["id"] == id && o["builtin"] == false));

    // Deleting the organ in use falls back to the demo organ.
    assert_eq!(s.request(Method::POST, &format!("/organs/{id}/delete"), None).await.0, 204);
    assert_eq!(s.sim().await["organ_id"], "demo");
    assert_eq!(s.request(Method::POST, "/organs/demo/delete", None).await.0, 409);
}
