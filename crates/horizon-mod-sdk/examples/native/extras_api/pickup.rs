// Ask pickup_item: {"player", "item", "count"}. Canceled, the stack isn't offered again for 10 ticks.
fn mod_extras_api_pickup(
    game: &mut crate::game::Game<'_>,
    player: u128,
    item: &str,
    count: u32,
    canceled: &mut bool,
) {
    use crate::game::mod_extras_api as m;
    if *canceled || m::subscribers("pickup_item").is_empty() {
        return;
    }
    let data = || serde_json::json!({"player": m::uuid(player), "item": item, "count": count});
    *canceled = m::ask(game, "pickup_item", data).is_some();
}
