// Ask explosion: {"dimension", "x", "y", "z", "blocks"} (how many it would break). Canceled, it
// breaks no block; entities are hurt as before.
fn mod_extras_api_explosion(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    center: [f64; 3],
    blocks: &mut Vec<[i32; 3]>,
) {
    use crate::game::mod_extras_api as m;
    if blocks.is_empty() || m::subscribers("explosion").is_empty() {
        return;
    }
    let count = blocks.len();
    let data = || {
        serde_json::json!({"dimension": dimension, "x": center[0], "y": center[1], "z": center[2], "blocks": count})
    };
    if m::ask(game, "explosion", data).is_some() {
        blocks.clear();
    }
}
