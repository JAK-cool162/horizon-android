//! Example Horizon server mod on the mod bridge (capability `bridge`, API level 9): a sandboxed
//! `.hmod` using an API that a native mod adds. The Extras API native mod (`extras_api.hnmod`)
//! serves `extras`: `command` runs a vanilla command as the server's console, and it publishes the
//! server's `tps` and `mspt`. This mod gives each new player a starter kit with it, and adds `/tps`.
use horizon_mod_sdk::bridge;
use horizon_mod_sdk::server::{Args, Player, Registration, ServerMod, players};
use horizon_mod_sdk::storage;
use std::collections::BTreeMap;

/// The native API this mod uses, and the version it was written for.
const API: &str = "extras";
const VERSION: u32 = 1;
/// The starter kit: item and count.
const KIT: [(&str, u32); 3] = [
    ("minecraft:stone_pickaxe", 1),
    ("minecraft:bread", 8),
    ("minecraft:torch", 16),
];

/// A player's kit on its way: the calls not answered yet.
struct Giving {
    player: String,
    name: String,
    left: usize,
    failed: Option<String>,
}

#[derive(Default)]
struct WelcomeKit {
    /// The ids of the players who got their kit, one per line in storage.
    given: Vec<String>,
    /// By the id `bridge::call` returned: whose kit the call is part of.
    calls: BTreeMap<u64, usize>,
    giving: Vec<Giving>,
}

/// The storage key of the players who got their kit.
const GIVEN: &str = "given";

/// `give Alex minecraft:bread 8`.
fn give_line(name: &str, item: &str, count: u32) -> String {
    format!("give {name} {item} {count}")
}

impl ServerMod for WelcomeKit {
    fn init() -> Result<(Self, Registration), String> {
        let given = storage::get(GIVEN).unwrap_or_default();
        let given = String::from_utf8_lossy(&given)
            .lines()
            .map(str::to_owned)
            .collect();
        Ok((
            WelcomeKit {
                given,
                ..WelcomeKit::default()
            },
            Registration::new().on_joins().command("tps"),
        ))
    }

    fn player_joined(&mut self, player: &Player) {
        if self.given.contains(&player.id) {
            return;
        }
        if bridge::version(API).is_none_or(|v| v < VERSION) {
            players::tell(
                &player.id,
                "No starter kit: this server needs the Extras API native mod (extras_api.hnmod)",
            );
            return;
        }
        let kit = self.giving.len();
        let mut left = 0;
        for (item, count) in KIT {
            if let Ok(id) = bridge::call(API, "command", &give_line(&player.name, item, count)) {
                self.calls.insert(id, kit);
                left += 1;
            }
        }
        self.giving.push(Giving {
            player: player.id.clone(),
            name: player.name.clone(),
            left,
            failed: None,
        });
    }

    fn bridge_reply(&mut self, id: u64, _api: &str, _function: &str, answer: Result<&str, &str>) {
        let Some(kit) = self.calls.remove(&id) else {
            return;
        };
        let giving = &mut self.giving[kit];
        giving.left -= 1;
        if let Err(why) = answer {
            giving.failed.get_or_insert_with(|| why.to_owned());
        }
        if giving.left > 0 {
            return;
        }
        match &giving.failed {
            None => {
                self.given.push(giving.player.clone());
                let _ = storage::set(GIVEN, self.given.join("\n").as_bytes());
                players::tell(
                    &giving.player,
                    &format!("Welcome, {}! Here is your starter kit.", giving.name),
                );
            }
            Some(why) => {
                players::tell(&giving.player, &format!("Your starter kit failed: {why}"));
            }
        }
    }

    fn command(
        &mut self,
        _player: &Player,
        _name: &str,
        _args: &Args,
    ) -> Result<Option<String>, String> {
        match (bridge::get(API, "tps"), bridge::get(API, "mspt")) {
            (Some(tps), Some(mspt)) => Ok(Some(format!("{tps} TPS, {mspt} ms per tick"))),
            _ => Err("Install the Extras API native mod (extras_api.hnmod) for /tps".into()),
        }
    }
}

horizon_mod_sdk::export_server_mod!(WelcomeKit);

#[cfg(test)]
mod tests {
    #[test]
    fn give_lines_are_vanilla_give_commands() {
        assert_eq!(
            super::give_line("Alex", "minecraft:bread", 8),
            "give Alex minecraft:bread 8"
        );
    }
}
