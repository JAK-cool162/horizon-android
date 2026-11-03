// Extras API: the native API `extras` for sandboxed `.hmod` mods (capability `bridge`), in the
// spirit of Fabric API's events and helpers. The game gets this file as
// `crate::game::mod_extras_api`. Version 2 (version 1's `command`, `tps` and `mspt` kept);
// docs/android-native-mods.md lists it all. Players are named by UUID (as `players::online`
// gives them) or by name. JSON is the form of every argument and answer but `command`'s.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde_json::{Value, json};

/// The mod's id (its settings folder), name and version.
pub const ID: &str = "extras_api";
pub const NAME: &str = "Extras API";
pub const VERSION: &str = "2.0.0";
pub const DESCRIPTION: &str = "Gives sandboxed mods the API extras: events (joins, deaths, kills, drops), cancellable callbacks (block breaks and uses, attacks, item pickups, commands, explosions), player helpers (action bar, sidebar, tab list, nicknames, experience) and vanilla commands as the server's console. Mods using it can do what a server operator can.";
/// The API's name and version, as mods ask for them.
pub const API: &str = "extras";
pub const API_VERSION: u32 = 2;

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

/// A mod's call to `extras`: its answer.
pub(super) fn serve(
    game: &mut crate::game::Game<'_>,
    call: &crate::game::native_hooks::NativeBridgeCall<'_>,
) -> Result<String, String> {
    let function = call.function;
    if !call.server {
        return Err(format!("{function} is for server mods"));
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
