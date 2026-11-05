// The mod check: what a Java mod (.jar) or a native mod (.hnmod) uses, read without running or
// converting it, and which of those parts Horizon's .hmod API (or Hapi through the mod bridge)
// can do. Free and instant, on the phone: it tells the player whether a conversion is worth it
// before an AI spends their key on one.
use std::{io::Read, path::Path};

/// One part of a mod, and what Horizon can do of it.
pub struct Part {
    pub what: &'static str,
    pub horizon: &'static str,
    /// Yes, partly, or no.
    pub fit: Fit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    Yes,
    Partly,
    No,
}

pub struct Report {
    pub parts: Vec<Part>,
    pub verdict: String,
}

/// What Java mods use, by the class names their bytecode references, and Horizon's answer.
const JAVA: &[(&[&str], &str, &str, Fit)] = &[
    (
        &[
            "net/minecraft/client/gui/Gui",
            "net/minecraft/client/gui/Hud",
            "HudRenderCallback",
            "HudElementRegistry",
            "RenderGuiEvent",
            "RenderGuiLayerEvent",
            "RegisterGuiLayersEvent",
        ],
        "Draws on the screen (HUD)",
        "Yes: the hud capability draws text, sprites and boxes",
        Fit::Yes,
    ),
    (
        &[
            "net/minecraft/world/food/FoodData",
            "net/minecraft/world/food/FoodProperties",
        ],
        "Reads hunger, food values and saturation",
        "Yes: food, saturation, exhaustion (single player) and the held food's values",
        Fit::Yes,
    ),
    (
        &[
            "TooltipComponent",
            "ItemTooltipCallback",
            "ItemTooltipEvent",
            "RenderTooltipEvent",
            "Item$TooltipContext",
        ],
        "Adds lines or pictures to item tooltips",
        "Partly: no tooltips, but a mod can draw beside the held item's name",
        Fit::Partly,
    ),
    (
        &[
            "ClientTickEvents",
            "ServerTickEvents",
            "PlayerTickEvent",
            "ServerTickEvent",
            "ClientTickEvent",
        ],
        "Runs something every tick",
        "Yes: the tick event",
        Fit::Yes,
    ),
    (
        &[
            "CommandRegistrationCallback",
            "ClientCommandRegistrationCallback",
            "RegisterCommandsEvent",
            "com/mojang/brigadier/",
        ],
        "Adds commands",
        "Yes: the command capability",
        Fit::Yes,
    ),
    (
        &["PlayerBlockBreakEvents", "BlockEvent$BreakEvent"],
        "Reacts to blocks being broken",
        "Yes: server mods' block events, or Hapi's break_block",
        Fit::Yes,
    ),
    (
        &[
            "UseBlockCallback",
            "UseItemCallback",
            "AttackEntityCallback",
            "AttackBlockCallback",
            "PlayerInteractEvent",
        ],
        "Reacts to using or attacking",
        "Yes: Hapi's use_block and attack events",
        Fit::Yes,
    ),
    (
        &[
            "KeyBindingHelper",
            "net/minecraft/client/KeyMapping",
            "RegisterKeyMappingsEvent",
        ],
        "Adds key bindings",
        "Yes: mod keys (the input capability), touch buttons included",
        Fit::Yes,
    ),
    (
        &[
            "ServerPlayConnectionEvents",
            "PlayerLoggedInEvent",
            "PlayerLoggedOutEvent",
        ],
        "Reacts to players joining or leaving",
        "Yes: server mods' join and leave events",
        Fit::Yes,
    ),
    (
        &["ServerLivingEntityEvents", "LivingDeathEvent"],
        "Reacts to deaths",
        "Yes: Hapi's death event",
        Fit::Yes,
    ),
    (
        &["WorldRenderEvents", "RenderLevelStageEvent"],
        "Draws in the world",
        "Partly: the render capability draws block outlines",
        Fit::Partly,
    ),
    (
        &["org/spongepowered/asm/mixin/Mixin"],
        "Changes Minecraft's own code (mixins)",
        "Partly: rewritten with an API event where one exists; the rest needs a native .hnmod",
        Fit::Partly,
    ),
    (
        &[
            "CustomPacketPayload",
            "PayloadTypeRegistry",
            "ServerPlayNetworking",
            "ClientPlayNetworking",
        ],
        "Sends its own network packets",
        "Partly: a two-sided mod's halves talk through quiet commands",
        Fit::Partly,
    ),
    (
        &[
            "net/minecraft/client/gui/screens/Screen",
            "ScreenEvents",
            "ScreenEvent",
        ],
        "Adds or changes screens",
        "Partly: a mod gets a settings page, not screens of its own",
        Fit::Partly,
    ),
    (
        &["AbstractContainerMenu", "MenuType", "ScreenHandler"],
        "Adds inventories or menus",
        "No: no custom container menus",
        Fit::No,
    ),
    (
        &[
            "net/minecraft/world/level/block/entity/BlockEntity",
            "BlockEntityType",
        ],
        "Adds machines or block entities",
        "No: no block entities in mods yet",
        Fit::No,
    ),
    (
        &[
            "FabricEntityTypeBuilder",
            "EntityType$Builder",
            "EntityRenderers",
        ],
        "Adds new mobs or entities",
        "No: no new entity types in mods yet",
        Fit::No,
    ),
    (
        &["DeferredRegister", "net/minecraft/core/Registry"],
        "Registers blocks, items or other content",
        "Yes: content (blocks, items, recipes) comes over even on the phone",
        Fit::Yes,
    ),
    (
        &["DebugScreenEntry"],
        "Adds lines to the F3 debug screen",
        "No: the debug screen takes no mod lines",
        Fit::No,
    ),
];

/// Hapi's events by the hook names a native mod's injections target.
const HAPI: &[(&str, &str)] = &[
    ("destroy_block", "Hapi's break_block event"),
    ("break_block", "Hapi's break_block event"),
    ("right_click_block", "Hapi's use_block event"),
    ("use_block", "Hapi's use_block event"),
    ("attack", "Hapi's attack event"),
    ("died", "Hapi's death event"),
    ("death", "Hapi's death event"),
    ("killed", "Hapi's killed event"),
    ("dropped", "Hapi's dropped event"),
    ("picked_up", "Hapi's pickup event"),
    ("pickup", "Hapi's pickup event"),
    ("explosion", "Hapi's explosion event"),
    (
        "logged_in",
        "Hapi's join event, or server mods' player_joined",
    ),
    ("join", "Hapi's join event, or server mods' player_joined"),
    (
        "logged_out",
        "Hapi's leave event, or server mods' player_left",
    ),
    ("leave", "Hapi's leave event, or server mods' player_left"),
    ("command", "the command capability, or Hapi's command event"),
    ("tick", "the tick event"),
    ("settings", "a mod settings page (the ui capability)"),
];

/// Checks the mod at `path` (a .jar or a .hnmod).
pub fn check(path: &Path, name: &str) -> Result<Report, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| format!("{name}: not a mod archive ({e})"))?;
    if name.to_ascii_lowercase().ends_with(".hnmod") {
        native(&mut zip, name)
    } else {
        java(&mut zip, name)
    }
}

fn java(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Report, String> {
    let mut found = vec![false; JAVA.len()];
    let (mut classes, mut content) = (0, 0);
    let mut bytes = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let path = entry.name().to_owned();
        if path.ends_with(".class") {
            classes += 1;
            bytes.clear();
            (&mut entry)
                .take(4 << 20)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            for (k, (patterns, ..)) in JAVA.iter().enumerate() {
                if !found[k] && patterns.iter().any(|p| references(&bytes, p)) {
                    found[k] = true;
                }
            }
        } else if (path.starts_with("assets/") || path.starts_with("data/")) && !path.ends_with('/')
        {
            content += 1;
        }
    }
    let mut parts: Vec<Part> = JAVA
        .iter()
        .zip(&found)
        .filter(|(_, hit)| **hit)
        .map(|((_, what, horizon, fit), _)| Part {
            what,
            horizon,
            fit: *fit,
        })
        .collect();
    if content > 0 && !parts.iter().any(|p| p.what.starts_with("Registers")) {
        parts.push(Part {
            what: "Has textures, models or data files",
            horizon: "Yes: they come over with the phone converter",
            fit: Fit::Yes,
        });
    }
    let verdict = verdict(name, &parts, classes);
    Ok(Report { parts, verdict })
}

fn native(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Report, String> {
    let mut parts = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if !entry.name().ends_with("horizon-mixin.toml") {
            continue;
        }
        let mut text = String::new();
        (&mut entry)
            .take(1 << 20)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        for line in text.lines() {
            let Some(target) = line
                .trim()
                .strip_prefix("target")
                .and_then(|t| t.split('"').nth(1))
            else {
                continue;
            };
            let hook = target.rsplit("::").next().unwrap_or(target);
            if !seen.insert(hook.to_owned()) {
                continue;
            }
            // Hapi's own plumbing (its row in the mods list, the bridge): nothing to port.
            if ["native_mods", "native_bridge_apis", "native_bridge_call"].contains(&hook) {
                continue;
            }
            let event = hook
                .strip_prefix("native_")
                .and_then(|h| HAPI.iter().find(|(key, _)| h.contains(key)));
            parts.push(match event {
                Some((_, answer)) => Part {
                    what: leak(format!("Hooks {hook}")),
                    horizon: leak(format!("Yes: {answer}")),
                    fit: Fit::Yes,
                },
                None if hook.starts_with("native_") => Part {
                    what: leak(format!("Hooks {hook}")),
                    horizon: "No: Hapi doesn't pass this hook to .hmod mods yet",
                    fit: Fit::No,
                },
                None => Part {
                    what: leak(format!("Patches the game's code at {hook}")),
                    horizon: "No: only a native mod can change the game's own code",
                    fit: Fit::No,
                },
            });
        }
    }
    let verdict = verdict(name, &parts, 0);
    Ok(Report { parts, verdict })
}

/// A report's text lives as long as the dialog showing it; checks are rare, so leaking the few
/// strings a native mod's hooks make is simpler than threading lifetimes through the chooser.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// Whether a class file's bytes reference `pattern`: a class or event name alone (no `/`) or a
/// package (`com/mojang/brigadier/`) anywhere; a full class name only as itself (a constant pool
/// entry of exactly its length, or in a descriptor ending `;` or an inner class `$`), so
/// `net/minecraft/core/Registry` is not found in `RegistryAccess`.
fn references(bytes: &[u8], pattern: &str) -> bool {
    let needle = pattern.as_bytes();
    if !pattern.contains('/') || pattern.ends_with('/') {
        return bytes.windows(needle.len()).any(|w| w == needle);
    }
    let mut from = 0;
    while let Some(at) = bytes[from..]
        .windows(needle.len())
        .position(|w| w == needle)
    {
        let i = from + at;
        let entry =
            i >= 2 && usize::from(u16::from_be_bytes([bytes[i - 2], bytes[i - 1]])) == needle.len();
        if entry || matches!(bytes.get(i + needle.len()), Some(b';' | b'$')) {
            return true;
        }
        from = i + 1;
    }
    false
}

/// One line: how much of the mod Horizon can do, and what to do next.
fn verdict(name: &str, parts: &[Part], classes: usize) -> String {
    let count = |fit: Fit| parts.iter().filter(|p| p.fit == fit).count();
    let (yes, partly, no) = (count(Fit::Yes), count(Fit::Partly), count(Fit::No));
    let code = match classes {
        0 => String::new(),
        n => format!(" ({n} Java classes)"),
    };
    let advice = if parts.is_empty() {
        "Nothing recognised: an AI conversion may still find something, or not."
    } else if no == 0 && partly == 0 {
        "Good fit: Convert with AI should manage all of it."
    } else if yes + partly >= no {
        "Partly fits: an AI conversion can bring the parts marked Yes or Partly, not the rest."
    } else {
        "Poor fit: most of it needs what Horizon's mods can't do yet."
    };
    format!("{name}{code}: {yes} yes, {partly} partly, {no} no. {advice}")
}
