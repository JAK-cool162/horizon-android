// Ask command: {"player", "name", "command"} (without its slash), any command a player sends.
// Canceled, it doesn't run, and the reason is the player's answer.
fn mod_extras_api_command(
    game: &mut crate::game::Game<'_>,
    player: u128,
    name: &str,
    command: &str,
    canceled: &mut bool,
    replies: &mut Vec<horizon_client::text::Text>,
) {
    use crate::game::mod_extras_api as m;
    if *canceled || m::subscribers("command").is_empty() {
        return;
    }
    let data = || serde_json::json!({"player": m::uuid(player), "name": name, "command": command});
    if let Some(why) = m::ask(game, "command", data) {
        *canceled = true;
        if !why.is_empty() {
            replies.push(horizon_client::text::Text::literal(why));
        }
    }
}
