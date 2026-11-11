//! Importing and exporting files: MIDI, MusicXML, CSV and the MIDI backup.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use organ_core::musicxml::{self, XmlAnalysis, XmlImportOptions};
use organ_core::smf::{self, ExportFormat, ImportOptions, MidiAnalysis};
use organ_core::track::{Track, TrackSummary};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::api::{ApiErrorBody, AppState};
use crate::services::AppEvent;

/// Largest file accepted for import.
pub const MAX_IMPORT_BYTES: usize = 20 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/import/analyse", post(analyse))
        .route("/import", post(import))
        .route("/library/{id}/midi", get(export_midi))
        .route("/export/library.csv", get(export_csv))
        .route("/export/library-midi.zip", get(export_zip))
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Midi,
    Musicxml,
}

fn kind_of(name: &str, bytes: &[u8]) -> Option<FileKind> {
    let lower = name.to_lowercase();
    if bytes.starts_with(b"MThd") || lower.ends_with(".mid") || lower.ends_with(".midi") {
        Some(FileKind::Midi)
    } else if lower.ends_with(".musicxml") || lower.ends_with(".xml") || lower.ends_with(".mxl") || bytes.starts_with(b"PK") || bytes.starts_with(b"<?xml") {
        Some(FileKind::Musicxml)
    } else {
        None
    }
}

/// Title from a file name: "abide-with-me.mid" -> "abide with me".
fn title_from(name: &str) -> String {
    let stem = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = stem.rsplit_once('.').map_or(stem, |(s, _)| s);
    let t = stem.replace(['_', '-'], " ").trim().to_string();
    if t.is_empty() { "Imported".into() } else { t }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct FileQuery {
    /// The file's name, used to tell the format and as a default title.
    pub name: String,
    /// JSON import options (`ImportOptions` for MIDI, `XmlImportOptions` for MusicXML).
    #[serde(default)]
    pub options: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportAnalysis {
    pub kind: FileKind,
    pub title: String,
    pub midi: Option<MidiAnalysis>,
    pub musicxml: Option<XmlAnalysis>,
}

/// Look inside a file before importing it: channels or staves, length,
/// and whether it was recorded on the current organ.
#[utoipa::path(post, path = "/api/v1/import/analyse", tag = "files", params(FileQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, body = ImportAnalysis), (status = 400, body = ApiErrorBody)))]
pub async fn analyse(State(s): State<AppState>, Query(q): Query<FileQuery>, body: Bytes) -> Result<Json<ImportAnalysis>, Response> {
    let bad = |e: String| error(StatusCode::BAD_REQUEST, e);
    let kind = kind_of(&q.name, &body).ok_or_else(|| bad("Use a MIDI (.mid) or MusicXML (.musicxml, .mxl) file.".into()))?;
    let title = title_from(&q.name);
    Ok(Json(match kind {
        FileKind::Midi => ImportAnalysis {
            kind,
            title,
            midi: Some(smf::analyse(&body, &s.live.organ()).map_err(|e| bad(e.to_string()))?),
            musicxml: None,
        },
        FileKind::Musicxml => {
            let xml = musicxml::read_text(&body).map_err(|e| bad(e.to_string()))?;
            let a = musicxml::analyse(&xml).map_err(|e| bad(e.to_string()))?;
            ImportAnalysis { kind, title: if a.title.is_empty() { title } else { a.title.clone() }, midi: None, musicxml: Some(a) }
        }
    }))
}

/// Import a file into the library. MIDI files may become several pieces.
#[utoipa::path(post, path = "/api/v1/import", tag = "files", params(FileQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, body = Vec<TrackSummary>), (status = 400, body = ApiErrorBody)))]
pub async fn import(State(s): State<AppState>, Query(q): Query<FileQuery>, body: Bytes) -> Result<Json<Vec<TrackSummary>>, Response> {
    let bad = |e: String| error(StatusCode::BAD_REQUEST, e);
    let kind = kind_of(&q.name, &body).ok_or_else(|| bad("Use a MIDI (.mid) or MusicXML (.musicxml, .mxl) file.".into()))?;
    let title = title_from(&q.name);
    let options = q.options.as_deref().unwrap_or("{}");
    let tracks: Vec<Track> = match kind {
        FileKind::Midi => {
            let opts: ImportOptions = serde_json::from_str(options).map_err(|e| bad(format!("bad options: {e}")))?;
            smf::import(&body, &s.live.organ(), &opts, &title).map_err(|e| bad(e.to_string()))?
        }
        FileKind::Musicxml => {
            let opts: XmlImportOptions = serde_json::from_str(options).map_err(|e| bad(format!("bad options: {e}")))?;
            let xml = musicxml::read_text(&body).map_err(|e| bad(e.to_string()))?;
            vec![musicxml::import(&xml, &opts, &title).map_err(|e| bad(e.to_string()))?]
        }
    };
    let mut saved = Vec::new();
    for mut t in tracks {
        t.id = s.library.unique_id(&t.info.title);
        saved.push(s.library.save(&t).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?);
    }
    let _ = s.app_events.send(AppEvent::Library { saved: None });
    Ok(Json(saved))
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ExportQuery {
    /// `organ` (default): the current organ's MIDI. `general_midi`: one channel per division.
    #[serde(default)]
    pub format: Option<ExportFormat>,
}

fn download(bytes: Vec<u8>, mime: &'static str, name: &str) -> Response {
    let disposition = format!("attachment; filename=\"{}\"", name.replace('"', "'"));
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
            (header::CONTENT_DISPOSITION, HeaderValue::from_str(&disposition).unwrap_or(HeaderValue::from_static("attachment"))),
        ],
        bytes,
    )
        .into_response()
}

/// Download a track as a MIDI file.
#[utoipa::path(get, path = "/api/v1/library/{id}/midi", tag = "files", params(("id" = String, Path, description = "Track id"), ExportQuery),
    responses((status = 200, content_type = "audio/midi"), (status = 404, body = ApiErrorBody)))]
pub async fn export_midi(State(s): State<AppState>, Path(id): Path<String>, Query(q): Query<ExportQuery>) -> Response {
    let Some(t) = s.library.get(&id) else {
        return error(StatusCode::NOT_FOUND, format!("no track called {id:?}"));
    };
    let bytes = smf::export(&t, q.format.unwrap_or(ExportFormat::Organ), &s.live.organ(), &smf::default_channels());
    download(bytes, "audio/midi", &format!("{}.mid", smf::file_name(&t.info.title)))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

/// The library as a spreadsheet (CSV).
#[utoipa::path(get, path = "/api/v1/export/library.csv", tag = "files",
    responses((status = 200, content_type = "text/csv")))]
pub async fn export_csv(State(s): State<AppState>) -> Response {
    let mut out = String::from("Title,Composer,Tune,Performer,Publication,Copyright,Volume,Length (s),Verses,Organ,Source\r\n");
    for t in s.library.list() {
        let i = &t.info;
        let row = [
            csv_field(&i.title),
            csv_field(&i.composer),
            csv_field(&i.melody),
            csv_field(&i.performer),
            csv_field(&i.publication),
            csv_field(&i.copyright),
            i.volume.to_string(),
            (t.duration_ms / 1000).to_string(),
            t.verses.to_string(),
            csv_field(&t.organ_name),
            format!("{:?}", t.source).to_lowercase(),
        ];
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    download(out.into_bytes(), "text/csv; charset=utf-8", "cecilia-library.csv")
}

/// Every track as a MIDI file for the current organ, zipped: a backup that
/// any MIDI program can open.
#[utoipa::path(get, path = "/api/v1/export/library-midi.zip", tag = "files",
    responses((status = 200, content_type = "application/zip")))]
pub async fn export_zip(State(s): State<AppState>) -> Response {
    let tracks: Vec<Track> = s.library.list().iter().filter_map(|t| s.library.get(&t.id)).collect();
    let organ = s.live.organ();
    let bytes = tokio::task::spawn_blocking(move || smf::library_zip(&tracks, &organ)).await.unwrap_or_default();
    download(bytes, "application/zip", "cecilia-library-midi.zip")
}
