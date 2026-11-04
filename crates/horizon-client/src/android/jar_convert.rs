// A Java mod (.jar) converted on the device into a content .hmod: its blocks, items, recipes,
// loot tables, tags, worldgen, textures, models, sounds and languages, carried over by
// horizon-port-pack's carry step (docs/modpacks.md), with recipes upgraded to 26.3 and each
// block given the vanilla behaviour its properties suggest. Its Java code is not converted:
// Horizon cannot run it, so what needs code is reported for a port.
use horizon_port_pack::{carry, jar};
use std::path::{Path, PathBuf};

/// What a conversion made: the mod id, and a summary for the player.
pub struct Converted {
    pub id: String,
    pub summary: String,
}

/// Replaces the jar at `staged` with the content .hmod carried over from it; `cache` holds the
/// work folder while it runs.
pub fn convert(staged: &Path, cache: &Path) -> Result<Converted, String> {
    let info = jar::read(staged)?;
    let work = cache.join(format!(".convert-{}", info.mod_id));
    let _ = std::fs::remove_dir_all(&work);
    let result = (|| {
        // One jar: only its own namespaces are its content; every vanilla block may serve as
        // a template.
        let own = |id: &str| id == info.mod_id;
        let pack = carry::Pack {
            loaded: &own,
            mods: &own,
        };
        let report = carry::carry(
            staged,
            &info,
            &work,
            None,
            &|_| true,
            &pack,
            &Default::default(),
        )?;
        let folder = work.join("carry");
        let manifest = std::fs::read_to_string(folder.join(horizon_mods::manifest::MANIFEST))
            .map_err(|e| format!("the converted mod has no manifest: {e}"))?;
        let mut files = Vec::new();
        collect(&folder, &folder, &mut files)?;
        if files.is_empty() {
            return Err(format!(
                "{} has no blocks, items, recipes or textures to convert: it is all Java code{}",
                display_name(&info),
                code_note(&info)
            ));
        }
        let refs: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        let bytes = horizon_mods::Package::write_files(&manifest, None, &refs)?;
        std::fs::write(staged, bytes).map_err(|e| e.to_string())?;
        Ok(Converted {
            id: info.mod_id.clone(),
            summary: summary(&info, &report),
        })
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Every file under `dir` but the manifest, by its path from `root` with `/` separators.
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(root, &path, out)?;
            continue;
        }
        let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
        let name = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if name == horizon_mods::manifest::MANIFEST {
            continue;
        }
        out.push((name, std::fs::read(&path).map_err(|e| e.to_string())?));
    }
    Ok(())
}

fn display_name(info: &jar::JarInfo) -> String {
    let name = if info.name.is_empty() {
        &info.mod_id
    } else {
        &info.name
    };
    match info.version.as_str() {
        "" => name.clone(),
        v => format!("{name} {v}"),
    }
}

/// What the jar's code means for the player, from its class count.
fn code_note(info: &jar::JarInfo) -> String {
    match info.census.classes {
        0 => String::new(),
        n => format!(
            ". Its Java code ({n} classes) can't run in Horizon: anything that needs code \
             (machines, special tools, menus, new mechanics) needs a port"
        ),
    }
}

/// One paragraph: what came over, what was left out, and what needs a port.
pub fn summary(info: &jar::JarInfo, report: &carry::Report) -> String {
    let c = &info.census;
    // Items are counted only when carry-over declared them (with the oracle's facts): the jar's
    // item models include those of blocks left out.
    let items = match report.items {
        0 => String::new(),
        1 => "1 item, ".to_owned(),
        n => format!("{n} items, "),
    };
    let mut parts = vec![format!(
        "Converted {}: {} blocks, {items}{} recipes, {} textures",
        display_name(info),
        report.blocks.len(),
        c.recipes.saturating_sub(report.recipes_kept.len()),
        c.textures,
    )];
    if !report.blocks_unknown.is_empty() {
        parts.push(format!("{} blocks left out", report.blocks_unknown.len()));
    }
    if !report.recipes_kept.is_empty() {
        parts.push(format!(
            "{} recipes of the mod's own kinds won't craft",
            report.recipes_kept.len()
        ));
    }
    if !info.depends.is_empty() {
        parts.push(format!("it needs {}", info.depends.join(", ")));
    }
    format!("{}{}.", parts.join("; "), code_note(info))
}
