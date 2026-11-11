//! Model Context Protocol server, so an AI assistant (and, later, voice
//! control) can run the organ: "draw the Open Diapason", "swell half open",
//! "record this", "stop".
//!
//! Transport: MCP Streamable HTTP at `POST /mcp`, answering every request
//! with a single JSON response (no server-initiated SSE stream, so `GET`
//! returns 405). The server is stateless: no `Mcp-Session-Id` is issued.
//!
//! Tools act on the organ connected to the MIDI **output** (which may be the
//! built-in simulator) using the current organ definition, and read its state
//! from [`LiveOrgan`](organ_core::live::LiveOrgan). Replies are short plain
//! sentences, because they may be read aloud.

use std::collections::BTreeSet;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use organ_core::driver::OrganEvent;
use organ_core::organ::{Equivalent, OrganDefinition, Stop};
use organ_core::MidiError;
use serde_json::{json, Value};

use crate::api::{origin_allowed, AppState};

/// Newest first; we answer with the client's version when we support it.
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

pub const DEFAULT_ASSISTANT_NAME: &str = "Cecilia";

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

pub async fn post(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if let Some(denied) = guard(&s, &headers) {
        return denied;
    }
    let msg: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return rpc_error(Value::Null, -32700, "Parse error"),
    };
    if msg.is_array() {
        return rpc_error(Value::Null, -32600, "JSON-RPC batches are not supported");
    }
    let (Some(id), Some(method)) = (msg.get("id").cloned(), msg.get("method").and_then(Value::as_str)) else {
        // A notification (e.g. notifications/initialized) or a response.
        return StatusCode::ACCEPTED.into_response();
    };
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    match dispatch(&s, method, &params).await {
        Ok(result) => Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response(),
        Err((code, message)) => rpc_error(id, code, &message),
    }
}

/// We don't offer a server-to-client event stream.
pub async fn get(State(s): State<AppState>, headers: HeaderMap) -> Response {
    guard(&s, &headers).unwrap_or_else(|| StatusCode::METHOD_NOT_ALLOWED.into_response())
}

/// Reject other web pages (DNS-rebinding protection, required by the MCP
/// spec) and, when a token is configured, unauthenticated callers.
fn guard(s: &AppState, headers: &HeaderMap) -> Option<Response> {
    if !origin_allowed(headers, &s.allowed_origins) {
        return Some(StatusCode::FORBIDDEN.into_response());
    }
    if let Some(token) = &s.mcp_token {
        let given = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if !constant_time_eq(given.as_bytes(), token.as_bytes()) {
            return Some(
                (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer")]).into_response(),
            );
        }
    }
    None
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn rpc_error(id: Value, code: i32, message: &str) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }))
        .into_response()
}

type RpcResult = Result<Value, (i32, String)>;

async fn dispatch(s: &AppState, method: &str, params: &Value) -> RpcResult {
    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked
                .filter(|v| PROTOCOL_VERSIONS.contains(v))
                .unwrap_or(PROTOCOL_VERSIONS[0]);
            let name = assistant_name(s);
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "cecilia",
                    "title": format!("{name}, the organ assistant"),
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": instructions(&name),
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_list() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            tracing::info!(tool = name, %args, "mcp tool call");
            let (text, is_error) = match call_tool(s, name, &args).await {
                Ok(text) => (text, false),
                Err(text) => (text, true),
            };
            Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
        }
        _ => Err((-32601, format!("Method not found: {method}"))),
    }
}

pub fn assistant_name(s: &AppState) -> String {
    s.settings
        .get()
        .assistant_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_ASSISTANT_NAME.into())
}

fn instructions(name: &str) -> String {
    format!(
        "You are {name}, the assistant that runs a church organ through Cecilia. \
         Use these tools to draw and push in stops, set the swell pedal, select the organ, \
         and record or play music. Replies may be read aloud during a service, so keep them \
         to a sentence. Only start playback or recording when asked. If a stop name matches \
         several stops, ask which one rather than guessing. 'Stop' on its own means stop \
         recording or playback; 'stop everything' or 'silence' means all_notes_off."
    )
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

fn tool_list() -> Value {
    let none = json!({ "type": "object", "properties": {} });
    let stops = json!({
        "type": "object",
        "properties": {
            "stops": {
                "type": "array",
                "items": { "type": "string" },
                "minItems": 1,
                "description": "Stop names as spoken, e.g. \"Open Diapason\", \"Swell Oboe\", \"principal 4\", \"swell to great\"."
            }
        },
        "required": ["stops"]
    });
    let read_only = json!({ "readOnlyHint": true, "openWorldHint": false });
    let acts = json!({ "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false });
    json!([
        { "name": "organ_status", "title": "Organ status",
          "description": "Which organ is selected and connected, which stops are drawn, the swell pedal, and whether recording or playback is running.",
          "inputSchema": none, "annotations": read_only },
        { "name": "list_stops", "title": "List stops",
          "description": "All stops on the organ, grouped by division, marking the ones that are drawn.",
          "inputSchema": { "type": "object", "properties": {
              "division": { "type": "string", "description": "Only this division, e.g. \"Great\"." } } },
          "annotations": read_only },
        { "name": "draw_stops", "title": "Draw stops",
          "description": "Pull out (switch on) one or more stops. Stops already drawn are left as they are.",
          "inputSchema": stops, "annotations": acts },
        { "name": "push_in_stops", "title": "Push in stops",
          "description": "Push in (switch off) one or more stops.",
          "inputSchema": stops, "annotations": acts },
        { "name": "general_cancel", "title": "General cancel",
          "description": "Push in every stop at once.",
          "inputSchema": none, "annotations": acts },
        { "name": "set_swell", "title": "Set swell pedal",
          "description": "Move an expression (swell) pedal. 0 is closed (quietest), 100 fully open.",
          "inputSchema": { "type": "object", "properties": {
              "percent": { "type": "number", "minimum": 0, "maximum": 100 },
              "division": { "type": "string", "description": "Which pedal, e.g. \"Swell\". Defaults to the first one." } },
            "required": ["percent"] },
          "annotations": acts },
        { "name": "all_notes_off", "title": "Silence",
          "description": "Silence the organ immediately: every note off on every channel. Stops are unchanged.",
          "inputSchema": none, "annotations": acts },
        { "name": "list_organs", "title": "List organs",
          "description": "Organ definitions available on this server.",
          "inputSchema": none, "annotations": read_only },
        { "name": "select_organ", "title": "Select organ",
          "description": "Choose which organ definition is in use (also what the simulator plays).",
          "inputSchema": { "type": "object", "properties": {
              "organ": { "type": "string", "description": "Organ name or id." } },
            "required": ["organ"] },
          "annotations": acts },
        { "name": "start_recording", "title": "Start recording",
          "description": "Start recording everything played on the organ. Replaces the previous recording.",
          "inputSchema": none,
          "annotations": { "readOnlyHint": false, "destructiveHint": true, "openWorldHint": false } },
        { "name": "play_recording", "title": "Play recording",
          "description": "Play the last recording back on the organ.",
          "inputSchema": none, "annotations": acts },
        { "name": "stop", "title": "Stop",
          "description": "Stop recording or playback.",
          "inputSchema": none, "annotations": acts },
    ])
}

type ToolResult = Result<String, String>;

async fn call_tool(s: &AppState, name: &str, args: &Value) -> ToolResult {
    match name {
        "organ_status" => organ_status(s).await,
        "list_stops" => list_stops(s, args.get("division").and_then(Value::as_str)),
        "draw_stops" => set_stops(s, &string_list(args, "stops")?, true).await,
        "push_in_stops" => set_stops(s, &string_list(args, "stops")?, false).await,
        "general_cancel" => {
            send_event(s, &OrganEvent::AllStopsOff).await?;
            Ok("All stops pushed in.".into())
        }
        "set_swell" => set_swell(s, args).await,
        "all_notes_off" => {
            s.simulator.release_keys();
            match s.hub.panic().await {
                Ok(()) | Err(MidiError::NoOutput) => Ok("Silenced.".into()),
                Err(e) => Err(midi_message(e)),
            }
        }
        "list_organs" => Ok(s
            .organs
            .list()
            .iter()
            .map(|o| format!("{} ({} stops)", o.name, o.stops))
            .collect::<Vec<_>>()
            .join("\n")),
        "select_organ" => select_organ(s, args.get("organ").and_then(Value::as_str).unwrap_or_default()),
        "start_recording" => {
            s.hub.test_record().await.map_err(midi_message)?;
            Ok("Recording.".into())
        }
        "play_recording" => {
            let st = s.hub.test_play().await.map_err(midi_message)?;
            Ok(format!("Playing the last recording ({}).", seconds(st.test.duration_ms)))
        }
        "stop" => {
            let before = s.hub.status().await.map_err(midi_message)?;
            let st = s.hub.test_stop().await.map_err(midi_message)?;
            Ok(match state_name(&before.test.state).as_str() {
                "recording" => format!("Recording stopped ({}).", seconds(st.test.duration_ms)),
                "playing" => "Playback stopped.".into(),
                _ => "Nothing was playing or recording.".into(),
            })
        }
        _ => Err(format!("Unknown tool {name:?}.")),
    }
}

fn string_list(args: &Value, key: &str) -> Result<Vec<String>, String> {
    let list: Vec<String> = args
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    if list.is_empty() {
        return Err(format!("Give at least one name in {key:?}."));
    }
    Ok(list)
}

fn midi_message(e: MidiError) -> String {
    match e {
        MidiError::NoOutput => {
            "No organ is connected. Choose an Organ out on MIDI Setup (the simulator counts).".into()
        }
        other => format!("{}.", capitalise(&other.to_string())),
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn seconds(ms: u64) -> String {
    let s = (ms + 500) / 1000;
    match s {
        0..=59 => format!("{s} s"),
        _ => format!("{} min {} s", s / 60, s % 60),
    }
}

fn state_name<T: serde::Serialize>(state: &T) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

async fn send_event(s: &AppState, event: &OrganEvent) -> Result<(), String> {
    for msg in s.live.encode(event) {
        s.hub.send(msg).await.map_err(midi_message)?;
    }
    Ok(())
}

async fn organ_status(s: &AppState) -> ToolResult {
    let organ = s.live.organ();
    let console = s.live.console();
    let st = s.hub.status().await.map_err(midi_message)?;
    let mut lines = vec![format!("Organ: {}.", organ.name)];
    lines.push(match &st.output {
        Some(out) => format!("Playing to {out}."),
        None => "No organ output selected.".into(),
    });
    if !st.inputs.is_empty() {
        lines.push(format!("Listening to {}.", st.inputs.join(", ")));
    }
    let drawn: Vec<String> = organ
        .stops
        .iter()
        .filter(|st| console.is_on(&st.id))
        .map(|st| stop_label(&organ, st))
        .collect();
    lines.push(if drawn.is_empty() {
        "No stops drawn.".into()
    } else {
        format!("Drawn: {}.", drawn.join(", "))
    });
    for (role, value) in &console.expression {
        if let Some(d) = organ.division(*role) {
            lines.push(format!("{} pedal {}% open.", d.name, percent(*value)));
        }
    }
    lines.push(match state_name(&st.test.state).as_str() {
        "recording" => "Recording now.".into(),
        "playing" => "Playing back now.".into(),
        _ if st.test.events > 0 => format!("A recording of {} is ready.", seconds(st.test.duration_ms)),
        _ => "Nothing recorded yet.".into(),
    });
    Ok(lines.join("\n"))
}

fn percent(value: u8) -> u32 {
    (value as u32 * 100 + 63) / 127
}

fn list_stops(s: &AppState, division: Option<&str>) -> ToolResult {
    let organ = s.live.organ();
    let console = s.live.console();
    let divisions: Vec<_> = match division {
        Some(q) => {
            let d = find_division(&organ, q).ok_or_else(|| format!("This organ has no division called {q:?}."))?;
            vec![d]
        }
        None => organ.divisions.iter().collect(),
    };
    Ok(divisions
        .iter()
        .map(|d| {
            let stops: Vec<String> = organ
                .stops
                .iter()
                .filter(|st| st.division == d.role)
                .map(|st| {
                    let name = st.names.join(" ");
                    if console.is_on(&st.id) { format!("{name} (drawn)") } else { name }
                })
                .collect();
            format!("{}: {}", d.name, stops.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

async fn set_stops(s: &AppState, names: &[String], on: bool) -> ToolResult {
    let organ = s.live.organ();
    let mut ids = BTreeSet::new();
    let mut problems = Vec::new();
    for q in names {
        match resolve_stop(&organ, q) {
            Resolved::One(id) => {
                ids.insert(id);
            }
            Resolved::Several(options) => {
                problems.push(format!("\"{q}\" could be {}", or_list(&options)))
            }
            Resolved::None => problems.push(format!("there's no stop called \"{q}\"")),
        }
    }
    // Change nothing unless every name was understood: half a registration
    // is worse than none.
    if !problems.is_empty() {
        return Err(format!("{}.", capitalise(&problems.join("; "))));
    }
    let mut changed = Vec::new();
    let mut unchanged = Vec::new();
    for id in &ids {
        let stop = organ.stop(id).expect("resolved id exists");
        let label = stop_label(&organ, stop);
        if s.live.console().is_on(id) == on {
            unchanged.push(label);
            continue;
        }
        send_event(s, &OrganEvent::Stop { stop: id.clone(), on }).await?;
        changed.push(label);
    }
    let verb = if on { "Drew" } else { "Pushed in" };
    let state = if on { "already drawn" } else { "already in" };
    let mut parts = Vec::new();
    if !changed.is_empty() {
        parts.push(format!("{verb} {}.", and_list(&changed)));
    }
    if !unchanged.is_empty() {
        parts.push(format!("{} {} {state}.", and_list(&unchanged), if unchanged.len() == 1 { "was" } else { "were" }));
    }
    Ok(parts.join(" "))
}

async fn set_swell(s: &AppState, args: &Value) -> ToolResult {
    let organ = s.live.organ();
    let pct = args
        .get("percent")
        .and_then(Value::as_f64)
        .ok_or("Say how far open, from 0 to 100.")?
        .clamp(0.0, 100.0);
    let division = match args.get("division").and_then(Value::as_str) {
        Some(q) => find_division(&organ, q).ok_or_else(|| format!("This organ has no division called {q:?}."))?,
        None => organ
            .divisions
            .iter()
            .find(|d| d.expression.is_some())
            .ok_or("This organ has no swell pedal.")?,
    };
    if division.expression.is_none() {
        return Err(format!("The {} has no swell pedal.", division.name));
    }
    let value = (pct * 127.0 / 100.0).round() as u8;
    send_event(s, &OrganEvent::Expression { division: division.role, value }).await?;
    Ok(format!("{} pedal {}% open.", division.name, pct.round()))
}

fn select_organ(s: &AppState, query: &str) -> ToolResult {
    let summaries = s.organs.list();
    let q = tokens(query);
    let matches: Vec<_> = summaries
        .iter()
        .filter(|o| {
            let have = [tokens(&o.name), tokens(&o.id)].concat();
            !q.is_empty() && q.iter().all(|t| have.iter().any(|h| h == t || (t.len() >= 3 && h.starts_with(t.as_str()))))
        })
        .collect();
    let organ = match matches.as_slice() {
        [one] => s.organs.get(&one.id).ok_or("That organ has just been removed.")?,
        [] => return Err(format!("There's no organ called \"{query}\".")),
        many => {
            let names: Vec<String> = many.iter().map(|o| o.name.clone()).collect();
            return Err(format!("\"{query}\" could be {}.", or_list(&names)));
        }
    };
    s.set_organ(organ.clone()).map_err(|e| e.to_string())?;
    Ok(format!("Now using {}.", organ.name))
}

// ---------------------------------------------------------------------------
// Understanding spoken stop names
// ---------------------------------------------------------------------------

/// How a stop is said aloud: couplers by their own name ("Swell to Great"),
/// everything else with its division ("Great Open Diapason 8").
pub fn stop_label(organ: &OrganDefinition, stop: &Stop) -> String {
    let names = stop.names.join(" ");
    match &stop.equivalent {
        Equivalent::Coupler { .. } => names,
        _ => match organ.division(stop.division) {
            Some(d) => format!("{} {names}", d.name),
            None => names,
        },
    }
}

/// Lower-case words, with spoken numbers as digits and filler words dropped,
/// so "the eight foot open diapason" and "Open Diapason 8′" compare equal.
fn tokens(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '/'))
        .filter(|w| !w.is_empty())
        .filter_map(|w| {
            let w = match w {
                "thirty-two" | "thirtytwo" => "32",
                "sixteen" => "16",
                "eight" => "8",
                "four" => "4",
                "two" => "2",
                "one" => "1",
                "the" | "a" | "stop" | "stops" | "foot" | "feet" | "ft" | "please" => return None,
                w => w,
            };
            Some(w.to_owned())
        })
        .collect()
}

/// Extra words a stop answers to beyond its label.
fn stop_tokens(organ: &OrganDefinition, stop: &Stop) -> Vec<String> {
    let mut t = tokens(&stop.names.join(" "));
    if let Some(full) = &stop.full_name {
        t.extend(tokens(full));
    }
    if let Some(d) = organ.division(stop.division) {
        t.extend(tokens(&d.name));
    }
    t.extend(tokens(&format!("{:?}", stop.division)));
    let aliases: Vec<String> = t
        .iter()
        .filter_map(|w| {
            Some(match w.as_str() {
                "ii" => "2",
                "iii" => "3",
                "iv" => "4",
                "v" => "5",
                "vi" => "6",
                "great" => "gt",
                "swell" => "sw",
                "pedal" => "ped",
                "choir" => "ch",
                "tremulant" | "tremolo" => "trem",
                _ => return None,
            })
        })
        .map(str::to_owned)
        .collect();
    t.extend(aliases);
    t
}

#[derive(Debug, PartialEq)]
enum Resolved {
    One(String),
    Several(Vec<String>),
    None,
}

fn resolve_stop(organ: &OrganDefinition, query: &str) -> Resolved {
    let q = tokens(query);
    if q.is_empty() {
        return Resolved::None;
    }
    // Score: 2 per word that matches exactly, 1 per word that starts a word
    // ("diap" for "diapason"). Every spoken word must match something.
    let mut best = 0;
    let mut hits: Vec<&Stop> = Vec::new();
    for stop in &organ.stops {
        let have = stop_tokens(organ, stop);
        let mut score = 0;
        for t in &q {
            if have.contains(t) {
                score += 2;
            } else if t.len() >= 3 && have.iter().any(|h| h.starts_with(t.as_str())) {
                score += 1;
            } else {
                score = 0;
                break;
            }
        }
        if score == 0 {
            continue;
        }
        if score > best {
            best = score;
            hits.clear();
        }
        if score == best {
            hits.push(stop);
        }
    }
    match hits.as_slice() {
        [] => Resolved::None,
        [one] => Resolved::One(one.id.clone()),
        many => Resolved::Several(many.iter().take(6).map(|st| stop_label(organ, st)).collect()),
    }
}

fn find_division<'a>(organ: &'a OrganDefinition, query: &str) -> Option<&'a organ_core::organ::Division> {
    let q = tokens(query);
    organ.divisions.iter().find(|d| {
        let mut have = tokens(&d.name);
        have.extend(tokens(&format!("{:?}", d.role)));
        !q.is_empty() && q.iter().all(|t| have.iter().any(|h| h.starts_with(t.as_str())))
    })
}

fn and_list(items: &[String]) -> String {
    join_last(items, "and")
}

fn or_list(items: &[String]) -> String {
    join_last(items, "or")
}

fn join_last(items: &[String], word: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {word} {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use organ_core::organ::builtin_organs;

    fn makin() -> OrganDefinition {
        builtin_organs().into_iter().find(|o| o.id == "makin-2m").unwrap()
    }

    #[test]
    fn spoken_stop_names_resolve() {
        let o = makin();
        let one = |q: &str| match resolve_stop(&o, q) {
            Resolved::One(id) => id,
            other => panic!("{q:?} -> {other:?}"),
        };
        assert_eq!(one("open diapason"), "gt-opendiap");
        assert_eq!(one("the Oboe"), "sw-oboe");
        assert_eq!(one("principal four foot"), "gt-principal");
        assert_eq!(one("swell to great"), "gt-sw");
        assert_eq!(one("great to pedal"), "ped-gt");
        assert_eq!(one("mixture four"), "gt-mixture");
        assert_eq!(one("trem"), "sw-tremulant");
        assert_eq!(one("pedal bourdon"), "ped-bourdon");
        assert_eq!(one("celeste"), "sw-celeste");
    }

    #[test]
    fn ambiguous_and_unknown_names_are_reported() {
        let o = makin();
        match resolve_stop(&o, "bourdon") {
            Resolved::Several(opts) => {
                assert_eq!(opts, vec!["Pedal Bourdon 16".to_string(), "Great Bourdon 16".to_string()])
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(resolve_stop(&o, "vox humana"), Resolved::None);
        assert_eq!(resolve_stop(&o, ""), Resolved::None);
    }

    #[test]
    fn lists_read_naturally() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(and_list(&v(&["A"])), "A");
        assert_eq!(and_list(&v(&["A", "B", "C"])), "A, B and C");
        assert_eq!(or_list(&v(&["A", "B"])), "A or B");
    }
}
