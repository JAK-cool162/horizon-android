// Event mob_killed: {"player", "entity", "kind"}, a mob the player's melee blow killed.
fn mod_extras_api_killed(game: &mut crate::game::Game<'_>, player: u128, entity: u32, kind: &str) {
    use crate::game::mod_extras_api as m;
    m::notify(game, "mob_killed", || {
        serde_json::json!({"player": m::uuid(player), "entity": entity, "kind": kind})
    });
}
