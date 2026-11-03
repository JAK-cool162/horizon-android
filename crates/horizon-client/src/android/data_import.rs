//! Copies worlds, add-ons and settings out of another Horizon Android app (the official
//! release, or another build) through that app's own Files provider: the player picks its
//! folder in Android's picker, and only what that one grant shows is read. Nothing is
//! overwritten but settings, which keep a `.before-import` copy.
use crate::import_plan::{Counts, Source};
use jni::{
    JNIEnv,
    objects::{JObject, JString, JValue},
};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

/// The picker's request code; `HorizonActivity` forwards its result here.
pub const REQUEST: i32 = 4102;
const MAX_DEPTH: usize = 32;
const MAX_FILES: usize = 1_000_000;

/// What the picked folder is read for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Worlds, settings and add-ons of another Horizon app.
    OtherApp,
    /// Minecraft's own files in a `.minecraft` (a Java launcher's, or a copy), for the installer.
    Minecraft,
}

/// The pending pick: its mode, this profile's folder and the app's folder.
static TARGET: Mutex<Option<(Mode, PathBuf, PathBuf)>> = Mutex::new(None);
static WORKING: AtomicBool = AtomicBool::new(false);

pub fn busy() -> bool {
    WORKING.load(Ordering::Acquire)
}

/// Opens Android's folder picker: `mode` says what is read, into the profile at `game` of the
/// app whose folder is `home`.
pub fn begin(mode: Mode, game: PathBuf, home: PathBuf) -> Result<(), String> {
    if busy() {
        return Err("An import is running: wait for it to finish".into());
    }
    *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = Some((mode, game, home));
    if !crate::native_ime::pick_tree() {
        return Err("Android's folder picker is unavailable".into());
    }
    crate::documents::notify(
        match mode {
            Mode::OtherApp => {
                "Choose the other app's Horizon Android folder (or its Worlds), then Use this folder"
            }
            Mode::Minecraft => {
                "Choose a folder with Minecraft in it (a launcher's, or a .minecraft), then Use this folder"
            }
        },
        false,
    );
    Ok(())
}

/// The picker's answer: a tree URI granted to this app, or nothing.
pub fn tree_result(env: &mut JNIEnv<'_>, activity: &JObject<'_>, result: i32, uri: &JString<'_>) {
    let Some((mode, game, home)) = TARGET.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    if result != -1 || uri.is_null() {
        crate::documents::notify("Import cancelled", false);
        return;
    }
    let setup = (|| -> jni::errors::Result<_> {
        let text: String = env.get_string(uri)?.into();
        Ok((env.get_java_vm()?, env.new_global_ref(activity)?, text))
    })();
    let Ok((vm, activity, uri)) = setup else {
        let _ = env.exception_clear();
        crate::documents::notify("Could not read the chosen folder", false);
        return;
    };
    WORKING.store(true, Ordering::Release);
    let spawned = std::thread::Builder::new()
        .name("Data import".into())
        .spawn(move || {
            let result = vm
                .attach_current_thread()
                .map_err(|e| e.to_string())
                .and_then(|mut env| match mode {
                    Mode::OtherApp => {
                        run(&mut env, activity.as_obj(), &uri, &game).map(|c| c.summary())
                    }
                    Mode::Minecraft => minecraft(&mut env, activity.as_obj(), &uri, &home),
                });
            WORKING.store(false, Ordering::Release);
            match result {
                Ok(summary) => crate::documents::notify(summary, true),
                Err(error) => crate::documents::notify(format!("Import failed: {error}"), true),
            }
        });
    if spawned.is_err() {
        WORKING.store(false, Ordering::Release);
        crate::documents::notify("Import failed: no worker thread", false);
    }
}

struct Entry {
    id: String,
    name: String,
    folder: bool,
}

/// The tree's resolver and URI, and this app's own provider (refused as a source).
struct Tree<'a> {
    resolver: JObject<'a>,
    uri: JObject<'a>,
}

/// The granted tree, its root document and its provider's authority.
fn open_tree<'a>(
    env: &mut JNIEnv<'a>,
    activity: &JObject<'_>,
    uri: &str,
) -> Result<(Tree<'a>, String, String), String> {
    let jni = |e: jni::errors::Error| e.to_string();
    let resolver = env
        .call_method(
            activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .and_then(|v| v.l())
        .map_err(jni)?;
    let text = JObject::from(env.new_string(uri).map_err(jni)?);
    let uri = env
        .call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&text)],
        )
        .and_then(|v| v.l())
        .map_err(jni)?;
    let authority = string(env, &uri, "getAuthority")?;
    let root: String = {
        let id = env
            .call_static_method(
                "android/provider/DocumentsContract",
                "getTreeDocumentId",
                "(Landroid/net/Uri;)Ljava/lang/String;",
                &[JValue::Object(&uri)],
            )
            .and_then(|v| v.l())
            .map_err(jni)?;
        env.get_string((&id).into()).map_err(jni)?.into()
    };
    Ok((Tree { resolver, uri }, root, authority))
}

fn run(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    game: &Path,
) -> Result<Counts, String> {
    let (tree, root, authority) = open_tree(env, activity, uri)?;
    let package = string(env, activity, "getPackageName")?;
    if authority == format!("{package}.documents") {
        return Err("that is this app's own folder: choose the other Horizon app".into());
    }
    let mut copier = Copier {
        counts: Counts::default(),
        files: 0,
        shown: Instant::now(),
    };
    // A folder the player picked itself, else the folders of a picked app or profile.
    let picked: Vec<(Source, String)> = match Source::of(&root) {
        Some(source) => vec![(source, root)],
        None => list(env, &tree, &root)?
            .into_iter()
            .filter(|e| e.folder)
            .filter_map(|e| Source::of(&e.id).map(|s| (s, e.id)))
            .collect(),
    };
    for (source, id) in picked {
        copier.source(env, &tree, source, &id, game)?;
    }
    Ok(copier.counts)
}

/// Minecraft's files in the picked folder, put where the installer looks before downloading.
fn minecraft(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    uri: &str,
    home: &Path,
) -> Result<String, String> {
    let (tree, root, _) = open_tree(env, activity, uri)?;
    crate::documents::notify("Asking Mojang which files Minecraft 26.3 needs...", false);
    let wanted = horizon_client::official_install::wanted(home).map_err(|e| e.to_string())?;
    let mut seeder = Seeder {
        wanted: &wanted,
        copied: 0,
        assets: 0,
        client: false,
        bytes: 0,
        shown: Instant::now(),
    };
    // The folder itself, or a `.minecraft` (or launcher folder) a few levels down.
    let mut queue = vec![(root, 0usize)];
    let mut listed = 0;
    let mut found = 0;
    while let Some((id, depth)) = queue.pop() {
        listed += 1;
        if listed > 400 {
            break;
        }
        let children = list(env, &tree, &id)?;
        let assets = children.iter().find(|e| e.folder && e.name == "assets");
        let versions = children.iter().find(|e| e.folder && e.name == "versions");
        if assets.is_some() || versions.is_some() {
            found += 1;
            if let Some(versions) = versions {
                seeder.client(env, &tree, &versions.id)?;
            }
            if let Some(assets) = assets {
                seeder.assets(env, &tree, &assets.id)?;
            }
            continue;
        }
        if depth < 4 {
            for child in children.into_iter().filter(|e| e.folder) {
                queue.push((child.id, depth + 1));
            }
        }
    }
    if found == 0 {
        return Ok(
            "No Minecraft there: choose a .minecraft folder, or the launcher folder that holds one"
                .into(),
        );
    }
    let mut text = format!(
        "Found Minecraft: {} of {} game assets{} ({} MiB) copied.",
        seeder.assets,
        wanted.objects.len(),
        if seeder.client {
            " and the 26.3 game"
        } else {
            ""
        },
        seeder.bytes >> 20,
    );
    text.push_str(if seeder.client && seeder.assets == wanted.objects.len() {
        " Tap Download Minecraft: nothing is left to download."
    } else {
        " Tap Download Minecraft: only the rest is downloaded."
    });
    Ok(text)
}

/// Copies the files the installer wants into its cache, each checked against Mojang's SHA-1.
struct Seeder<'w> {
    wanted: &'w horizon_client::official_install::Wanted,
    copied: usize,
    assets: usize,
    client: bool,
    bytes: u64,
    shown: Instant,
}

impl Seeder<'_> {
    /// `versions/26.3/26.3.jar`.
    fn client(
        &mut self,
        env: &mut JNIEnv<'_>,
        tree: &Tree<'_>,
        versions: &str,
    ) -> Result<(), String> {
        let version = horizon_core::VANILLA_VERSION;
        let Some(folder) = list(env, tree, versions)?
            .into_iter()
            .find(|e| e.folder && e.name == version)
        else {
            return Ok(());
        };
        let jar = format!("{version}.jar");
        if let Some(file) = list(env, tree, &folder.id)?
            .into_iter()
            .find(|e| !e.folder && e.name == jar)
        {
            let (sha1, size) = self.wanted.client.clone();
            let target = self.wanted.cache.join("client.jar");
            if self.copy(env, tree, &file.id, &target, &sha1, size)? {
                self.client = true;
            }
        }
        Ok(())
    }
    /// `assets/objects/<2 hex>/<sha1>`: the ones this version uses.
    fn assets(
        &mut self,
        env: &mut JNIEnv<'_>,
        tree: &Tree<'_>,
        assets: &str,
    ) -> Result<(), String> {
        let Some(objects) = list(env, tree, assets)?
            .into_iter()
            .find(|e| e.folder && e.name == "objects")
        else {
            return Ok(());
        };
        let objects_dir = self.wanted.cache.join("objects");
        std::fs::create_dir_all(&objects_dir).map_err(|e| e.to_string())?;
        for bucket in list(env, tree, &objects.id)?
            .into_iter()
            .filter(|e| e.folder)
        {
            if bucket.name.len() != 2 {
                continue;
            }
            for file in list(env, tree, &bucket.id)? {
                let Some(&size) = self.wanted.objects.get(&file.name) else {
                    continue;
                };
                let target = objects_dir.join(&file.name);
                if self.copy(env, tree, &file.id, &target, &file.name.clone(), size)? {
                    self.assets += 1;
                }
            }
        }
        Ok(())
    }
    /// One file into `target` if its size and SHA-1 are Mojang's; true when it is there now.
    fn copy(
        &mut self,
        env: &mut JNIEnv<'_>,
        tree: &Tree<'_>,
        id: &str,
        target: &Path,
        sha1: &str,
        size: u64,
    ) -> Result<bool, String> {
        use sha1::{Digest, Sha1};
        if std::fs::metadata(target).is_ok_and(|m| m.len() == size) {
            // The installer checks it again before using it.
            return Ok(true);
        }
        let mut input = open(env, tree, id)?;
        let partial = target.with_extension("import-partial");
        let mut output = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut digest = Sha1::new();
        let mut buffer = vec![0u8; 1 << 16];
        let mut count = 0u64;
        loop {
            let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > size {
                break;
            }
            digest.update(&buffer[..n]);
            output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        }
        drop(output);
        let matches = count == size
            && digest
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                == sha1;
        if !matches {
            let _ = std::fs::remove_file(&partial);
            return Ok(false);
        }
        std::fs::rename(&partial, target).map_err(|e| e.to_string())?;
        self.copied += 1;
        self.bytes += size;
        if self.shown.elapsed().as_millis() >= 700 {
            self.shown = Instant::now();
            crate::documents::notify(
                format!(
                    "Copying Minecraft files: {} files, {} MiB...",
                    self.copied,
                    self.bytes >> 20
                ),
                false,
            );
        }
        Ok(true)
    }
}

fn string(env: &mut JNIEnv<'_>, object: &JObject<'_>, method: &str) -> Result<String, String> {
    let value = env
        .call_method(object, method, "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
        .map_err(|e| e.to_string())?;
    if value.is_null() {
        return Ok(String::new());
    }
    Ok(env
        .get_string((&value).into())
        .map_err(|e| e.to_string())?
        .into())
}

/// The children of document `parent` in the granted tree.
fn list(env: &mut JNIEnv<'_>, tree: &Tree<'_>, parent: &str) -> Result<Vec<Entry>, String> {
    env.with_local_frame(32, |env| -> jni::errors::Result<Vec<Entry>> {
        let parent = JObject::from(env.new_string(parent)?);
        let children = env
            .call_static_method(
                "android/provider/DocumentsContract",
                "buildChildDocumentsUriUsingTree",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&tree.uri), JValue::Object(&parent)],
            )?
            .l()?;
        let projection = env.new_object_array(3, "java/lang/String", JObject::null())?;
        for (i, column) in ["document_id", "_display_name", "mime_type"].iter().enumerate() {
            let column = env.new_string(column)?;
            env.set_object_array_element(&projection, i as i32, column)?;
        }
        let null = JObject::null();
        let cursor = env
            .call_method(
                &tree.resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&children),
                    JValue::Object(projection.as_ref()),
                    JValue::Object(&null),
                    JValue::Object(&null),
                    JValue::Object(&null),
                ],
            )?
            .l()?;
        let mut entries = Vec::new();
        if cursor.is_null() {
            return Ok(entries);
        }
        let read = (|| -> jni::errors::Result<()> {
            while env.call_method(&cursor, "moveToNext", "()Z", &[])?.z()? {
                let mut column = |i: i32| -> jni::errors::Result<String> {
                    let value = env
                        .call_method(&cursor, "getString", "(I)Ljava/lang/String;", &[JValue::Int(i)])?
                        .l()?;
                    let text = if value.is_null() {
                        String::new()
                    } else {
                        env.get_string((&value).into())?.into()
                    };
                    env.delete_local_ref(value)?;
                    Ok(text)
                };
                let id = column(0)?;
                let name = column(1)?;
                let mime = column(2)?;
                entries.push(Entry {
                    id,
                    name,
                    folder: mime == "vnd.android.document/directory",
                });
            }
            Ok(())
        })();
        env.call_method(&cursor, "close", "()V", &[])?;
        read?;
        Ok(entries)
    })
    .map_err(|e| {
        let _ = env.exception_clear();
        format!("could not list the other app's files ({e})")
    })
}

/// Document `id` of the granted tree, opened for reading.
fn open(env: &mut JNIEnv<'_>, tree: &Tree<'_>, id: &str) -> Result<std::fs::File, String> {
    use std::os::fd::FromRawFd;
    env.with_local_frame(16, |env| -> jni::errors::Result<std::fs::File> {
        let id = JObject::from(env.new_string(id)?);
        let uri = env
            .call_static_method(
                "android/provider/DocumentsContract",
                "buildDocumentUriUsingTree",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&tree.uri), JValue::Object(&id)],
            )?
            .l()?;
        let mode = JObject::from(env.new_string("r")?);
        let descriptor = env
            .call_method(
                &tree.resolver,
                "openFileDescriptor",
                "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
                &[JValue::Object(&uri), JValue::Object(&mode)],
            )?
            .l()?;
        if descriptor.is_null() {
            return Err(jni::errors::Error::NullPtr("openFileDescriptor"));
        }
        let raw = env.call_method(&descriptor, "detachFd", "()I", &[])?.i()?;
        env.call_method(&descriptor, "close", "()V", &[])?;
        if raw < 0 {
            return Err(jni::errors::Error::NullPtr("detachFd"));
        }
        // detachFd handed the descriptor to Rust.
        Ok(unsafe { std::fs::File::from_raw_fd(raw) })
    })
    .map_err(|e| {
        let _ = env.exception_clear();
        format!("could not read a file of the other app ({e})")
    })
}

struct Copier {
    counts: Counts,
    files: usize,
    shown: Instant,
}

impl Copier {
    fn source(
        &mut self,
        env: &mut JNIEnv<'_>,
        tree: &Tree<'_>,
        source: Source,
        id: &str,
        game: &Path,
    ) -> Result<(), String> {
        let dir = game.join(source.folder());
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("refusing a redirected folder".into());
        }
        for entry in list(env, tree, id)? {
            if !source.accepts(&entry.name) {
                continue;
            }
            match (source, entry.folder) {
                (Source::Worlds, true) => {
                    let target = crate::import_plan::world_folder(&dir, &entry.name);
                    self.folder(env, tree, &entry.id, &target, 0)?;
                    self.counts.worlds += 1;
                }
                (Source::Worlds, false) | (_, true) => {}
                (Source::Configuration, false) => {
                    let target = dir.join(&entry.name);
                    if target.is_file() {
                        let _ = std::fs::copy(
                            &target,
                            dir.join(format!("{}.before-import", entry.name)),
                        );
                    }
                    self.file(env, tree, &entry.id, &target, true)?;
                    self.counts.settings += 1;
                }
                (_, false) => {
                    let target = dir.join(&entry.name);
                    if target.exists() {
                        self.counts.skipped += 1;
                    } else {
                        self.file(env, tree, &entry.id, &target, false)?;
                        self.counts.add_ons += 1;
                    }
                }
            }
        }
        Ok(())
    }

    /// A world's folder, with everything in it.
    fn folder(
        &mut self,
        env: &mut JNIEnv<'_>,
        tree: &Tree<'_>,
        id: &str,
        target: &Path,
        depth: usize,
    ) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err("a world folder is nested too deeply".into());
        }
        std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
        for entry in list(env, tree, id)? {
            if !crate::import_plan::safe_name(&entry.name) {
                continue;
            }
            let path = target.join(&entry.name);
            if entry.folder {
                self.folder(env, tree, &entry.id, &path, depth + 1)?;
            } else {
                self.file(env, tree, &entry.id, &path, false)?;
            }
        }
        Ok(())
    }

    /// One file, written beside its place and then moved there.
    fn file(
        &mut self,
        env: &mut JNIEnv<'_>,
        tree: &Tree<'_>,
        id: &str,
        target: &Path,
        replace: bool,
    ) -> Result<(), String> {
        self.files += 1;
        if self.files > MAX_FILES {
            return Err("too many files".into());
        }
        if target.exists() && !replace {
            self.counts.skipped += 1;
            return Ok(());
        }
        let mut input = open(env, tree, id)?;
        let partial = target.with_extension("import-partial");
        let mut output = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut buffer = vec![0u8; 1 << 16];
        loop {
            let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            self.counts.bytes += n as u64;
            if self.shown.elapsed().as_millis() >= 700 {
                self.shown = Instant::now();
                crate::documents::notify(
                    format!(
                        "Importing from the other app: {} files, {} MiB...",
                        self.files,
                        self.counts.bytes >> 20
                    ),
                    false,
                );
            }
        }
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        std::fs::rename(&partial, target).map_err(|e| e.to_string())
    }
}
