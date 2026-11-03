// A click on the Extras API's page: allows or takes back that permission.
fn mod_extras_api_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::mod_extras_api as m;
    if mod_id == m::ID {
        m::settings_click(game, button);
    }
}
