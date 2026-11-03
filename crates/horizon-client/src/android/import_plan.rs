//! Where data copied out of another Horizon Android app goes: no JNI, so the host content
//! tests cover it (`tools/test_android_content.py`).
use std::path::{Path, PathBuf};

/// What a folder of the other app's Files provider holds, by the last part of its document id
/// (`p0/worlds`, `p0/mods`...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Worlds,
    Mods,
    NativeMods,
    Packs,
    Shaders,
    Configuration,
}

impl Source {
    pub fn of(document_id: &str) -> Option<Self> {
        match document_id.rsplit('/').next()? {
            "worlds" => Some(Self::Worlds),
            "mods" => Some(Self::Mods),
            "native-mods" => Some(Self::NativeMods),
            "packs" => Some(Self::Packs),
            "shaders" => Some(Self::Shaders),
            "configuration" => Some(Self::Configuration),
            _ => None,
        }
    }
    /// The folder of this app's profile it goes to.
    pub fn folder(self) -> &'static str {
        match self {
            Self::Worlds => "saves",
            Self::Mods => "horizon-mods",
            Self::NativeMods => "horizon-native-mods",
            Self::Packs => "resourcepacks",
            Self::Shaders => "shaderpacks",
            Self::Configuration => "",
        }
    }
    /// Whether a file of this folder is taken: add-ons by their extension, settings by name.
    pub fn accepts(self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        safe_name(name)
            && match self {
                Self::Worlds => true,
                Self::Mods => lower.ends_with(".hmod"),
                Self::NativeMods => lower.ends_with(".hnmod"),
                Self::Packs => lower.ends_with(".zip"),
                Self::Shaders => lower.ends_with(".zip") || lower.ends_with(".hshader"),
                Self::Configuration => CONFIG_FILES.contains(&name),
            }
    }
}

/// A file or folder name that stays where it is put: no path, no hidden or control characters.
pub fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 180
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.chars().any(char::is_control)
}

/// The settings files the other app's provider shows (its `configuration` folder).
pub const CONFIG_FILES: &[&str] = &[
    "options.txt",
    "horizon-graphics.txt",
    "horizon-touch.json",
    "horizon-keypad.json",
];

/// A world's folder in `saves` that keeps an existing world of the same name:
/// `Name`, else `Name (imported)`, `Name (imported 2)`...
pub fn world_folder(saves: &Path, name: &str) -> PathBuf {
    let first = saves.join(name);
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| {
            saves.join(if n == 1 {
                format!("{name} (imported)")
            } else {
                format!("{name} (imported {n})")
            })
        })
        .find(|p| !p.exists())
        .expect("an unused name")
}

/// What an import copied, for the message after it.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Counts {
    pub worlds: usize,
    pub add_ons: usize,
    pub settings: usize,
    pub skipped: usize,
    pub bytes: u64,
}

impl Counts {
    pub fn summary(&self) -> String {
        if self.worlds + self.add_ons + self.settings == 0 {
            return "Nothing to import there: choose the other app's Horizon Android folder, or its Worlds".into();
        }
        let mut parts = Vec::new();
        if self.worlds > 0 {
            parts.push(format!("{} world(s)", self.worlds));
        }
        if self.add_ons > 0 {
            parts.push(format!("{} mod/pack file(s)", self.add_ons));
        }
        if self.settings > 0 {
            parts.push(format!("{} settings file(s)", self.settings));
        }
        let mut text = format!(
            "Imported {} ({} MiB)",
            parts.join(", "),
            self.bytes / (1 << 20)
        );
        if self.skipped > 0 {
            text.push_str(&format!(
                "; kept {} file(s) this app already had",
                self.skipped
            ));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_folders_map_to_this_profile() {
        assert_eq!(Source::of("p0/worlds"), Some(Source::Worlds));
        assert_eq!(Source::of("p3/native-mods"), Some(Source::NativeMods));
        assert_eq!(Source::of("reports"), None);
        assert_eq!(Source::of("horizon"), None);
        assert!(Source::Mods.accepts("veinminer.hmod") && !Source::Mods.accepts("x.zip"));
        assert!(Source::Configuration.accepts("options.txt"));
        assert!(!Source::Configuration.accepts("accounts.json"));
        assert!(!Source::Worlds.accepts("../escape") && !Source::Worlds.accepts(".hidden"));
    }
    #[test]
    fn an_imported_world_never_replaces_one() {
        let saves = std::env::temp_dir().join(format!("horizon-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&saves);
        std::fs::create_dir_all(saves.join("New World")).unwrap();
        assert_eq!(world_folder(&saves, "Fresh"), saves.join("Fresh"));
        assert_eq!(
            world_folder(&saves, "New World"),
            saves.join("New World (imported)")
        );
        std::fs::create_dir_all(saves.join("New World (imported)")).unwrap();
        assert_eq!(
            world_folder(&saves, "New World"),
            saves.join("New World (imported 2)")
        );
        std::fs::remove_dir_all(&saves).unwrap();
    }
    #[test]
    fn the_summary_names_what_came_over() {
        assert!(Counts::default().summary().starts_with("Nothing to import"));
        let counts = Counts {
            worlds: 2,
            add_ons: 1,
            settings: 3,
            skipped: 1,
            bytes: 5 << 20,
        };
        assert_eq!(
            counts.summary(),
            "Imported 2 world(s), 1 mod/pack file(s), 3 settings file(s) (5 MiB); kept 1 file(s) this app already had"
        );
    }
}
