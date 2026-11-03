// The Extras API's page in Options > Mods: the permissions mods asked for, each a switch.
fn mod_extras_api_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::mod_extras_api as m;
    if mod_id == m::ID {
        buttons.extend(m::settings_buttons(game));
    }
}
