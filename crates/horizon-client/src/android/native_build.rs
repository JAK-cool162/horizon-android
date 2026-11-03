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
