// Ask use_block: the acting player, and {"x", "y", "z", "face": [dx, dy, dz], "block"}. Once
// per hand, as the game handles a right click.
fn mod_extras_api_use_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    use crate::game::mod_extras_api as m;
    if *canceled || m::subscribers("use_block").is_empty() {
        return;
    }
    let block = game.state_string(game.world.block(pos[0], pos[1], pos[2]));
    let data = serde_json::json!({"x": pos[0], "y": pos[1], "z": pos[2], "face": face, "block": block});
    *canceled = m::ask_acting(game, "use_block", data);
}
