//! Cecilia keeps itself up to date.
//!
//! It checks the GitHub releases once in a while, shows what's new, and
//! installs a new version when someone presses Update, or by itself overnight
//! if that's switched on. It never updates while music is playing, recording
//! or a service is running; it backs up the data folder first; and it can go
//! back to the previous version.
//!
//! * **Raspberry Pi / Linux (.deb):** Cecilia runs without admin rights, so it
//!   asks a small root helper (`cecilia-update`, started by systemd when a
//!   request file appears) to download the release from GitHub, check it
//!   against the published SHA-256, install it and restart Cecilia. The
//!   request names only a version, so nothing else can be installed this way.
//! * **Windows (.zip):** Cecilia downloads and checks the zip itself, swaps in
//!   the new program and web files (keeping the old ones beside them for
//!   going back), and restarts.
//! * **Development builds** only report what's available.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::api::{ApiErrorBody, AppState};
use crate::settings::{UpdateChannel, UpdateSettings};

pub const DEFAULT_RELEASES_URL: &str = "https://api.github.com/repos/garethcheyne/cecilia/releases";
/// Where the Linux helper picks up requests and leaves results (owned by the `cecilia` user).
const DEB_REQUEST_DIR: &str = "/var/lib/cecilia/update";
const DEB_HELPER: &str = "/usr/lib/cecilia/cecilia-update";
/// How many backups taken before updates are kept.
const KEEP_BACKUPS: usize = 3;

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---------------------------------------------------------------------------
// Versions and releases
// ---------------------------------------------------------------------------

/// "v1.2.3" or "1.2.3-beta.1" → comparable parts. A pre-release sorts
/// before the release it leads to.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64, Option<String>)> {
    let t = text.trim().trim_start_matches('v');
    let (core, pre) = match t.split_once('-') {
        Some((c, p)) => (c, Some(p.to_string())),
        None => (t, None),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let v = (parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?);
    Some((v.0, v.1, v.2, pre))
}

/// Is `a` newer than `b`?
pub fn is_newer(a: &str, b: &str) -> bool {
    match (parse_version(a), parse_version(b)) {
        (Some(x), Some(y)) => {
            let core = (x.0, x.1, x.2).cmp(&(y.0, y.1, y.2));
            match core {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Less => false,
                // Same numbers: a release is newer than its pre-releases.
                std::cmp::Ordering::Equal => match (&x.3, &y.3) {
                    (None, Some(_)) => true,
                    (Some(p), Some(q)) => p > q,
                    _ => false,
                },
            }
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// The newest release on this channel that has a package for this computer.
pub fn pick_release(releases: &[GithubRelease], channel: UpdateChannel, kind: InstallKind) -> Option<&GithubRelease> {
    releases
        .iter()
        .filter(|r| !r.draft && (channel == UpdateChannel::Preview || !r.prerelease))
        .filter(|r| parse_version(&r.tag_name).is_some())
        .filter(|r| kind == InstallKind::Development || asset_for(r, kind).is_some())
        .max_by(|a, b| {
            if is_newer(&a.tag_name, &b.tag_name) {
                std::cmp::Ordering::Greater
            } else if is_newer(&b.tag_name, &a.tag_name) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
}

/// The package in a release for this kind of installation and processor.
pub fn asset_for(r: &GithubRelease, kind: InstallKind) -> Option<&GithubAsset> {
    let wanted = match kind {
        InstallKind::Deb => format!("_{}.deb", deb_arch()?),
        InstallKind::WindowsZip => "cecilia-windows-x64.zip".to_string(),
        InstallKind::Development => return None,
    };
    r.assets.iter().find(|a| a.name.ends_with(&wanted))
}

fn deb_arch() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "aarch64" => Some("arm64"),
        "arm" => Some("armhf"),
        "x86_64" => Some("amd64"),
        _ => None,
    }
}

/// The SHA-256 in a `.sha256` file ("<hex>  <name>" or just "<hex>").
pub fn parse_sha256(text: &str) -> Option<String> {
    let hex = text.split_whitespace().next()?.to_ascii_lowercase();
    (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex)
}

// ---------------------------------------------------------------------------
// How this copy of Cecilia was installed
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    /// The Debian package (Raspberry Pi, Linux): updated by the root helper.
    Deb,
    /// The Windows zip: updated in place.
    WindowsZip,
    /// Built from source: updates come through git.
    Development,
}

pub fn detect_kind() -> InstallKind {
    if let Ok(k) = std::env::var("CECILIA_UPDATE_KIND") {
        return match k.as_str() {
            "deb" => InstallKind::Deb,
            "windows-zip" => InstallKind::WindowsZip,
            _ => InstallKind::Development,
        };
    }
    let exe = std::env::current_exe().unwrap_or_default();
    if cfg!(target_os = "linux") && Path::new(DEB_HELPER).exists() && exe.starts_with("/usr/bin") {
        return InstallKind::Deb;
    }
    let in_build_tree = exe.components().any(|c| c.as_os_str() == "target");
    if cfg!(windows) && !in_build_tree && exe.parent().is_some_and(|d| d.join("web").is_dir()) {
        return InstallKind::WindowsZip;
    }
    InstallKind::Development
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReleaseInfo {
    pub version: String,
    pub name: String,
    /// What's new, as written in the release (Markdown).
    pub notes: String,
    pub published: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Idle,
    Checking,
    Downloading,
    Installing,
    Restarting,
    Failed,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BackupInfo {
    pub file: String,
    pub bytes: u64,
    pub created: u64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UpdateStatus {
    pub current: String,
    pub kind: InstallKind,
    pub latest: Option<ReleaseInfo>,
    /// A newer version is available on this channel.
    pub available: bool,
    pub phase: UpdatePhase,
    pub error: Option<String>,
    /// What the last update did ("Updated from 0.1.0 to 0.2.0"), once.
    pub last_result: Option<String>,
    /// Unix ms of the last successful check.
    pub last_checked: Option<u64>,
    pub settings: UpdateSettings,
    /// Why an update can't be installed right now, if it can't.
    pub blocked: Option<String>,
    /// The version Cecilia would go back to.
    pub previous: Option<String>,
    pub backups: Vec<BackupInfo>,
}

#[derive(Default)]
struct Inner {
    latest: Option<GithubRelease>,
    phase: Option<UpdatePhase>,
    error: Option<String>,
    last_result: Option<String>,
    last_checked: Option<u64>,
}

#[derive(Clone)]
pub struct Updater {
    inner: Arc<Mutex<Inner>>,
    releases_url: String,
    http: reqwest::Client,
    pub kind: InstallKind,
    data_dir: PathBuf,
}

impl Updater {
    pub fn new(data_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("Cecilia/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");
        let u = Self {
            inner: Arc::new(Mutex::new(Inner::default())),
            releases_url: crate::env("UPDATE_URL").unwrap_or_else(|| DEFAULT_RELEASES_URL.into()),
            http,
            kind: detect_kind(),
            data_dir,
        };
        u.collect_result();
        u
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Pick up what the last update did (the Linux helper leaves a note).
    fn collect_result(&self) {
        let note = if self.kind == InstallKind::Deb {
            Path::new(DEB_REQUEST_DIR).join("result")
        } else {
            self.data_dir.join("updates").join("result")
        };
        if let Ok(text) = std::fs::read_to_string(&note) {
            let _ = std::fs::remove_file(&note);
            let text = text.trim().to_string();
            let mut g = self.lock();
            if let Some(msg) = text.strip_prefix("failed ") {
                g.phase = Some(UpdatePhase::Failed);
                g.error = Some(msg.to_string());
            } else if !text.is_empty() {
                g.last_result = Some(text);
            }
        }
    }

    pub async fn check(&self, channel: UpdateChannel) -> Result<(), String> {
        self.lock().phase = Some(UpdatePhase::Checking);
        let result = async {
            let res = self
                .http
                .get(&self.releases_url)
                .send()
                .await
                .map_err(|_| "Couldn't reach GitHub to check for updates.".to_string())?;
            if !res.status().is_success() {
                return Err(format!("GitHub said {} when checking for updates.", res.status()));
            }
            let releases: Vec<GithubRelease> =
                res.json().await.map_err(|e| format!("The release list was unreadable: {e}"))?;
            Ok(pick_release(&releases, channel, self.kind).cloned())
        }
        .await;
        let mut g = self.lock();
        match result {
            Ok(latest) => {
                g.latest = latest;
                g.last_checked = Some(organ_core::recorder::unix_ms());
                g.phase = None;
                g.error = None;
                Ok(())
            }
            Err(e) => {
                g.phase = Some(UpdatePhase::Failed);
                g.error = Some(e.clone());
                Err(e)
            }
        }
    }

    fn previous_version(&self) -> Option<String> {
        let p = match self.kind {
            InstallKind::Deb => Path::new(DEB_REQUEST_DIR).join("previous-version"),
            InstallKind::WindowsZip => exe_dir()?.join("cecilia.previous-version"),
            InstallKind::Development => return None,
        };
        std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| parse_version(s).is_some())
    }

    pub fn backups(&self) -> Vec<BackupInfo> {
        let dir = self.data_dir.join("backups");
        let mut out: Vec<BackupInfo> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let meta = e.metadata().ok()?;
                (name.starts_with("before-") && name.ends_with(".zip")).then(|| BackupInfo {
                    file: name,
                    bytes: meta.len(),
                    created: meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_millis() as u64),
                })
            })
            .collect();
        out.sort_by_key(|b| std::cmp::Reverse(b.created));
        out
    }

    pub fn status(&self, settings: UpdateSettings, blocked: Option<String>) -> UpdateStatus {
        let g = self.lock();
        let latest = g.latest.as_ref().map(|r| ReleaseInfo {
            version: r.tag_name.trim_start_matches('v').to_string(),
            name: r.name.clone().unwrap_or_else(|| r.tag_name.clone()),
            notes: r.body.clone().unwrap_or_default(),
            published: r.published_at.clone(),
            url: r.html_url.clone(),
        });
        let available = latest.as_ref().is_some_and(|l| is_newer(&l.version, current_version()));
        let blocked = blocked.or_else(|| match self.kind {
            InstallKind::Development => Some("This is a development build: update it with git.".into()),
            _ => None,
        });
        UpdateStatus {
            current: current_version().into(),
            kind: self.kind,
            latest,
            available,
            phase: g.phase.unwrap_or(UpdatePhase::Idle),
            error: g.error.clone(),
            last_result: g.last_result.clone(),
            last_checked: g.last_checked,
            settings,
            blocked,
            previous: drop_guard_then(g, || self.previous_version()),
            backups: self.backups(),
        }
    }

    /// Back up the data folder, then install `version` (or go back to the
    /// previous one when `version` is None).
    pub async fn install(&self, version: Option<String>) -> Result<(), String> {
        if self.kind == InstallKind::Development {
            return Err("This is a development build: update it with git.".into());
        }
        let label = version.clone().unwrap_or_else(|| "previous".into());
        self.lock().phase = Some(UpdatePhase::Downloading);
        let result = async {
            self.backup_before(&label).await?;
            match self.kind {
                InstallKind::Deb => self.request_deb(version.as_deref()),
                InstallKind::WindowsZip => match &version {
                    Some(v) => self.install_windows(v).await,
                    None => rollback_windows(),
                },
                InstallKind::Development => unreachable!(),
            }
        }
        .await;
        let mut g = self.lock();
        match result {
            Ok(()) => {
                g.phase = Some(UpdatePhase::Restarting);
                g.error = None;
                Ok(())
            }
            Err(e) => {
                g.phase = Some(UpdatePhase::Failed);
                g.error = Some(e.clone());
                Err(e)
            }
        }
    }

    async fn backup_before(&self, label: &str) -> Result<(), String> {
        let dir = self.data_dir.clone();
        let label = label.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect::<String>();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let backups = dir.join("backups");
            std::fs::create_dir_all(&backups)?;
            let bytes = crate::system_api::make_backup(&dir)?;
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            std::fs::write(backups.join(format!("before-{label}-{stamp}.zip")), bytes)?;
            // Keep the newest few.
            let mut old: Vec<_> = std::fs::read_dir(&backups)?
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("before-"))
                .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
                .collect();
            old.sort_by_key(|o| std::cmp::Reverse(o.0));
            for (_, p) in old.into_iter().skip(KEEP_BACKUPS) {
                let _ = std::fs::remove_file(p);
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Couldn't back up before updating, so nothing was changed: {e}"))
    }

    /// Ask the root helper to install a version (or go back).
    fn request_deb(&self, version: Option<&str>) -> Result<(), String> {
        let line = match version {
            Some(v) => {
                if parse_version(v).is_none() {
                    return Err("That isn't a version number.".into());
                }
                format!("install {v}\n")
            }
            None => "rollback\n".into(),
        };
        let dir = Path::new(DEB_REQUEST_DIR);
        std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't ask for the update: {e}"))?;
        let tmp = dir.join("request.tmp");
        std::fs::write(&tmp, line).map_err(|e| format!("Couldn't ask for the update: {e}"))?;
        std::fs::rename(&tmp, dir.join("request")).map_err(|e| format!("Couldn't ask for the update: {e}"))?;
        self.lock().phase = Some(UpdatePhase::Installing);
        Ok(())
    }

    async fn install_windows(&self, version: &str) -> Result<(), String> {
        let release = self.lock().latest.clone().filter(|r| r.tag_name.trim_start_matches('v') == version);
        let release = release.ok_or("Check for updates again first.")?;
        let asset = asset_for(&release, InstallKind::WindowsZip).ok_or("That release has no Windows download.")?;
        let sum_asset = release
            .assets
            .iter()
            .find(|a| a.name == format!("{}.sha256", asset.name))
            .ok_or("That release has no checksum, so it can't be checked. Not installed.")?;
        let expected = parse_sha256(&self.get_text(&sum_asset.browser_download_url).await?)
            .ok_or("The checksum file is unreadable.")?;
        let bytes = self.get_bytes(&asset.browser_download_url).await?;
        let actual = hex(&Sha256::digest(&bytes));
        if actual != expected {
            return Err("The download didn't match its checksum, so it wasn't installed.".into());
        }
        self.lock().phase = Some(UpdatePhase::Installing);
        let dir = exe_dir().ok_or("Couldn't find where Cecilia is installed.")?;
        tokio::task::spawn_blocking(move || apply_windows_zip(&dir, &bytes, current_version()))
            .await
            .map_err(|e| e.to_string())??;
        restart_soon();
        Ok(())
    }

    async fn get_text(&self, url: &str) -> Result<String, String> {
        let res = self.http.get(url).send().await.map_err(|_| "The download failed.".to_string())?;
        if !res.status().is_success() {
            return Err(format!("The download failed ({}).", res.status()));
        }
        res.text().await.map_err(|e| e.to_string())
    }

    async fn get_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let res = self.http.get(url).send().await.map_err(|_| "The download failed.".to_string())?;
        if !res.status().is_success() {
            return Err(format!("The download failed ({}).", res.status()));
        }
        res.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
    }
}

fn drop_guard_then<T, F: FnOnce() -> T>(guard: std::sync::MutexGuard<'_, Inner>, f: F) -> T {
    drop(guard);
    f()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

// ---------------------------------------------------------------------------
// Windows: swap in place
// ---------------------------------------------------------------------------

/// Unpack a release zip over the installation in `dir`, keeping the current
/// program and web files as `cecilia.previous.exe` and `web.previous` for
/// going back. Windows lets a running program be renamed, not overwritten.
pub fn apply_windows_zip(dir: &Path, zip_bytes: &[u8], running_version: &str) -> Result<(), String> {
    let staging = dir.join("update-staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|_| "The download isn't a valid zip.".to_string())?;
    zip.extract(&staging).map_err(|e| format!("Couldn't unpack the update: {e}"))?;
    // The zip holds a "cecilia" folder with cecilia.exe and web/.
    let root =
        if staging.join("cecilia").join("cecilia.exe").exists() { staging.join("cecilia") } else { staging.clone() };
    if !root.join("cecilia.exe").exists() || !root.join("web").is_dir() {
        return Err("The update is missing the program or its web files.".into());
    }
    let exe = dir.join("cecilia.exe");
    let old_exe = dir.join("cecilia.previous.exe");
    let web = dir.join("web");
    let old_web = dir.join("web.previous");
    let _ = std::fs::remove_file(&old_exe);
    let _ = std::fs::remove_dir_all(&old_web);
    std::fs::rename(&exe, &old_exe).map_err(|e| format!("Couldn't set the old program aside: {e}"))?;
    if let Err(e) = std::fs::rename(root.join("cecilia.exe"), &exe) {
        let _ = std::fs::rename(&old_exe, &exe);
        return Err(format!("Couldn't put the new program in place: {e}"));
    }
    if web.exists() {
        std::fs::rename(&web, &old_web).map_err(|e| format!("Couldn't set the old web files aside: {e}"))?;
    }
    std::fs::rename(root.join("web"), &web).map_err(|e| format!("Couldn't put the new web files in place: {e}"))?;
    std::fs::write(dir.join("cecilia.previous-version"), running_version).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

/// Swap the previous program and web files back in, then restart.
fn rollback_windows() -> Result<(), String> {
    let dir = exe_dir().ok_or("Couldn't find where Cecilia is installed.")?;
    restore_previous_windows(&dir)?;
    restart_soon();
    Ok(())
}

pub fn restore_previous_windows(dir: &Path) -> Result<(), String> {
    let (exe, old_exe) = (dir.join("cecilia.exe"), dir.join("cecilia.previous.exe"));
    let (web, old_web) = (dir.join("web"), dir.join("web.previous"));
    if !old_exe.exists() || !old_web.is_dir() {
        return Err("There's no previous version to go back to.".into());
    }
    let newer_exe = dir.join("cecilia.newer.exe");
    let newer_web = dir.join("web.newer");
    let _ = std::fs::remove_file(&newer_exe);
    let _ = std::fs::remove_dir_all(&newer_web);
    std::fs::rename(&exe, &newer_exe).map_err(|e| e.to_string())?;
    std::fs::rename(&old_exe, &exe).map_err(|e| e.to_string())?;
    std::fs::rename(&web, &newer_web).map_err(|e| e.to_string())?;
    std::fs::rename(&old_web, &web).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(dir.join("cecilia.previous-version"));
    Ok(())
}

/// Start the (new) program and leave, once this response has gone out.
fn restart_soon() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(800));
        if let Ok(exe) = std::env::current_exe() {
            // current_exe still names cecilia.exe, which is now the new one.
            let exe = exe.parent().map(|d| d.join("cecilia.exe")).unwrap_or(exe);
            let _ = std::process::Command::new(exe).args(std::env::args().skip(1)).spawn();
        }
        std::process::exit(0);
    });
}

// ---------------------------------------------------------------------------
// When it's safe, and the background schedule
// ---------------------------------------------------------------------------

/// Why updating now would interrupt something, if it would.
pub fn busy(s: &AppState) -> Option<String> {
    use organ_core::player::PlayerState;
    use organ_core::recorder::RecorderState;
    if !matches!(s.player.status().state, PlayerState::Stopped) {
        return Some("Music is playing. Updates wait until it stops.".into());
    }
    if s.recorder.status().state == RecorderState::Recording {
        return Some("Cecilia is recording. Updates wait until it's finished.".into());
    }
    if s.service.status().state != crate::playlists::ServiceState::Idle {
        return Some("A service is running. Updates wait until it ends.".into());
    }
    None
}

/// Check now and then; install overnight when allowed and nothing's happening.
pub fn start_schedule(s: AppState) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(90)).await;
        let mut last_check = std::time::Instant::now() - Duration::from_secs(24 * 3600);
        loop {
            let settings = s.settings.get().updates;
            if last_check.elapsed() > Duration::from_secs(6 * 3600) {
                let _ = s.updater.check(settings.channel).await;
                last_check = std::time::Instant::now();
            }
            let status = s.updater.status(settings.clone(), None);
            let hour = chrono::Timelike::hour(&chrono::Local::now());
            if settings.auto_install
                && status.available
                && (2..5).contains(&hour)
                && status.phase != UpdatePhase::Failed
                && busy(&s).is_none()
                && s.updater.kind != InstallKind::Development
            {
                if let Some(v) = status.latest.map(|l| l.version) {
                    tracing::info!(version = %v, "installing an update overnight");
                    let _ = s.updater.install(Some(v)).await;
                }
            }
            tokio::time::sleep(Duration::from_secs(10 * 60)).await;
        }
    });
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/updates", get(update_status))
        .route("/updates/check", post(update_check))
        .route("/updates/install", post(update_install))
        .route("/updates/rollback", post(update_rollback))
        .route("/updates/settings", put(update_settings))
}

fn error(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(ApiErrorBody { error: msg.into() })).into_response()
}

fn status_of(s: &AppState) -> UpdateStatus {
    s.updater.status(s.settings.get().updates, busy(s))
}

/// The version running, what's available, and how updating is set up.
#[utoipa::path(get, path = "/api/v1/updates", tag = "updates", responses((status = 200, body = UpdateStatus)))]
pub async fn update_status(State(s): State<AppState>) -> Json<UpdateStatus> {
    Json(status_of(&s))
}

/// Look for a new version now.
#[utoipa::path(post, path = "/api/v1/updates/check", tag = "updates",
    responses((status = 200, body = UpdateStatus), (status = 503, body = ApiErrorBody)))]
pub async fn update_check(State(s): State<AppState>) -> Result<Json<UpdateStatus>, Response> {
    s.updater.check(s.settings.get().updates.channel).await.map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(Json(status_of(&s)))
}

/// Install the newest version now (after a backup). Cecilia restarts.
#[utoipa::path(post, path = "/api/v1/updates/install", tag = "updates",
    responses((status = 200, body = UpdateStatus), (status = 409, body = ApiErrorBody), (status = 502, body = ApiErrorBody)))]
pub async fn update_install(State(s): State<AppState>) -> Result<Json<UpdateStatus>, Response> {
    let st = status_of(&s);
    if let Some(why) = st.blocked {
        return Err(error(StatusCode::CONFLICT, why));
    }
    let latest =
        st.latest.filter(|_| st.available).ok_or_else(|| error(StatusCode::CONFLICT, "Cecilia is up to date."))?;
    s.updater.install(Some(latest.version)).await.map_err(|e| error(StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(status_of(&s)))
}

/// Go back to the version that was running before the last update.
#[utoipa::path(post, path = "/api/v1/updates/rollback", tag = "updates",
    responses((status = 200, body = UpdateStatus), (status = 409, body = ApiErrorBody)))]
pub async fn update_rollback(State(s): State<AppState>) -> Result<Json<UpdateStatus>, Response> {
    let st = status_of(&s);
    if let Some(why) = st.blocked {
        return Err(error(StatusCode::CONFLICT, why));
    }
    if st.previous.is_none() {
        return Err(error(StatusCode::CONFLICT, "There's no previous version to go back to."));
    }
    s.updater.install(None).await.map_err(|e| error(StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(status_of(&s)))
}

/// Channel (stable or preview) and whether to install overnight by itself.
#[utoipa::path(put, path = "/api/v1/updates/settings", tag = "updates", request_body = UpdateSettings,
    responses((status = 200, body = UpdateStatus)))]
pub async fn update_settings(
    State(s): State<AppState>,
    Json(b): Json<UpdateSettings>,
) -> Result<Json<UpdateStatus>, Response> {
    let channel_changed = s.settings.get().updates.channel != b.channel;
    s.settings
        .update(|st| st.updates = b.clone())
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if channel_changed {
        let _ = s.updater.check(b.channel).await;
    }
    Ok(Json(status_of(&s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, pre: bool, assets: &[&str]) -> GithubRelease {
        GithubRelease {
            tag_name: tag.into(),
            name: None,
            body: Some(format!("What's new in {tag}")),
            draft: false,
            prerelease: pre,
            published_at: None,
            html_url: None,
            assets: assets
                .iter()
                .map(|n| GithubAsset { name: n.to_string(), browser_download_url: format!("https://x/{n}") })
                .collect(),
        }
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "1.0.0-beta.2"));
        assert!(is_newer("1.0.0-beta.2", "1.0.0-beta.1"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("nonsense", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.0"), "numbers, not text");
    }

    #[test]
    fn picks_the_newest_release_on_the_channel_with_a_package() {
        let zip = "cecilia-windows-x64.zip";
        let rs = vec![
            release("v0.3.0-beta.1", true, &[zip]),
            release("v0.2.0", false, &[zip]),
            release("v0.2.5", false, &[]),
            release("v0.1.0", false, &[zip]),
        ];
        let stable = pick_release(&rs, UpdateChannel::Stable, InstallKind::WindowsZip).unwrap();
        assert_eq!(stable.tag_name, "v0.2.0", "0.2.5 has no Windows package; 0.3.0 is a preview");
        let preview = pick_release(&rs, UpdateChannel::Preview, InstallKind::WindowsZip).unwrap();
        assert_eq!(preview.tag_name, "v0.3.0-beta.1");
    }

    #[test]
    fn reads_checksum_files() {
        let h = "a".repeat(64);
        assert_eq!(parse_sha256(&format!("{h}  cecilia.zip\n")), Some(h.clone()));
        assert_eq!(parse_sha256(&h.to_uppercase()), Some(h));
        assert_eq!(parse_sha256("not a hash"), None);
    }

    #[test]
    fn windows_update_swaps_and_goes_back() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("cecilia.exe"), "old program").unwrap();
        std::fs::create_dir(d.join("web")).unwrap();
        std::fs::write(d.join("web/index.html"), "old page").unwrap();

        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("cecilia/cecilia.exe", o).unwrap();
            z.write_all(b"new program").unwrap();
            z.start_file("cecilia/web/index.html", o).unwrap();
            z.write_all(b"new page").unwrap();
            z.finish().unwrap();
        }
        apply_windows_zip(d, buf.get_ref(), "0.1.0").unwrap();
        assert_eq!(std::fs::read_to_string(d.join("cecilia.exe")).unwrap(), "new program");
        assert_eq!(std::fs::read_to_string(d.join("web/index.html")).unwrap(), "new page");
        assert_eq!(std::fs::read_to_string(d.join("cecilia.previous.exe")).unwrap(), "old program");
        assert_eq!(std::fs::read_to_string(d.join("cecilia.previous-version")).unwrap(), "0.1.0");

        restore_previous_windows(d).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("cecilia.exe")).unwrap(), "old program");
        assert_eq!(std::fs::read_to_string(d.join("web/index.html")).unwrap(), "old page");
        assert!(!d.join("cecilia.previous-version").exists());
        assert!(restore_previous_windows(d).is_err(), "only one step back");
    }

    #[test]
    fn a_bad_zip_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cecilia.exe"), "old").unwrap();
        assert!(apply_windows_zip(dir.path(), b"not a zip", "0.1.0").is_err());
        assert_eq!(std::fs::read_to_string(dir.path().join("cecilia.exe")).unwrap(), "old");
    }
}
