//! Local add-on files. Imports remain archives: no untrusted path is extracted.
use std::{
    collections::HashSet,
    io::{self, Read},
    path::{Component, Path},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mods,
    /// Source-only native mixin bundles (`.hnmod`). They are kept, never run:
    /// a native mod plays only in a build compiled with it.
    NativeMods,
    Packs,
    Shaders,
}
impl Kind {
    pub const ALL: [Self; 4] = [Self::Mods, Self::NativeMods, Self::Packs, Self::Shaders];
    pub fn folder(self) -> &'static str {
        match self {
            Self::Mods => "horizon-mods",
            // The desktop launcher's instance folder for registry `.hnmod`s.
            Self::NativeMods => "horizon-native-mods",
            Self::Packs => "resourcepacks",
            Self::Shaders => "shaderpacks",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Mods => "Mods",
            Self::NativeMods => "Native Mods",
            Self::Packs => "Resource Packs",
            Self::Shaders => "Shaders",
        }
    }
    pub fn accepts(self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        match self {
            Self::Mods => lower.ends_with(".hmod"),
            Self::NativeMods => lower.ends_with(".hnmod"),
            Self::Packs => lower.ends_with(".zip"),
            Self::Shaders => lower.ends_with(".zip") || lower.ends_with(".hshader"),
        }
    }
}

/// What this build can do with a native mod (`.hnmod`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeStatus {
    /// Compiled into this build at this version: it plays.
    BuiltIn,
    /// Compiled into this build at another version.
    OtherVersion(String),
    /// The fixed Android copy of this id is compiled in; the file is kept but unused.
    Bundled,
    /// Plays only in a build compiled with it.
    NeedsBuild,
    /// Made for another game, or for hook points this source does not have.
    Incompatible(String),
}
impl NativeStatus {
    pub fn label(&self) -> String {
        match self {
            Self::BuiltIn => "Built in".into(),
            Self::OtherVersion(v) => format!("Build has {v}"),
            Self::Bundled => "Bundled copy".into(),
            Self::NeedsBuild => "Needs build".into(),
            Self::Incompatible(why) => why.clone(),
        }
    }
    pub fn plays(&self) -> bool {
        matches!(self, Self::BuiltIn)
    }
}

/// The manifest of the native mod at `path`, read alone (listing, not importing).
pub fn native_manifest(path: &Path) -> Result<horizon_mods::index::native::Manifest, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let entry = zip
        .by_name(horizon_mods::index::native::MANIFEST)
        .map_err(|_| "missing horizon-native.json".to_string())?;
    let mut bytes = Vec::new();
    entry
        .take(64 << 10)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let manifest: horizon_mods::index::native::Manifest =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    manifest.check()?;
    Ok(manifest)
}

/// Whether the native mod `id` at `path` cannot play in an APK on any source: it has
/// post-build steps (they run in a desktop version folder) or adds a module to the desktop
/// binary's own files, which the Android library does not compile. A conservative reading
/// of its `horizon-mixin.toml`; `tools/android_native_mods.py` parses it.
pub fn native_desktop_only(path: &Path, id: &str) -> bool {
    let read = || -> Option<String> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
        let entry = zip
            .by_name(&format!("mixins/{id}/horizon-mixin.toml"))
            .ok()?;
        let mut text = String::new();
        entry.take(1 << 20).read_to_string(&mut text).ok()?;
        Some(text)
    };
    read().is_some_and(|text| desktop_only_mixin(&text))
}
fn desktop_only_mixin(text: &str) -> bool {
    text.lines().any(|line| {
        let key: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        key.starts_with("[[post_build]]")
            || key.starts_with("post_build=")
            || key.contains("\"crates/horizon-client/src/main.rs\"")
            || key.contains("\"crates/horizon-client/src/bin/")
    })
}

/// The native mods a build was compiled with: `id@version` entries separated by commas
/// (`HORIZON_ANDROID_NATIVE_MODS`, set by `tools/android_native_mods.py`).
pub fn built_native_mods(list: &str) -> Vec<(&str, &str)> {
    list.split(',')
        .filter_map(|e| e.trim().split_once('@'))
        .filter(|(id, version)| !id.is_empty() && !version.is_empty())
        .collect()
}

/// Whether `manifest` plays in a build of `game` whose native hook points are version `hooks`,
/// compiled with the native mods `built` and the fixed Android mods `bundled`.
pub fn native_status(
    manifest: &horizon_mods::index::native::Manifest,
    built: &[(&str, &str)],
    bundled: &[&str],
    game: &str,
    hooks: u32,
) -> NativeStatus {
    if let Some((_, version)) = built.iter().find(|(id, _)| *id == manifest.id) {
        return if *version == manifest.version {
            NativeStatus::BuiltIn
        } else {
            NativeStatus::OtherVersion((*version).to_owned())
        };
    }
    if bundled.contains(&manifest.id.as_str()) {
        return NativeStatus::Bundled;
    }
    if manifest.game != game {
        return NativeStatus::Incompatible(format!("For Minecraft {}", manifest.game));
    }
    if manifest.hooks.is_some_and(|needed| needed > hooks) {
        return NativeStatus::Incompatible("Needs newer Horizon".into());
    }
    NativeStatus::NeedsBuild
}
pub const MAX_INPUT: u64 = 256 << 20;
const MAX_EXPANDED: u64 = 512 << 20;
pub fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 180
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.chars().any(char::is_control)
        && matches!(
            Path::new(name).components().next(),
            Some(Component::Normal(_))
        )
        && Path::new(name).components().count() == 1
}
pub fn validate(
    path: &Path,
    kind: Kind,
    name: &str,
    cancelled: impl Fn() -> bool,
) -> crate::Result<bool> {
    if !plain_name(name) || !kind.accepts(name) {
        return Err(
            "Choose a ZIP pack, .hshader shader pack, sandboxed .hmod or native .hnmod matching this folder"
                .into(),
        );
    }
    let file = std::fs::File::open(path)?;
    if kind == Kind::Mods && file.metadata()?.len() > 32 << 20 {
        return Err("Horizon mod exceeds 32 MiB".into());
    }
    if kind == Kind::NativeMods {
        // Rust and TOML sources for one mod id, checked as the registry checks them.
        // Nothing in the bundle runs here.
        let bundle = horizon_mods::index::native::Bundle::read(path)
            .map_err(|e| format!("Invalid native mod: {e}"))?;
        if !plain_name(&format!("{}.hnmod", bundle.manifest.id)) {
            return Err("Native mod has an invalid ID".into());
        }
        return Ok(false);
    }
    if file.metadata()?.len() > MAX_INPUT {
        return Err("Import exceeds 256 MiB".into());
    }
    let mut zip = zip::ZipArchive::new(file)?;
    if zip.len() > 32768 {
        return Err("Archive contains too many files".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    let mut pack = false;
    let mut shader = false;
    let mut native_shader = false;
    let mut manifest = false;
    let mut buffer = [0u8; 65536];
    for i in 0..zip.len() {
        if cancelled() {
            return Err("Import cancelled".into());
        }
        let mut entry = zip.by_index(i)?;
        let raw = entry.name()?.into_owned();
        if raw.is_empty()
            || raw.len() > 1024
            || raw.chars().any(char::is_control)
            || raw.contains(['\\', ':', '\0'])
            || raw.starts_with('/')
            || raw.split('/').any(|p| p == ".." || p == ".")
            || !names.insert(raw.clone())
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Archive contains an unsafe or duplicate path".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Archive size overflow")?;
        if total > MAX_EXPANDED || kind == Kind::Mods && total > 64 << 20 {
            return Err("Expanded archive exceeds 512 MiB".into());
        }
        pack |= raw == "pack.mcmeta";
        native_shader |= raw == "horizon-shaders.toml";
        shader |= native_shader
            || raw.starts_with("shaders/")
                && !entry.is_dir()
                && ["vsh", "fsh", "glsl", "csh", "gsh", "tcs", "tes"]
                    .iter()
                    .any(|ext| raw.ends_with(&format!(".{ext}")));
        manifest |= raw == "horizon-mod.toml";
        let expected = entry.size();
        if raw == "pack.mcmeta" && expected > 1 << 20 {
            return Err("Pack metadata is too large".into());
        }
        let mut metadata = Vec::new();
        let mut actual = 0u64;
        while !entry.is_dir() {
            if cancelled() {
                return Err("Import cancelled".into());
            }
            let n = entry.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            actual += n as u64;
            if raw == "pack.mcmeta" {
                metadata.extend_from_slice(&buffer[..n]);
            }
            if actual > expected || actual > MAX_EXPANDED {
                return Err("Archive size mismatch".into());
            }
        }
        if !entry.is_dir() && actual != expected {
            return Err("Incomplete archive entry".into());
        }
        if raw == "pack.mcmeta"
            && !serde_json::from_slice::<serde_json::Value>(&metadata)?
                .get("pack")
                .is_some_and(|v| v.is_object())
        {
            return Err("Resource pack metadata has no pack object".into());
        }
    }
    if !match kind {
        Kind::Packs => pack,
        Kind::Shaders => shader,
        Kind::Mods => manifest,
        Kind::NativeMods => true,
    } {
        return Err(format!(
            "Archive is not a {} package (required files must be at the ZIP root)",
            kind.label()
        )
        .into());
    }
    if kind == Kind::Mods {
        // Accept both sandboxed code and upstream content-only packages. The
        // shared parser requires mod.wasm unless the manifest grants registries.
        horizon_mods::Package::read(path).map_err(|e| format!("Invalid Horizon mod: {e}"))?;
    }
    Ok(kind == Kind::Shaders && native_shader)
}
pub fn copy_bounded(
    input: &mut impl Read,
    output: &mut impl io::Write,
    mut progress: impl FnMut(u64) -> bool,
) -> io::Result<u64> {
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        if !progress(total) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Import cancelled",
            ));
        }
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_INPUT {
            return Err(io::Error::other("Import exceeds 256 MiB"));
        }
        output.write_all(&buffer[..n])?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn validates_structure_paths_metadata_and_cancellation() {
        let dir = std::env::temp_dir().join(format!(
            "horizon-import-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("stage.part");
        let metadata = br#"{"pack":{"pack_format":1,"description":"test"}}"#;
        std::fs::write(
            &path,
            archive(&[("pack.mcmeta", metadata), ("assets/test.png", b"png")]),
        )
        .unwrap();
        assert_eq!(
            validate(&path, Kind::Packs, "Test.zip", || false).unwrap(),
            false
        );
        assert!(validate(&path, Kind::Packs, "Test.zip", || true).is_err());
        assert!(validate(&path, Kind::Packs, "../Test.zip", || false).is_err());
        for name in ["../outside", "/absolute", "C:/outside", "folder\\outside"] {
            std::fs::write(&path, archive(&[("pack.mcmeta", metadata), (name, b"bad")])).unwrap();
            assert!(
                validate(&path, Kind::Packs, "Test.zip", || false).is_err(),
                "{name}"
            );
        }
        std::fs::write(&path, archive(&[("pack.mcmeta", b"not JSON")])).unwrap();
        assert!(validate(&path, Kind::Packs, "Test.zip", || false).is_err());
        std::fs::write(&path, archive(&[("shaders/test.fsh", b"shader")])).unwrap();
        assert!(validate(&path, Kind::Shaders, "Test.zip", || false).is_ok());
        assert!(validate(&path, Kind::Packs, "Test.zip", || false).is_err());
        std::fs::write(&path, archive(&[("horizon-shaders.toml", b"manifest")])).unwrap();
        assert!(validate(&path, Kind::Shaders, "Test.zip", || false).unwrap());
        std::fs::write(&path, archive(&[("horizon-mod.toml", b"manifest")])).unwrap();
        assert!(validate(&path, Kind::Mods, "Test.hmod", || false).is_err());
        let content = b"id='test_content'\nname='Content'\nversion='1'\napi='0.2'\ncapabilities=['registries']\n";
        std::fs::write(&path, archive(&[("horizon-mod.toml", content)])).unwrap();
        assert!(validate(&path, Kind::Mods, "Content.hmod", || false).is_ok());
        let code = b"id='test_code'\nname='Code'\nversion='1'\napi='0.1'\ncapabilities=['hud']\n";
        std::fs::write(&path, archive(&[("horizon-mod.toml", code)])).unwrap();
        assert!(validate(&path, Kind::Mods, "Code.hmod", || false).is_err());
        std::fs::write(
            &path,
            archive(&[("horizon-mod.toml", code), ("mod.wasm", b"wasm")]),
        )
        .unwrap();
        assert!(validate(&path, Kind::Mods, "Code.hmod", || false).is_ok());
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "link",
                "../outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        std::fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
        assert!(validate(&path, Kind::Packs, "Test.zip", || false).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    const REV: &str = "0123456789abcdef0123456789abcdef01234567";
    fn native(id: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
        let manifest = format!(
            r#"{{"schema":1,"id":"{id}","version":"1.0.0","game":"26.3","horizon":"{REV}","mixins":"{REV}","hooks":1}}"#
        );
        let mixin = format!(
            "[mod]\nid = \"{id}\"\nversion = \"1.0.0\"\n[compatibility]\nhorizon_revision = \"{REV}\"\n"
        );
        let toml = format!("mixins/{id}/horizon-mixin.toml");
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("horizon-native.json", manifest.as_bytes()),
            (&toml, mixin.as_bytes()),
        ];
        entries.extend_from_slice(extra);
        archive(&entries)
    }
    #[test]
    fn native_mods_are_checked_without_running_them() {
        let dir = std::env::temp_dir().join(format!(
            "horizon-native-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("stage.part");
        std::fs::write(
            &path,
            native("peek", &[("mixins/peek/hook.rs", b"fn hook() {}")]),
        )
        .unwrap();
        assert_eq!(
            validate(&path, Kind::NativeMods, "peek-1.0.0.hnmod", || false).unwrap(),
            false
        );
        // The wrong folder or extension, a script beside the sources, another mod's files.
        assert!(validate(&path, Kind::NativeMods, "peek.zip", || false).is_err());
        assert!(validate(&path, Kind::Mods, "peek.hnmod", || false).is_err());
        for extra in ["mixins/peek/build.sh", "mixins/other/hook.rs", "../hook.rs"] {
            std::fs::write(&path, native("peek", &[(extra, b"echo")])).unwrap();
            assert!(
                validate(&path, Kind::NativeMods, "peek.hnmod", || false).is_err(),
                "{extra}"
            );
        }
        std::fs::write(&path, archive(&[("horizon-native.json", b"{}")])).unwrap();
        assert!(validate(&path, Kind::NativeMods, "peek.hnmod", || false).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn native_status_says_whether_this_build_plays_it() {
        let manifest =
            |id: &str, game: &str, hooks: Option<u32>| horizon_mods::index::native::Manifest {
                schema: 1,
                id: id.into(),
                version: "1.0.0".into(),
                game: game.into(),
                horizon: REV.into(),
                mixins: REV.into(),
                hooks,
            };
        let built = built_native_mods(" peek@1.0.0, ,broken,zoom@2.0.0,@1");
        assert_eq!(built, [("peek", "1.0.0"), ("zoom", "2.0.0")]);
        let status = |m| native_status(&m, &built, &["minimap"], "26.3", 80);
        assert_eq!(
            status(manifest("peek", "26.3", Some(1))),
            NativeStatus::BuiltIn
        );
        assert_eq!(
            status(manifest("zoom", "26.3", Some(1))),
            NativeStatus::OtherVersion("2.0.0".into())
        );
        assert_eq!(
            status(manifest("minimap", "26.3", None)),
            NativeStatus::Bundled
        );
        assert_eq!(
            status(manifest("sort", "26.3", Some(80))),
            NativeStatus::NeedsBuild
        );
        assert_eq!(
            status(manifest("sort", "26.3", None)),
            NativeStatus::NeedsBuild
        );
        assert!(matches!(
            status(manifest("sort", "26.3", Some(81))),
            NativeStatus::Incompatible(_)
        ));
        assert!(matches!(
            status(manifest("sort", "26.2", Some(1))),
            NativeStatus::Incompatible(_)
        ));
        assert!(NativeStatus::BuiltIn.plays() && !NativeStatus::NeedsBuild.plays());
    }
    #[test]
    fn desktop_only_native_mods_are_recognised() {
        let module = |file: &str| {
            format!("[[module]]\nid = \"m\"\nfile = \"{file}\"\nname = \"m\"\nsource = \"m.rs\"\n")
        };
        assert!(desktop_only_mixin(
            "[mod]\nid = \"a\"\n[[ post_build ]]\ncrate = \"e\"\n"
        ));
        assert!(desktop_only_mixin(
            "post_build = [{ crate = \"e\", bin = \"b\" }]\n"
        ));
        assert!(desktop_only_mixin(&module(
            "crates/horizon-client/src/main.rs"
        )));
        assert!(desktop_only_mixin(&module(
            "crates/horizon-client/src/bin/horizon-ctl/main.rs"
        )));
        assert!(!desktop_only_mixin(&module(
            "crates/horizon-client/src/game.rs"
        )));
        assert!(!desktop_only_mixin("[mod]\nid = \"post_build_helper\"\n"));
    }
    #[test]
    fn streamed_copy_is_bounded_and_cancellable() {
        let mut out = Vec::new();
        assert_eq!(
            copy_bounded(&mut Cursor::new(b"test"), &mut out, |_| true).unwrap(),
            4
        );
        assert_eq!(out, b"test");
        assert!(copy_bounded(&mut io::repeat(0), &mut io::sink(), |_| true).is_err());
        assert!(copy_bounded(&mut Cursor::new(b"test"), &mut io::sink(), |_| false).is_err());
    }
}
