// Extras API: the native API `extras` for sandboxed `.hmod` mods (capability `bridge`), in the
// spirit of Fabric API's events and helpers. The game gets this file as
// `crate::game::mod_extras_api`. Version 3: version 2's server API, and the window's video
// options and shader pack for client mods (a `.hmod` asks, this native code does it);
// docs/android-native-mods.md lists it all. Players are named by UUID (as `players::online`
// gives them) or by name. JSON is the form of every argument and answer but `command`'s.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde_json::{Value, json};

/// The mod's id (its settings folder), name and version.
pub const ID: &str = "extras_api";
pub const NAME: &str = "Extras API";
pub const VERSION: &str = "3.2.0";
pub const DESCRIPTION: &str = "Gives sandboxed mods the API extras: events (joins, deaths, kills, drops), cancellable callbacks (block breaks and uses, attacks, item pickups, commands, explosions), player helpers (action bar, sidebar, tab list, nicknames, experience), vanilla commands as the server's console, and the video options and shader pack. Mods using it can do what a server operator can, and change your video settings.";
/// The API's name and version, as mods ask for them.
pub const API: &str = "extras";
pub const API_VERSION: u32 = 3;

/// What a mod hears with the next tick, after it happened.
pub const EVENTS: [&str; 5] = ["player_join", "player_leave", "player_death", "mob_killed", "item_dropped"];
/// What a mod is asked at once and may cancel: it answers `cancel`, or `cancel <text>` to tell
/// the player why.
pub const ASKS: [&str; 6] = ["break_block", "use_block", "attack_entity", "pickup_item", "command", "explosion"];

/// The mods subscribed to each event and ask, by name. The window and the server thread run
/// apart; a mod id only reaches the mods of the game it runs in.
static SUBSCRIBED: Mutex<BTreeMap<String, BTreeSet<String>>> = Mutex::new(BTreeMap::new());

/// A UUID as server mods see players' ids: `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`.
pub fn uuid(u: u128) -> String {
    let h = format!("{u:032x}");
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// The mods subscribed to `name`.
pub fn subscribers(name: &str) -> Vec<String> {
    let subscribed = SUBSCRIBED.lock().unwrap_or_else(|e| e.into_inner());
    subscribed.get(name).map(|s| s.iter().cloned().collect()).unwrap_or_default()
}

/// `subscribe` and `unsubscribe`: a JSON list of event and ask names.
fn subscribe(mod_id: &str, args: &str, on: bool) -> Result<String, String> {
    let names: Vec<String> =
        serde_json::from_str(args).map_err(|_| "expected a JSON list of names".to_owned())?;
    if let Some(bad) = names.iter().find(|n| !EVENTS.contains(&n.as_str()) && !ASKS.contains(&n.as_str())) {
        return Err(format!("{API} has no event {bad}"));
    }
    let mut subscribed = SUBSCRIBED.lock().unwrap_or_else(|e| e.into_inner());
    for name in &names {
        let mods = subscribed.entry(name.clone()).or_default();
        if on {
            mods.insert(mod_id.to_owned());
        } else {
            mods.remove(mod_id);
        }
    }
    Ok(json!(names).to_string())
}

/// `command`'s argument as the command line to run.
pub fn command_line(args: &str) -> Result<&str, String> {
    let line = args.trim();
    let line = line.strip_prefix('/').unwrap_or(line);
    if line.is_empty() {
        return Err("command takes a command line, such as give Alex minecraft:bread 8".into());
    }
    if line.contains(['\n', '\r']) {
        return Err("command runs one line per call".into());
    }
    Ok(line)
}

/// The ticks a second the server keeps up: its rate, or 1000 / milliseconds when that is lower.
pub fn tps(rate: f32, mspt: f64) -> f64 {
    let rate = f64::from(rate);
    if mspt > 0.0 { rate.min(1000.0 / mspt) } else { rate }
}

/// A player as the API shows them.
pub(super) fn player_json(p: &crate::game::native_server::NativeServerPlayer) -> Value {
    json!({
        "player": uuid(p.uuid),
        "name": p.name,
        "permission": p.permission,
        "dimension": p.dimension,
        "x": p.position[0],
        "y": p.position[1],
        "z": p.position[2],
        "game_mode": p.game_mode,
        "health": p.health,
    })
}

/// The online player a call names by `player` (UUID or name).
fn find(game: &crate::game::Game<'_>, args: &Value) -> Result<u128, String> {
    let who = args["player"].as_str().ok_or("expected \"player\": a UUID or a name")?;
    game.native_server_players()
        .into_iter()
        .find(|p| uuid(p.uuid) == who || p.name.eq_ignore_ascii_case(who))
        .map(|p| p.uuid)
        .ok_or_else(|| format!("{who} is not online"))
}

fn text(args: &Value, key: &str) -> Result<horizon_client::text::Text, String> {
    let line = args[key].as_str().ok_or_else(|| format!("expected \"{key}\": a text"))?;
    Ok(horizon_client::text::Text::literal(line))
}

fn done(ok: bool) -> Result<String, String> {
    if ok { Ok(String::new()) } else { Err("the player is not here".into()) }
}

/// The video options client mods read (`options`) and set (`set_option`), as JSON.
fn options(s: &horizon_client::settings::Settings) -> Value {
    json!({
        "render_distance": s.render_distance,
        "simulation_distance": s.simulation_distance,
        "max_fps": s.max_fps,
        "graphics": s.graphics_preset,
        "clouds": s.clouds,
        "particles": s.particles,
        "smooth_lighting": s.smooth_lighting,
        "entity_shadows": s.entity_shadows,
        "biome_blend": s.biome_blend,
        "mipmap_levels": s.mipmap_levels,
        "entity_distance": s.entity_distance,
        "gamma": s.gamma,
        "fov": s.fov,
        "view_bobbing": s.view_bobbing,
        "shader_pack": s.horizon.shader_pack,
        "cave_culling": s.horizon.cave_culling,
        "entity_culling": s.horizon.entity_culling,
        "fog_culling": s.horizon.fog_culling,
    })
}

/// A number in `min..=max` from `value`.
fn number(value: &Value, min: f64, max: f64) -> Result<f64, String> {
    let n = value.as_f64().ok_or("expected a number")?;
    if (min..=max).contains(&n) { Ok(n) } else { Err(format!("expected {min} to {max}")) }
}

fn flag(value: &Value) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| "expected true or false".to_owned())
}

/// `set_option`: `{"name", "value"}`, applied to the window as its Video Settings would, and saved.
fn set_option(game: &mut crate::game::Game<'_>, args: &Value) -> Result<String, String> {
    let name = args["name"].as_str().ok_or("expected \"name\": an option")?;
    let value = &args["value"];
    let s = &mut game.settings;
    let mut custom = true;
    match name {
        "render_distance" => s.render_distance = number(value, 2.0, 32.0)? as i32,
        "simulation_distance" => s.simulation_distance = number(value, 5.0, 32.0)? as i32,
        "max_fps" => s.max_fps = number(value, 10.0, 260.0)? as u32,
        "graphics" => {
            let preset = value.as_str().unwrap_or_default();
            if !["fast", "fancy", "fabulous"].contains(&preset) {
                return Err("expected fast, fancy or fabulous".into());
            }
            s.graphics_preset = preset.into();
            horizon_client::options_ui::apply_preset(s);
            custom = false;
        }
        "clouds" => {
            let clouds = value.as_str().unwrap_or_default();
            if !["false", "fast", "true"].contains(&clouds) {
                return Err("expected \"false\", \"fast\" or \"true\"".into());
            }
            s.clouds = clouds.into();
        }
        "particles" => s.particles = number(value, 0.0, 2.0)? as u8,
        "smooth_lighting" => s.smooth_lighting = flag(value)?,
        "entity_shadows" => s.entity_shadows = flag(value)?,
        "biome_blend" => s.biome_blend = number(value, 0.0, 7.0)? as i32,
        "mipmap_levels" => s.mipmap_levels = number(value, 0.0, 4.0)? as i32,
        "entity_distance" => s.entity_distance = number(value, 0.5, 5.0)? as f32,
        // Past 1 it brightens further, as fullbright mods do.
        "gamma" => {
            s.gamma = number(value, 0.0, 10.0)? as f32;
            custom = false;
        }
        "fov" => {
            s.fov = number(value, 30.0, 110.0)? as f32;
            custom = false;
        }
        "view_bobbing" => {
            s.view_bobbing = flag(value)?;
            custom = false;
        }
        // Horizon's culling (Video Settings > Horizon's, not vanilla's): the preset stays.
        "cave_culling" => {
            s.horizon.cave_culling = flag(value)?;
            custom = false;
        }
        "entity_culling" => {
            s.horizon.entity_culling = flag(value)?;
            custom = false;
        }
        "fog_culling" => {
            s.horizon.fog_culling = flag(value)?;
            custom = false;
        }
        "shader_pack" => {
            let pick = value.as_str().unwrap_or_default();
            let packs = horizon_client::shader_pack_list::scan(&crate::game::Game::shader_packs_dir());
            if !pick.is_empty() && !packs.iter().any(|p| p.file == pick) {
                return Err(format!("no shader pack {pick} in shaderpacks"));
            }
            s.horizon.shader_pack = pick.into();
            custom = false;
        }
        other => return Err(format!("no option {other}")),
    }
    if custom && name != "graphics" {
        s.graphics_preset = "custom".into();
    }
    game.apply_settings();
    game.settings.save(&crate::game::Game::options_path());
    Ok(options(&game.settings).to_string())
}

/// A client mod's call: the window's options.
fn serve_client(game: &mut crate::game::Game<'_>, function: &str, args: &str) -> Result<String, String> {
    match function {
        "options" => Ok(options(&game.settings).to_string()),
        "set_option" => {
            let args: Value = serde_json::from_str(args).map_err(|e| format!("arguments: {e}"))?;
            set_option(game, &args)
        }
        "shader_packs" => {
            let packs = horizon_client::shader_pack_list::scan(&crate::game::Game::shader_packs_dir());
            let packs: Vec<Value> =
                packs.iter().map(|p| json!({"file": p.file, "title": p.title})).collect();
            Ok(json!(packs).to_string())
        }
        other => Err(format!("{other} is for server mods, or {API} has no such function")),
    }
}

/// What a mod needs the player's permission for, by group: its name and what the player reads.
/// Reading (players, tps, options, events) needs none.
pub const GROUPS: [(&str, &str); 4] = [
    ("commands", "run commands as the server"),
    ("players", "message, rename and give experience to players"),
    ("control", "cancel what players do"),
    ("settings", "change your video settings"),
];

/// The group a call needs, if any.
pub fn group_of(function: &str, args: &str) -> Option<&'static str> {
    match function {
        "command" => Some("commands"),
        "tell" | "actionbar" | "sidebar" | "tablist" | "nickname" | "xp" | "levels"
        | "sleep_ignored" => Some("players"),
        "set_option" => Some("settings"),
        // Subscribing to an ask lets the mod cancel; events only hear.
        "subscribe" => {
            let names: Vec<String> = serde_json::from_str(args).unwrap_or_default();
            names.iter().any(|n| ASKS.contains(&n.as_str())).then_some("control")
        }
        _ => None,
    }
}

/// The player's decisions, kept in the game folder (`.horizon-mod-data/extras_api/permissions`),
/// one `mod_id group allow|deny|ask` a line; a server's admin can edit it.
pub fn read_permissions(text: &str) -> BTreeMap<(String, String), String> {
    text.lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let (m, g, s) = (parts.next()?, parts.next()?, parts.next()?);
            GROUPS.iter().any(|(name, _)| *name == g).then(|| ((m.to_owned(), g.to_owned()), s.to_owned()))
        })
        .collect()
}

pub fn write_permissions(all: &BTreeMap<(String, String), String>) -> String {
    all.iter().map(|((m, g), s)| format!("{m} {g} {s}\n")).collect()
}

fn permissions(game: &crate::game::Game<'_>) -> BTreeMap<(String, String), String> {
    read_permissions(&game.native_setting(ID, "permissions").unwrap_or_default())
}

pub(super) fn save_permissions(game: &crate::game::Game<'_>, all: &BTreeMap<(String, String), String>) {
    game.native_set_setting(ID, "permissions", &write_permissions(all));
}

/// What the player reads for a group.
pub fn describe(group: &str) -> &'static str {
    GROUPS.iter().find(|(g, _)| *g == group).map_or("do this", |(_, d)| d)
}

/// `Ok` when the player allowed `mod_id` the group; else the request is recorded (and the
/// operators told, once) and the call refused with where to allow it.
fn check(game: &mut crate::game::Game<'_>, server: bool, mod_id: &str, group: &str) -> Result<(), String> {
    let mut all = permissions(game);
    let key = (mod_id.to_owned(), group.to_owned());
    match all.get(&key).map(String::as_str) {
        Some("allow") => return Ok(()),
        Some(_) => {}
        None => {
            all.insert(key, "ask".into());
            save_permissions(game, &all);
            let line = horizon_client::text::Text::literal(format!(
                "[Extras API] {mod_id} asks to {}: Options > Mods > Extras API > Settings",
                describe(group)
            ));
            if server {
                let players = game.native_server_players();
                let ops: Vec<u128> = players.iter().filter(|p| p.permission >= 2).map(|p| p.uuid).collect();
                // Single player without cheats: its one player decides.
                let tell = if ops.is_empty() && players.len() == 1 { vec![players[0].uuid] } else { ops };
                for uuid in tell {
                    game.native_tell(uuid, line.clone());
                }
            } else {
                game.system_message(line);
            }
        }
    }
    Err(format!(
        "{mod_id} needs your permission to {}: allow it in Options > Mods > Extras API > Settings",
        describe(group)
    ))
}

/// The Extras API's settings page: one switch a mod and group it asked for.
pub(super) fn settings_buttons(game: &crate::game::Game<'_>) -> Vec<String> {
    let all = permissions(game);
    if all.is_empty() {
        return vec!["No mod has asked for a permission yet".into()];
    }
    all.iter()
        .map(|((m, g), s)| {
            let state = if s == "allow" { "Allowed" } else { "Not allowed" };
            format!("{m}: {}: {state}", describe(g))
        })
        .collect()
}

/// Button `button` of that page: allows or takes back that permission.
pub(super) fn settings_click(game: &crate::game::Game<'_>, button: usize) {
    let mut all = permissions(game);
    if let Some((_, state)) = all.iter_mut().nth(button) {
        *state = if state == "allow" { "deny".into() } else { "allow".into() };
        save_permissions(game, &all);
    }
}

/// A mod's call to `extras`: its answer.
pub(super) fn serve(
    game: &mut crate::game::Game<'_>,
    call: &crate::game::native_hooks::NativeBridgeCall<'_>,
) -> Result<String, String> {
    let function = call.function;
    if let Some(group) = group_of(function, call.args) {
        check(game, call.server, call.mod_id, group)?;
    }
    if !call.server {
        return serve_client(game, function, call.args);
    }
    if matches!(function, "options" | "set_option" | "shader_packs") {
        return Err(format!("{function} is for client mods: the window's options"));
    }
    if function == "command" {
        let line = command_line(call.args)?;
        return if game.native_run_command(line) { Ok(String::new()) } else { Err(format!("/{line} failed")) };
    }
    if function == "subscribe" || function == "unsubscribe" {
        return subscribe(call.mod_id, call.args, function == "subscribe");
    }
    if function == "players" {
        let players: Vec<Value> = game.native_server_players().iter().map(player_json).collect();
        return Ok(json!(players).to_string());
    }
    let args: Value = serde_json::from_str(call.args).map_err(|e| format!("arguments: {e}"))?;
    let who = find(game, &args)?;
    match function {
        "tell" => done(game.native_tell(who, text(&args, "text")?)),
        "actionbar" => done(game.native_action_bar(who, text(&args, "text")?)),
        "sidebar" => {
            if args["title"].is_null() {
                return done(game.native_sidebar(who, None));
            }
            let lines = args["lines"].as_array().map(Vec::as_slice).unwrap_or_default();
            let sidebar = crate::game::native_server::NativeSidebar {
                title: text(&args, "title")?,
                lines: lines
                    .iter()
                    .take(15)
                    .map(|l| {
                        let left = l.as_str().or(l["left"].as_str()).unwrap_or_default();
                        let right = l["right"].as_str().map(horizon_client::text::Text::literal);
                        (horizon_client::text::Text::literal(left), right)
                    })
                    .collect(),
            };
            done(game.native_sidebar(who, Some(&sidebar)))
        }
        "tablist" => done(game.native_tab_list(who, &text(&args, "header")?, &text(&args, "footer")?)),
        "nickname" => done(game.native_set_nickname(who, args["name"].as_str())),
        "xp" => {
            let points = args["points"].as_u64().ok_or("expected \"points\": a count")?;
            done(game.native_give_experience(who, points.min(u64::from(u32::MAX)) as u32))
        }
        "levels" => {
            let levels = args["levels"].as_i64().ok_or("expected \"levels\": a number")?;
            done(game.native_give_levels(who, levels.clamp(-100_000, 100_000) as i32))
        }
        "sleep_ignored" => {
            let ignored = args["ignored"].as_bool().ok_or("expected \"ignored\": true or false")?;
            done(game.native_sleep_ignored(who, ignored))
        }
        other => Err(format!("{API} has no function {other}")),
    }
}

/// Tells the mods subscribed to the event `name`, with the next tick.
pub(super) fn notify(game: &mut crate::game::Game<'_>, name: &str, data: impl FnOnce() -> Value) {
    let mods = subscribers(name);
    if mods.is_empty() || !game.native_on(ID) {
        return;
    }
    let data = data().to_string();
    for mod_id in mods {
        game.native_bridge_event_to(&mod_id, API, name, &data);
    }
}

/// Asks the mods subscribed to `name`, in turn: `Some(why)` when one cancels (`why` may be
/// empty), and then the later ones aren't asked.
pub(super) fn ask(game: &mut crate::game::Game<'_>, name: &str, data: impl FnOnce() -> Value) -> Option<String> {
    let mods = subscribers(name);
    if mods.is_empty() || !game.native_on(ID) {
        return None;
    }
    let data = data().to_string();
    mods.iter().find_map(|mod_id| {
        let answer = game.native_bridge_ask(mod_id, API, name, &data)?;
        let why = answer.strip_prefix("cancel")?;
        (why.is_empty() || why.starts_with(' ')).then(|| why.trim().to_owned())
    })
}

/// [`ask`] for what the acting player does: their data merged in, and the reason told to them.
pub(super) fn ask_acting(game: &mut crate::game::Game<'_>, name: &str, data: Value) -> bool {
    let player = game.native_acting_player();
    let why = ask(game, name, || {
        let mut all = player.as_ref().map(player_json).unwrap_or_else(|| json!({}));
        if let (Some(all), Some(more)) = (all.as_object_mut(), data.as_object()) {
            all.extend(more.clone());
        }
        all
    });
    if let (Some(why), Some(p)) = (&why, &player)
        && !why.is_empty()
    {
        game.native_tell(p.uuid, horizon_client::text::Text::literal(why));
    }
    why.is_some()
}
