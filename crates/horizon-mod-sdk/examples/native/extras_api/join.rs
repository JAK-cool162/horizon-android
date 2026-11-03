// Event player_join: {"player", "name"}.
fn mod_extras_api_join(game: &mut crate::game::Game<'_>, player: u128, name: &str) {
    use crate::game::mod_extras_api as m;
    m::notify(game, "player_join", || serde_json::json!({"player": m::uuid(player), "name": name}));
}
