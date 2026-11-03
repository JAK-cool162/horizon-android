// Ask attack_entity: the acting player, and {"entity", "kind", "at": [x, y, z]}.
fn mod_extras_api_attack(
    game: &mut crate::game::Game<'_>,
    entity: i32,
    kind: &str,
    pos: [f64; 3],
    canceled: &mut bool,
) {
    use crate::game::mod_extras_api as m;
    if *canceled || m::subscribers("attack_entity").is_empty() {
        return;
    }
    let data = serde_json::json!({"entity": entity, "kind": kind, "at": pos});
    *canceled = m::ask_acting(game, "attack_entity", data);
}
