//! What an in-app native build asks GitHub for and reads back: no network, no JNI, so the
//! host content tests cover it (`tools/test_android_content.py`).
use serde_json::Value;

/// The branch of the player's fork the app uploads `.hnmod` files to.
pub const MODS_BRANCH: &str = "native-mods";
/// The branch the Android dev workflow builds.
pub const BASE_BRANCH: &str = "android";
/// The Android dev workflow, which compiles the uploaded mods in (`native_mods` input).
pub const WORKFLOW: &str = "android-dev.yml";
/// The one-time workflow that makes the dev signing key on GitHub's runner.
pub const KEY_WORKFLOW: &str = "android-dev-key.yml";
/// The secret the dev workflow signs with, and the short-lived one the key workflow reads.
pub const SIGNING_SECRET: &str = "ANDROID_DEV_KEYSTORE_BASE64";
pub const SETUP_SECRET: &str = "DEV_KEY_SETUP_TOKEN";

/// Whether `text` looks like a GitHub token (fine-grained `github_pat_` or classic `ghp_`):
/// what Paste accepts from the clipboard.
pub fn token(text: &str) -> Option<&str> {
    let text = text.trim();
    ((text.starts_with("github_pat_") || text.starts_with("ghp_"))
        && (20..=255).contains(&text.len())
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    .then_some(text)
}

/// GitHub's new fine-grained token page, filled in where GitHub reads the parameters: a name,
/// the fork's owner, and the three permissions a build needs.
pub fn token_page(repo: Option<&str>) -> String {
    let mut url =
        "https://github.com/settings/personal-access-tokens/new?name=Horizon%20Android%20builds\
        &description=Builds%20APKs%20with%20native%20mods&expires_in=none\
        &contents=write&actions=write&secrets=write"
            .to_owned();
    if let Some((owner, _)) = repo.and_then(|r| r.split_once('/')) {
        url.push_str("&target_name=");
        url.push_str(owner);
    }
    url
}

/// `owner/name` of a GitHub repository, as GitHub allows them.
pub fn repository(text: &str) -> Option<String> {
    let text = text.trim().trim_end_matches('/');
    let text = text
        .strip_prefix("https://github.com/")
        .unwrap_or(text)
        .trim_end_matches(".git");
    let (owner, name) = text.split_once('/')?;
    let owner_ok = !owner.is_empty()
        && owner.len() <= 39
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let name_ok = !name.is_empty()
        && name.len() <= 100
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    (owner_ok && name_ok).then(|| format!("{owner}/{name}"))
}

/// The path a mod file gets in [`MODS_BRANCH`]; `None` for a name the workflow would refuse.
pub fn upload_path(file: &str) -> Option<String> {
    let id = file.strip_suffix(".hnmod")?;
    (!id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'))
    .then(|| format!("{MODS_BRANCH}/{file}"))
}

/// The Android ABI this APK runs as: the artifact to install.
pub fn abi() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64-v8a"
    } else if cfg!(target_arch = "arm") {
        "armeabi-v7a"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x86_64"
    }
}

/// The workflow dispatch: the uploaded paths, a tag that names the run, and every ABI when
/// this APK is not the default arm64 build.
pub fn dispatch(paths: &[String], request: &str, abi: &str) -> Value {
    serde_json::json!({
        "ref": BASE_BRANCH,
        "inputs": {
            "native_mods": paths.join(" "),
            "request": request,
            "all_abis": if abi == "arm64-v8a" { "false" } else { "true" },
        }
    })
}

/// The run a dispatch started: its run name carries `request` (`run-name` in the workflow).
pub fn find_run(runs: &Value, request: &str) -> Option<(u64, String)> {
    runs["workflow_runs"].as_array()?.iter().find_map(|run| {
        let title = run["display_title"].as_str()?;
        (title.contains(request)).then(|| {
            (
                run["id"].as_u64().unwrap_or(0),
                run["html_url"].as_str().unwrap_or_default().to_owned(),
            )
        })
    })
}

/// Where a run is: `Ok(None)` while it runs, `Ok(Some(()))` when it succeeded.
pub fn run_state(run: &Value) -> Result<Option<()>, String> {
    match (run["status"].as_str(), run["conclusion"].as_str()) {
        (Some("completed"), Some("success")) => Ok(Some(())),
        (Some("completed"), conclusion) => Err(format!(
            "The build {} on GitHub: open it for the log",
            match conclusion {
                Some("cancelled") => "was cancelled",
                Some("timed_out") => "timed out",
                _ => "failed",
            }
        )),
        _ => Ok(None),
    }
}

/// The APK artifact for `abi`: its download link and size.
pub fn artifact(artifacts: &Value, abi: &str) -> Option<(String, u64)> {
    artifacts["artifacts"].as_array()?.iter().find_map(|a| {
        let name = a["name"].as_str()?;
        (name.ends_with(&format!("-{abi}")) && a["expired"].as_bool() != Some(true)).then(|| {
            (
                a["archive_download_url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                a["size_in_bytes"].as_u64().unwrap_or(0),
            )
        })
    })
}

/// Standard base64 decoding (GitHub's secrets public key).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let text = text.trim().trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes() {
        acc = acc << 6 | u32::from(value(c)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Standard base64 with padding (the contents API's encoding).
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                TABLE[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn repositories_and_upload_paths_are_strict() {
        assert_eq!(
            repository(" JAK-cool162/horizon-android "),
            Some("JAK-cool162/horizon-android".into())
        );
        assert_eq!(
            repository("https://github.com/JAK-cool162/horizon-android.git"),
            Some("JAK-cool162/horizon-android".into())
        );
        for bad in [
            "",
            "owner",
            "a/b/c",
            "own er/repo",
            "owner/..",
            "../repo",
            "owner/re?po",
        ] {
            assert_eq!(repository(bad), None, "{bad}");
        }
        assert_eq!(
            upload_path("shulker_peek.hnmod"),
            Some("native-mods/shulker_peek.hnmod".into())
        );
        for bad in [
            "peek.hmod",
            ".hnmod",
            "../x.hnmod",
            "Peek.hnmod",
            "a b.hnmod",
        ] {
            assert_eq!(upload_path(bad), None, "{bad}");
        }
    }
    #[test]
    fn a_dispatch_names_its_run_and_finds_its_apk() {
        let body = dispatch(
            &["native-mods/a.hnmod".into(), "native-mods/b.hnmod".into()],
            "req-7",
            "arm64-v8a",
        );
        assert_eq!(body["ref"], "android");
        assert_eq!(
            body["inputs"]["native_mods"],
            "native-mods/a.hnmod native-mods/b.hnmod"
        );
        assert_eq!(body["inputs"]["all_abis"], "false");
        assert_eq!(
            dispatch(&[], "r", "armeabi-v7a")["inputs"]["all_abis"],
            "true"
        );
        let runs = json!({"workflow_runs": [
            {"id": 1, "display_title": "Android dev · abc", "html_url": "u1"},
            {"id": 2, "display_title": "Android dev · req-7", "html_url": "u2"}]});
        assert_eq!(find_run(&runs, "req-7"), Some((2, "u2".into())));
        assert_eq!(find_run(&runs, "req-8"), None);
        assert_eq!(run_state(&json!({"status": "in_progress"})), Ok(None));
        assert_eq!(
            run_state(&json!({"status": "completed", "conclusion": "success"})),
            Ok(Some(()))
        );
        assert!(run_state(&json!({"status": "completed", "conclusion": "failure"})).is_err());
        let artifacts = json!({"artifacts": [
            {"name": "horizon-dev-3-armeabi-v7a", "archive_download_url": "v7", "size_in_bytes": 5},
            {"name": "horizon-dev-3-arm64-v8a", "archive_download_url": "v8", "size_in_bytes": 9, "expired": false}]});
        assert_eq!(artifact(&artifacts, "arm64-v8a"), Some(("v8".into(), 9)));
        assert_eq!(artifact(&artifacts, "x86"), None);
    }
    #[test]
    fn pasted_tokens_and_the_token_page() {
        assert_eq!(
            token("  github_pat_11ABCDEFG0123456789_abcdefXYZ\n"),
            Some("github_pat_11ABCDEFG0123456789_abcdefXYZ")
        );
        assert_eq!(
            token("ghp_0123456789abcdefABCDEF0123456789"),
            Some("ghp_0123456789abcdefABCDEF0123456789")
        );
        for bad in [
            "",
            "hello world",
            "github_pat_",
            "gho_0123456789abcdef0123",
            "github_pat_abc def ghijklmnop",
        ] {
            assert_eq!(token(bad), None, "{bad}");
        }
        let page = token_page(Some("JAK-cool162/horizon-android"));
        assert!(page.starts_with("https://github.com/settings/personal-access-tokens/new?"));
        assert!(page.contains("&secrets=write") && page.ends_with("&target_name=JAK-cool162"));
        assert!(!page.contains(' '));
    }
    #[test]
    fn base64_matches_the_standard_vectors() {
        // RFC 4648 section 10.
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), encoded);
            assert_eq!(base64_decode(encoded).as_deref(), Some(plain.as_bytes()));
        }
        assert_eq!(base64_decode("Zm9v!"), None);
    }
}
