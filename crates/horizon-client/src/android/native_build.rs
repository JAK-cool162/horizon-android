//! An APK with the profile's native mods, built by the player's own GitHub fork. A phone cannot
//! compile Rust, so the fork's Android dev workflow compiles the mods in: this uploads them to
//! its `native-mods` branch, starts the workflow, follows it, downloads the APK and hands it to
//! Android's installer. The token never leaves the Keystore-encrypted store but for GitHub.
use crate::native_build_plan as plan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// The secure store's key for the GitHub token (its file names allow hex and dashes).
pub const TOKEN_KEY: &str = "0000-0001";
/// The profile folder holding the built APK; Android Files shows it as "APK builds".
pub const APK_FOLDER: &str = "horizon-builds";
pub const APK_FILE: &str = "horizon-native-mods.apk";
/// The run being followed, so a closed launcher picks it up again.
const RECORD: &str = "native-build.json";
const MAX_APK: u64 = 1 << 30;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Idle,
    Working(String),
    Ready,
    Failed(String),
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct Record {
    run: u64,
    url: String,
    mods: Vec<String>,
}

#[derive(Default)]
struct Shared {
    phase: Phase,
    url: Option<String>,
    running: bool,
}

#[derive(Default)]
pub struct NativeBuild {
    shared: Arc<Mutex<Shared>>,
}

pub fn token() -> Option<String> {
    use crate::account::SecretStore;
    crate::secure_store::AndroidStore
        .get(TOKEN_KEY)
        .ok()
        .flatten()
        .filter(|t| !t.trim().is_empty())
}
pub fn save_token(token: &str) -> Result<(), String> {
    use crate::account::SecretStore;
    let token = token.trim();
    if token.is_empty() {
        crate::secure_store::AndroidStore.delete(TOKEN_KEY)
    } else {
        crate::secure_store::AndroidStore.set(TOKEN_KEY, token)
    }
}
/// The repository builds go to: the one set in Setup, else the one this APK was built from.
pub fn repository(configured: &str) -> Option<String> {
    plan::repository(configured).or_else(|| plan::repository(option_env!("GITHUB_REPOSITORY")?))
}
pub fn apk(game: &Path) -> PathBuf {
    game.join(APK_FOLDER).join(APK_FILE)
}

impl NativeBuild {
    pub fn phase(&self) -> Phase {
        self.lock().phase.clone()
    }
    pub fn url(&self) -> Option<String> {
        self.lock().url.clone()
    }
    pub fn running(&self) -> bool {
        self.lock().running
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// What the launcher shows on opening: a run still going, or an APK waiting to install.
    pub fn restore(&self, game: &Path, repo: Option<String>) {
        if self.running() {
            return;
        }
        let record: Option<Record> = std::fs::read(game.join(RECORD))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        match (record, repo, token()) {
            (Some(record), Some(repo), Some(token)) => {
                self.lock().url = Some(record.url.clone());
                let api = Api::new(repo, token);
                self.spawn(game, move |game, set| follow(&api, game, &record, set));
            }
            _ if apk(game).is_file() => self.lock().phase = Phase::Ready,
            _ => {}
        }
    }
    /// Uploads `mods` (paths of `.hnmod` files) and builds an APK with them.
    pub fn start(&self, game: &Path, repo: String, token: String, mods: Vec<PathBuf>) {
        if self.running() {
            return;
        }
        let api = Api::new(repo, token);
        let shared = self.shared.clone();
        self.spawn(game, move |game, set| {
            let record = upload_and_dispatch(&api, game, &mods, set)?;
            shared.lock().unwrap_or_else(|e| e.into_inner()).url = Some(record.url.clone());
            follow(&api, game, &record, set)
        });
    }
    fn spawn(
        &self,
        game: &Path,
        job: impl FnOnce(&Path, &dyn Fn(String)) -> Result<(), String> + Send + 'static,
    ) {
        let shared = self.shared.clone();
        let game = game.to_owned();
        {
            let mut s = self.lock();
            s.running = true;
            s.phase = Phase::Working("Contacting GitHub...".into());
        }
        std::thread::spawn(move || {
            let set = |text: String| {
                shared.lock().unwrap_or_else(|e| e.into_inner()).phase = Phase::Working(text)
            };
            let result = job(&game, &set);
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            s.running = false;
            s.phase = match result {
                Ok(()) => Phase::Ready,
                Err(e) => Phase::Failed(e),
            };
        });
    }
}

struct Api {
    agent: ureq::Agent,
    repo: String,
    token: String,
}
impl Api {
    fn new(repo: String, token: String) -> Self {
        let agent = ureq::AgentBuilder::new()
            .https_only(true)
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(120))
            .build();
        Self { agent, repo, token }
    }
    fn request(&self, agent: &ureq::Agent, method: &str, url: &str) -> ureq::Request {
        agent
            .request(method, url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set("User-Agent", "horizon-android")
    }
    fn url(&self, path: &str) -> String {
        format!("https://api.github.com/repos/{}{path}", self.repo)
    }
    /// The JSON at `path`; `None` when GitHub says it is not there.
    fn get(&self, path: &str) -> Result<Option<Value>, String> {
        match self.request(&self.agent, "GET", &self.url(path)).call() {
            Ok(r) => r.into_json().map(Some).map_err(|e| e.to_string()),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(failure(e)),
        }
    }
    fn send(&self, method: &str, path: &str, body: Value) -> Result<Value, String> {
        let r = self
            .request(&self.agent, method, &self.url(path))
            .send_json(body)
            .map_err(failure)?;
        Ok(r.into_json().unwrap_or(Value::Null))
    }
    fn delete(&self, path: &str) -> Result<(), String> {
        match self.request(&self.agent, "DELETE", &self.url(path)).call() {
            Ok(_) | Err(ureq::Error::Status(404, _)) => Ok(()),
            Err(e) => Err(failure(e)),
        }
    }
    /// Starts `workflow` on the base branch and finds its run by the `request` tag it is given.
    fn dispatch(
        &self,
        workflow: &str,
        body: Value,
        request: &str,
    ) -> Result<(u64, String), String> {
        self.send(
            "POST",
            &format!("/actions/workflows/{workflow}/dispatches"),
            body,
        )?;
        let runs = format!(
            "/actions/workflows/{workflow}/runs?event=workflow_dispatch&branch={}&per_page=20",
            plan::BASE_BRANCH
        );
        for _ in 0..36 {
            std::thread::sleep(Duration::from_secs(5));
            if let Some(found) = self.get(&runs)?.and_then(|r| plan::find_run(&r, request)) {
                return Ok(found);
            }
        }
        Err("GitHub did not start the workflow: check Actions on the repository".into())
    }
}

/// A tag naming one dispatched run.
fn request_tag(kind: &str) -> String {
    format!(
        "{kind}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    )
}

/// The signing key the dev workflow needs, made once by the fork's key workflow: the app hands
/// that workflow its own token as a short-lived, sealed secret and removes it afterwards.
fn ensure_signing_key(api: &Api, set: &dyn Fn(String)) -> Result<(), String> {
    let path = format!("/actions/secrets/{}", plan::SIGNING_SECRET);
    if api
        .get(&path)
        .map_err(|e| format!("{e} (the token needs Secrets read and write too)"))?
        .is_some()
    {
        return Ok(());
    }
    let text = "Creating your signing key on GitHub (once)...";
    set(text.into());
    let key = api
        .get("/actions/secrets/public-key")?
        .ok_or("GitHub gave no key for the repository's secrets")?;
    let key_id = key["key_id"]
        .as_str()
        .ok_or("GitHub's secrets key has no id")?;
    let public: [u8; 32] = key["key"]
        .as_str()
        .and_then(plan::base64_decode)
        .and_then(|k| k.try_into().ok())
        .ok_or("GitHub's secrets key is not an X25519 key")?;
    let sealed = crate::sealed_box::seal(&public, api.token.as_bytes())?;
    let setup = format!("/actions/secrets/{}", plan::SETUP_SECRET);
    api.send(
        "PUT",
        &setup,
        serde_json::json!({"encrypted_value": plan::base64(&sealed), "key_id": key_id}),
    )?;
    let request = request_tag("key");
    let result = api
        .dispatch(
            plan::KEY_WORKFLOW,
            serde_json::json!({"ref": plan::BASE_BRANCH, "inputs": {"request": request}}),
            &request,
        )
        .and_then(|(run, _)| wait(api, run, &|_| set(text.into())));
    // The key workflow has read it, or failed: either way it must not stay.
    let removed = api.delete(&setup);
    result?;
    removed?;
    api.get(&path)?
        .map(|_| ())
        .ok_or("The key workflow ran but the signing key is missing: check Actions".into())
}

/// Waits for run `run`; `waiting(minutes)` hears each poll.
fn wait(api: &Api, run: u64, waiting: &dyn Fn(u64)) -> Result<(), String> {
    let started = Instant::now();
    loop {
        let state = api
            .get(&format!("/actions/runs/{run}"))?
            .ok_or("The run is gone from GitHub")?;
        match plan::run_state(&state) {
            Ok(Some(())) => return Ok(()),
            Ok(None) => {
                waiting(started.elapsed().as_secs() / 60);
                std::thread::sleep(Duration::from_secs(10));
            }
            Err(e) => return Err(e),
        }
    }
}

/// GitHub's answer as something the player can act on.
fn failure(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(401, _) => {
            "GitHub refused the token: set a new one in Setup > GitHub token".into()
        }
        ureq::Error::Status(code @ (403 | 404), _) => format!(
            "GitHub said {code}: the token needs Contents, Actions and Secrets read and write on your fork"
        ),
        ureq::Error::Status(code, r) => {
            let message = r
                .into_json::<Value>()
                .ok()
                .and_then(|v| v["message"].as_str().map(str::to_owned))
                .unwrap_or_default();
            format!("GitHub said {code} {message}")
        }
        ureq::Error::Transport(t) => format!("Could not reach GitHub: {t}"),
    }
}

fn upload_and_dispatch(
    api: &Api,
    game: &Path,
    mods: &[PathBuf],
    set: &dyn Fn(String),
) -> Result<Record, String> {
    api.get("")?
        .ok_or("The repository in Setup does not exist or the token cannot see it")?;
    ensure_signing_key(api, set)?;
    let base = api
        .get(&format!("/git/ref/heads/{}", plan::BASE_BRANCH))?
        .and_then(|r| r["object"]["sha"].as_str().map(str::to_owned))
        .ok_or("The repository has no android branch to build")?;
    if api
        .get(&format!("/git/ref/heads/{}", plan::MODS_BRANCH))?
        .is_none()
    {
        api.send(
            "POST",
            "/git/refs",
            serde_json::json!({"ref": format!("refs/heads/{}", plan::MODS_BRANCH), "sha": base}),
        )?;
    }
    let mut paths = Vec::new();
    for file in mods {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid mod file name")?;
        let path = plan::upload_path(name).ok_or(format!("{name}: not a mod file name"))?;
        set(format!("Uploading {name}..."));
        let bytes = std::fs::read(file).map_err(|e| format!("{name}: {e}"))?;
        let existing = api
            .get(&format!("/contents/{path}?ref={}", plan::MODS_BRANCH))?
            .and_then(|v| v["sha"].as_str().map(str::to_owned));
        let mut body = serde_json::json!({
            "message": format!("Native mod {name} for an Android build"),
            "content": plan::base64(&bytes),
            "branch": plan::MODS_BRANCH,
        });
        if let Some(sha) = existing {
            body["sha"] = sha.into();
        }
        api.send("PUT", &format!("/contents/{path}"), body)?;
        paths.push(path);
    }
    let request = request_tag("native");
    set("Starting the build on GitHub...".into());
    let (run, url) = api.dispatch(
        plan::WORKFLOW,
        plan::dispatch(&paths, &request, plan::abi()),
        &request,
    )?;
    let record = Record {
        run,
        url,
        mods: paths,
    };
    let _ = std::fs::write(
        game.join(RECORD),
        serde_json::to_vec(&record).unwrap_or_default(),
    );
    Ok(record)
}

/// Waits for the run, then downloads its APK for this ABI into the profile's APK folder.
fn follow(api: &Api, game: &Path, record: &Record, set: &dyn Fn(String)) -> Result<(), String> {
    let started = Instant::now();
    loop {
        let run = api
            .get(&format!("/actions/runs/{}", record.run))?
            .ok_or("The build run is gone from GitHub")?;
        match plan::run_state(&run) {
            Ok(Some(())) => break,
            Ok(None) => {
                let minutes = started.elapsed().as_secs() / 60;
                set(match run["status"].as_str() {
                    Some("queued") | Some("waiting") | Some("pending") => {
                        "Waiting for a GitHub runner...".into()
                    }
                    _ => format!(
                        "Building on GitHub ({minutes} min; usually 40-70). You can close Horizon."
                    ),
                });
                std::thread::sleep(Duration::from_secs(20));
            }
            Err(e) => {
                let _ = std::fs::remove_file(game.join(RECORD));
                return Err(e);
            }
        }
    }
    set("Downloading the APK...".into());
    let artifacts = api
        .get(&format!("/actions/runs/{}/artifacts", record.run))?
        .unwrap_or_default();
    let (url, size) = plan::artifact(&artifacts, plan::abi())
        .ok_or(format!("The build has no {} APK", plan::abi()))?;
    if size > MAX_APK {
        return Err("The build's download is too large".into());
    }
    let dir = game.join(APK_FOLDER);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Refusing a redirected APK folder".into());
    }
    // The archive link redirects to signed storage, which must not get the token.
    let direct = ureq::AgentBuilder::new()
        .https_only(true)
        .redirects(0)
        .timeout_connect(Duration::from_secs(20))
        .build();
    let first = api.request(&direct, "GET", &url).call().map_err(failure)?;
    let response = match first.header("Location") {
        Some(location) if (300..400).contains(&first.status()) => {
            let location = location.to_owned();
            api.agent.get(&location).call().map_err(|e| e.to_string())?
        }
        _ => first,
    };
    let archive = dir.join("download.partial");
    {
        let mut out = std::fs::File::create(&archive).map_err(|e| e.to_string())?;
        let mut input = response.into_reader().take(MAX_APK + 1);
        let mut buffer = vec![0u8; 1 << 16];
        let mut done = 0u64;
        loop {
            let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            if let Some(percent) = (done * 100).checked_div(size) {
                set(format!("Downloading the APK ({}%)...", percent.min(100)));
            }
        }
        if done > MAX_APK {
            let _ = std::fs::remove_file(&archive);
            return Err("The build's download is too large".into());
        }
    }
    let result = extract_apk(&archive, &dir);
    let _ = std::fs::remove_file(&archive);
    result?;
    let _ = std::fs::remove_file(game.join(RECORD));
    Ok(())
}

/// The AI conversion (`.github/workflows/ai-convert.yml`): its workflow, the branch the launcher
/// uploads the mod to, the artifact it answers with and the secret it needs.
const CONVERT_WORKFLOW: &str = "ai-convert.yml";
const CONVERT_BRANCH: &str = "ai-convert";
const CONVERT_ARTIFACT: &str = "converted-hmod";
/// The AI keys the workflow takes, any one: Gemini's or Groq's (through aider) or Anthropic's
/// (Claude Code).
const AI_SECRETS: [&str; 3] = ["GEMINI_API_KEY", "GROQ_API_KEY", "ANTHROPIC_API_KEY"];
const MAX_CONVERTED: u64 = 64 << 20;

/// A Java mod (.jar) or native mod (.hnmod) rewritten as a sandboxed .hmod by Claude Code on the
/// player's GitHub fork: this uploads it, starts the workflow, follows it and installs the .hmod
/// it answers with in the profile's mods, keeping its REPORT.md for the status line.
#[derive(Default)]
pub struct AiConvert {
    shared: Arc<Mutex<Shared>>,
    report: Arc<Mutex<Option<String>>>,
}
impl AiConvert {
    pub fn phase(&self) -> Phase {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .phase
            .clone()
    }
    pub fn running(&self) -> bool {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .running
    }
    /// The workflow run's page, once it started.
    pub fn url(&self) -> Option<String> {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .url
            .clone()
    }
    /// The conversion's report, once, after it finished.
    pub fn take_report(&self) -> Option<String> {
        self.report.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
    /// Converts `file` (a staged .jar or .hnmod) into `mods` (the profile's .hmod folder).
    pub fn start(&self, mods: PathBuf, repo: String, token: String, file: PathBuf) {
        if self.running() {
            return;
        }
        let api = Api::new(repo, token);
        let shared = self.shared.clone();
        let report = self.report.clone();
        {
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            s.running = true;
            s.url = None;
            s.phase = Phase::Working("Contacting GitHub...".into());
        }
        std::thread::spawn(move || {
            let set = |text: String| {
                shared.lock().unwrap_or_else(|e| e.into_inner()).phase = Phase::Working(text)
            };
            let url = |u: String| shared.lock().unwrap_or_else(|e| e.into_inner()).url = Some(u);
            let result = convert(&api, &mods, &file, &set, &url);
            let _ = std::fs::remove_file(&file);
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            s.running = false;
            s.phase = match result {
                Ok(text) => {
                    *report.lock().unwrap_or_else(|e| e.into_inner()) = Some(text);
                    Phase::Ready
                }
                Err(e) => Phase::Failed(e),
            };
        });
    }
}

/// Uploads, runs and installs one conversion; the answer is the report's text.
fn convert(
    api: &Api,
    mods: &Path,
    file: &Path,
    set: &dyn Fn(String),
    url: &dyn Fn(String),
) -> Result<String, String> {
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| content_name(n))
        .ok_or("Choose a .jar or .hnmod file")?
        .to_owned();
    api.get("")?
        .ok_or("The repository in Setup does not exist or the token cannot see it")?;
    let mut has_key = false;
    for secret in AI_SECRETS {
        has_key |= api
            .get(&format!("/actions/secrets/{secret}"))
            .map_err(|e| format!("{e} (the token needs Secrets read too)"))?
            .is_some();
    }
    if !has_key {
        return Err(
            "Add your AI key first: copy a Gemini key (free at aistudio.google.com), then tap \
             Setup > AI key"
                .into(),
        );
    }
    let base = api
        .get(&format!("/git/ref/heads/{}", plan::BASE_BRANCH))?
        .and_then(|r| r["object"]["sha"].as_str().map(str::to_owned))
        .ok_or("The repository has no android branch")?;
    if api
        .get(&format!("/git/ref/heads/{CONVERT_BRANCH}"))?
        .is_none()
    {
        api.send(
            "POST",
            "/git/refs",
            serde_json::json!({"ref": format!("refs/heads/{CONVERT_BRANCH}"), "sha": base}),
        )
        .map_err(|e| denied(e, "Contents"))?;
    }
    set(format!("Uploading {name}..."));
    let path = format!("convert/{name}");
    let bytes = std::fs::read(file).map_err(|e| format!("{name}: {e}"))?;
    let existing = api
        .get(&format!("/contents/{path}?ref={CONVERT_BRANCH}"))?
        .and_then(|v| v["sha"].as_str().map(str::to_owned));
    let mut body = serde_json::json!({
        "message": format!("{name}, to convert into a .hmod"),
        "content": plan::base64(&bytes),
        "branch": CONVERT_BRANCH,
    });
    if let Some(sha) = existing {
        body["sha"] = sha.into();
    }
    api.send("PUT", &format!("/contents/{path}"), body)
        .map_err(|e| denied(e, "Contents"))?;
    let request = request_tag("convert");
    set("Starting the AI conversion on GitHub...".into());
    let (run, link) = api
        .dispatch(
            CONVERT_WORKFLOW,
            serde_json::json!({"ref": plan::BASE_BRANCH, "inputs": {"file": path, "request": request}}),
            &request,
        )
        .map_err(|e| denied(e, "Actions"))?;
    url(link);
    let started = Instant::now();
    // The run goes on on GitHub whatever the phone's connection does: a lost poll waits and asks
    // again (Wi-Fi to mobile data, the screen off) rather than ending the conversion.
    let mut lost = 0;
    loop {
        let state = match api.get(&format!("/actions/runs/{run}")) {
            Ok(state) => {
                lost = 0;
                state.ok_or("The conversion run is gone from GitHub")?
            }
            Err(e) if e.starts_with("Could not reach GitHub") && lost < 60 => {
                lost += 1;
                set(format!(
                    "Waiting for the connection... the AI keeps converting {name} on GitHub"
                ));
                std::thread::sleep(Duration::from_secs(10));
                continue;
            }
            Err(e) => return Err(e),
        };
        match plan::run_state(&state) {
            Ok(Some(())) => break,
            Ok(None) => {
                set(format!(
                    "AI is converting {name} on GitHub ({} min; usually 5-20). You can close Horizon.",
                    started.elapsed().as_secs() / 60
                ));
                std::thread::sleep(Duration::from_secs(15));
            }
            Err(e) => return Err(e.replace("The build", "The conversion")),
        }
    }
    set("Downloading the converted mod...".into());
    let artifacts =
        retry(|| api.get(&format!("/actions/runs/{run}/artifacts")))?.unwrap_or_default();
    let (link, size) = artifacts["artifacts"]
        .as_array()
        .and_then(|list| {
            list.iter().find(|a| {
                a["name"].as_str() == Some(CONVERT_ARTIFACT) && a["expired"].as_bool() != Some(true)
            })
        })
        .map(|a| {
            (
                a["archive_download_url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                a["size_in_bytes"].as_u64().unwrap_or(0),
            )
        })
        .ok_or("The conversion gave nothing back: open its run for the log")?;
    if size > MAX_CONVERTED {
        return Err("The converted mod is too large".into());
    }
    std::fs::create_dir_all(mods).map_err(|e| e.to_string())?;
    let archive = mods.join(".ai-convert.partial");
    retry(|| download(api, &link, &archive, MAX_CONVERTED))?;
    let result = install_converted(&archive, mods);
    let _ = std::fs::remove_file(&archive);
    result
}

/// `call`, asked again a few times while GitHub can't be reached.
fn retry<T>(mut call: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let mut tries = 0;
    loop {
        match call() {
            Err(e) if e.starts_with("Could not reach GitHub") && tries < 6 => {
                tries += 1;
                std::thread::sleep(Duration::from_secs(10));
            }
            result => return result,
        }
    }
}

/// The AI provider an API key belongs to, by its prefix, and the secret the workflow reads it
/// from: Gemini's (`AIza`), Groq's (`gsk_`) or Anthropic's (`sk-ant-`).
pub fn ai_key(key: &str) -> Option<(&'static str, &'static str)> {
    let plain = (20..=240).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !plain {
        return None;
    }
    if key.starts_with("AIza") {
        Some(("Gemini", AI_SECRETS[0]))
    } else if key.starts_with("gsk_") {
        Some(("Groq", AI_SECRETS[1]))
    } else if key.starts_with("sk-ant-") {
        Some(("Anthropic", AI_SECRETS[2]))
    } else {
        None
    }
}

/// Saves an AI key as the fork's Actions secret its provider's conversion reads, sealed with the
/// repository's public key as GitHub asks: it goes to GitHub only and is not kept on the phone.
/// The result reaches the status line.
pub fn save_ai_key(repo: String, token: String, key: String) {
    let Some((provider, secret)) = ai_key(&key) else {
        return;
    };
    std::thread::spawn(move || {
        let api = Api::new(repo, token);
        let result = (|| -> Result<(), String> {
            let public = api
                .get("/actions/secrets/public-key")
                .map_err(|e| denied(e, "Secrets"))?
                .ok_or("GitHub gave no key for the repository's secrets")?;
            let key_id = public["key_id"]
                .as_str()
                .ok_or("GitHub's secrets key has no id")?;
            let sealing: [u8; 32] = public["key"]
                .as_str()
                .and_then(plan::base64_decode)
                .and_then(|k| k.try_into().ok())
                .ok_or("GitHub's secrets key is not an X25519 key")?;
            let sealed = crate::sealed_box::seal(&sealing, key.as_bytes())?;
            api.send(
                "PUT",
                &format!("/actions/secrets/{secret}"),
                serde_json::json!({"encrypted_value": plan::base64(&sealed), "key_id": key_id}),
            )
            .map_err(|e| denied(e, "Secrets"))?;
            Ok(())
        })();
        crate::documents::notify(
            match result {
                Ok(()) => format!(
                    "{provider} key saved on your GitHub fork (secret {secret}), not on this \
                     phone. Convert with AI is ready."
                ),
                Err(e) => format!("Could not save the {provider} key: {e}"),
            },
            false,
        );
    });
}

/// A refusal of the step that needs `permission`, named: GitHub's fine-grained tokens add a
/// permission as Read-only, and a 403 alone doesn't say which one to change.
fn denied(error: String, permission: &str) -> String {
    if error.contains("403") {
        format!(
            "GitHub refused (403): set the token's {permission} permission to \"Read and \
             write\" (github.com > Settings > Developer settings > Fine-grained tokens > your \
             token > Edit); the saved token stays the same"
        )
    } else {
        error
    }
}

/// A plain .jar or .hnmod file name.
fn content_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.ends_with(".jar") || lower.ends_with(".hnmod"))
        && !name.contains(['/', '\\'])
        && !name.starts_with('.')
        && name.len() <= 120
}

/// GitHub's artifact at `link` into `out`: the archive link redirects to signed storage, which
/// must not get the token.
fn download(api: &Api, link: &str, out: &Path, max: u64) -> Result<(), String> {
    let direct = ureq::AgentBuilder::new()
        .https_only(true)
        .redirects(0)
        .timeout_connect(Duration::from_secs(20))
        .build();
    let first = api.request(&direct, "GET", link).call().map_err(failure)?;
    let response = match first.header("Location") {
        Some(location) if (300..400).contains(&first.status()) => {
            let location = location.to_owned();
            api.agent.get(&location).call().map_err(|e| e.to_string())?
        }
        _ => first,
    };
    let mut file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut response.into_reader().take(max + 1), &mut file)
        .map_err(|e| e.to_string())?;
    if copied > max {
        let _ = std::fs::remove_file(out);
        return Err("The download is too large".into());
    }
    Ok(())
}

/// The .hmod in the artifact, checked as an import is, into `mods` (beside one of the same id,
/// never over it); the answer is the report.
fn install_converted(archive: &Path, mods: &Path) -> Result<String, String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut report = String::new();
    let mut hmod = None;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        if name.contains('/') || !entry.is_file() {
            continue;
        }
        if name == "REPORT.md" {
            (&mut entry)
                .take(64 << 10)
                .read_to_string(&mut report)
                .map_err(|e| e.to_string())?;
        } else if name.ends_with(".hmod") && hmod.is_none() {
            let mut bytes = Vec::new();
            (&mut entry)
                .take(MAX_CONVERTED)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            hmod = Some(bytes);
        }
    }
    let report = report.trim().to_owned();
    let Some(bytes) = hmod else {
        return Err(if report.is_empty() {
            "The AI could not convert this mod".into()
        } else {
            format!("Not converted: {}", first_paragraph(&report))
        });
    };
    let staged = mods.join(".ai-convert.hmod.partial");
    std::fs::write(&staged, &bytes).map_err(|e| e.to_string())?;
    let id = match horizon_mods::Package::read(&staged) {
        Ok(package) => package.manifest.id,
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            return Err(format!("The converted mod is not valid: {e}"));
        }
    };
    let mut target = mods.join(format!("{id}.hmod"));
    if target.exists() {
        target = mods.join(format!("{id}-ai.hmod"));
    }
    std::fs::rename(&staged, &target).map_err(|e| e.to_string())?;
    Ok(format!(
        "Converted with AI: {}. {} Enable it in game settings.",
        target.file_name().unwrap_or_default().to_string_lossy(),
        first_paragraph(&report)
    ))
}

/// The report's first paragraph without Markdown headings, for one status line.
fn first_paragraph(report: &str) -> String {
    let text: String = report
        .split("\n\n")
        .map(|p| {
            p.lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .find(|p| !p.trim().is_empty())
        .unwrap_or_default();
    let text = text.trim();
    match text.char_indices().nth(280) {
        Some((at, _)) => format!("{}...", &text[..at]),
        None => text.to_owned(),
    }
}

fn extract_apk(archive: &Path, dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let index = (0..zip.len())
        .find(|i| {
            zip.by_index(*i)
                .is_ok_and(|e| e.is_file() && e.name().ends_with(".apk") && !e.name().contains('/'))
        })
        .ok_or("The build's download has no APK")?;
    let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
    let partial = dir.join(format!("{APK_FILE}.partial"));
    let mut out = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    std::io::copy(&mut (&mut entry).take(MAX_APK), &mut out).map_err(|e| e.to_string())?;
    out.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(&partial, dir.join(APK_FILE)).map_err(|e| e.to_string())
}
