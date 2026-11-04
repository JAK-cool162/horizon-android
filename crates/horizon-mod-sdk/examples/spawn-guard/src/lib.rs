//! Example Horizon server mod on the mod bridge's asks: the Extras API native mod
//! (`extras_api.hnmod`, version 2) asks it before a block is broken or used and before an
//! explosion, and it answers `cancel` near the Overworld's 0, 0. Operators (level 2 and up)
//! build there as usual.
use horizon_mod_sdk::bridge;
use horizon_mod_sdk::server::{Registration, ServerMod};
use serde_json::Value;

const API: &str = "hapi";
/// The asks it subscribes to.
const ASKS: &str = r#"["break_block","use_block","explosion"]"#;
/// How far from 0, 0 the area reaches, in blocks.
const RADIUS: f64 = 16.0;

/// Whether the Extras API took the subscription; until then (the player hasn't allowed
/// "control" yet) it is asked again every 100 ticks.
#[derive(Default)]
struct SpawnGuard {
    subscribed: bool,
    warned: bool,
}

/// Whether `data` (an ask's JSON) is in the protected area.
fn protected(data: &Value) -> bool {
    let overworld = data["dimension"].as_str() == Some("minecraft:overworld");
    let (x, z) = (data["x"].as_f64(), data["z"].as_f64());
    let near = |v: Option<f64>| v.is_some_and(|v| v.abs() <= RADIUS);
    overworld && near(x) && near(z)
}

/// The answer to an ask: `cancel` with the reason told to the player, or none.
fn answer(name: &str, data: &Value) -> Option<String> {
    if !protected(data) {
        return None;
    }
    match name {
        "explosion" => Some("cancel".into()),
        "break_block" | "use_block" if data["permission"].as_u64().unwrap_or(0) < 2 => {
            Some("cancel The spawn area is protected".into())
        }
        _ => None,
    }
}

impl ServerMod for SpawnGuard {
    fn init() -> Result<(Self, Registration), String> {
        if bridge::version(API).is_none_or(|v| v < 2) {
            horizon_mod_sdk::warn(
                "Spawn Guard needs the Extras API 2 native mod (extras_api.hnmod)",
            );
        } else {
            bridge::call(API, "subscribe", ASKS)?;
        }
        Ok((SpawnGuard::default(), Registration::new().on_tick()))
    }

    fn tick(&mut self, tick: u64) {
        if !self.subscribed && tick % 100 == 0 && bridge::version(API).is_some_and(|v| v >= 2) {
            let _ = bridge::call(API, "subscribe", ASKS);
        }
    }

    fn bridge_reply(&mut self, _id: u64, api: &str, function: &str, answer: Result<&str, &str>) {
        if api != API || function != "subscribe" {
            return;
        }
        match answer {
            Ok(_) => self.subscribed = true,
            // Not allowed yet (Options > Mods > Extras API > Settings): said once.
            Err(why) if !self.warned => {
                self.warned = true;
                horizon_mod_sdk::warn(&format!("Spawn Guard: {why}"));
            }
            Err(_) => {}
        }
    }

    fn bridge_ask(&mut self, api: &str, name: &str, data: &str) -> Option<String> {
        if api != API {
            return None;
        }
        answer(name, &serde_json::from_str(data).ok()?)
    }
}

horizon_mod_sdk::export_server_mod!(SpawnGuard);

#[cfg(test)]
mod tests {
    use super::answer;
    use serde_json::json;
    #[test]
    fn only_the_area_around_spawn_in_the_overworld_is_guarded() {
        let at = |dimension: &str, x: f64, z: f64, permission: u64| json!({"dimension": dimension, "x": x, "z": z, "permission": permission});
        let cancel = Some("cancel The spawn area is protected".to_owned());
        assert_eq!(
            answer("break_block", &at("minecraft:overworld", 3.0, -16.0, 0)),
            cancel
        );
        assert_eq!(
            answer("break_block", &at("minecraft:overworld", 17.0, 0.0, 0)),
            None
        );
        assert_eq!(
            answer("break_block", &at("minecraft:the_nether", 0.0, 0.0, 0)),
            None
        );
        assert_eq!(
            answer("use_block", &at("minecraft:overworld", 0.0, 0.0, 2)),
            None
        );
        assert_eq!(
            answer("explosion", &at("minecraft:overworld", 0.0, 0.0, 0)).as_deref(),
            Some("cancel")
        );
    }
}
