// A sandboxed mod's call to `extras`; calls to other APIs are another mod's.
fn mod_extras_api_call(
    game: &mut crate::game::Game<'_>,
    call: &crate::game::native_hooks::NativeBridgeCall<'_>,
    reply: &mut Option<Result<String, String>>,
) {
    use crate::game::mod_extras_api as m;
    if call.api != m::API || reply.is_some() || !game.native_on(m::ID) {
        return;
    }
    *reply = Some(m::serve(game, call));
}
