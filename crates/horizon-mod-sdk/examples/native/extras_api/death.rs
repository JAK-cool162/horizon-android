// Event player_death: {"player", "cause"} (the death message's key, `death.attack.lava`, or null).
fn mod_extras_api_death(game: &mut crate::game::Game<'_>, player: u128, cause: Option<&str>) {
    use crate::game::mod_extras_api as m;
    m::notify(game, "player_death", || serde_json::json!({"player": m::uuid(player), "cause": cause}));
}
