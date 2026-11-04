// The server's tps and mspt, at each tick's end, for mods' `bridge::get("extras", ...)`.
fn mod_extras_api_tick(
    game: &mut crate::game::Game<'_>,
    phase: crate::game::native_lifecycle::ServerTickPhase,
) {
    use crate::game::mod_extras_api as m;
    use crate::game::native_lifecycle::ServerTickPhase;
    if !matches!(phase, ServerTickPhase::ServerTickEnd { .. }) {
        return;
    }
    let on = game.native_on(m::ID);
    let (rate, mspt) = game.native_tick_stats();
    let tps = on.then(|| format!("{:.1}", m::tps(rate, mspt)));
    let mspt = on.then(|| format!("{mspt:.1}"));
    for api in [m::API, m::OLD_API] {
        game.native_bridge_publish(api, "tps", tps.as_deref());
        game.native_bridge_publish(api, "mspt", mspt.as_deref());
    }
}
