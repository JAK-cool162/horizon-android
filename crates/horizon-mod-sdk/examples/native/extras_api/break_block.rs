// Ask break_block: the acting player, and {"x", "y", "z", "block"} (`minecraft:oak_log[axis=y]`).
fn mod_extras_api_break_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    state: horizon_world::registry::BlockStateId,
    canceled: &mut bool,
) {
    use crate::game::mod_extras_api as m;
    if *canceled || m::subscribers("break_block").is_empty() {
        return;
    }
    let block = game.state_string(state);
    let data = serde_json::json!({"x": pos[0], "y": pos[1], "z": pos[2], "block": block});
    *canceled = m::ask_acting(game, "break_block", data);
}
