// Event player_leave: {"player", "name"}.
fn mod_extras_api_leave(game: &mut crate::game::Game<'_>, player: u128, name: &str) {
    use crate::game::mod_extras_api as m;
    m::notify(game, "player_leave", || serde_json::json!({"player": m::uuid(player), "name": name}));
}
