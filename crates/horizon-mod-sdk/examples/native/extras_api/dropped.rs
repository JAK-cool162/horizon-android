// Event item_dropped: {"player", "item", "count"}.
fn mod_extras_api_dropped(game: &mut crate::game::Game<'_>, player: u128, item: &str, count: u32) {
    use crate::game::mod_extras_api as m;
    m::notify(game, "item_dropped", || {
        serde_json::json!({"player": m::uuid(player), "item": item, "count": count})
    });
}
