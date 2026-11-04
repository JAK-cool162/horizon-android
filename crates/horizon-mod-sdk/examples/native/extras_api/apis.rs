// The API this mod serves, while it is on.
fn mod_extras_api_apis(game: &crate::game::Game<'_>, apis: &mut Vec<(String, u32)>) {
    use crate::game::mod_extras_api as m;
    if game.native_on(m::ID) {
        apis.push((m::API.into(), m::API_VERSION));
        apis.push((m::OLD_API.into(), m::API_VERSION));
    }
}
