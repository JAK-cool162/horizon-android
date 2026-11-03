//! The mod bridge (capability `bridge`, docs/android-native-mods.md): native mods (`.hnmod`)
//! serve APIs of their own to sandboxed `.hmod` mods, so a new API needs no new function in
//! the mods' WIT contract. The hook points are `native_bridge_apis` and `native_bridge_call`
//! (`native_hooks.rs`); the helpers here publish values and send events. Calls are answered,
//! and values and events handed over, before each tick's mod hooks ([`Game::serve_bridge`]).
use super::Game;
use std::collections::BTreeMap;

/// A sandboxed mod's `bridge.call`, as `native_bridge_call` hears it.
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(crate) struct NativeBridgeCall<'a> {
    /// The calling mod's id (`horizon-mod.toml`).
    pub(crate) mod_id: &'a str,
    /// A server mod, running where the world is simulated; else a client mod
    /// in the player's window.
    pub(crate) server: bool,
    /// The API and function the mod named, and its arguments, in the form the
    /// API documents.
    pub(crate) api: &'a str,
    pub(crate) function: &'a str,
    pub(crate) args: &'a str,
}

/// Calls and events kept for the next tick's mod hooks, beyond which the
/// oldest are dropped: a bound for a mod that calls every frame.
const QUEUED: usize = 1024;

/// What the bridge keeps between ticks.
#[derive(Default)]
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(super) struct BridgeState {
    /// The values native mods published, by API and key (`bridge.get`).
    values: BTreeMap<(String, String), String>,
    /// Calls since the last tick's hooks: the calling mod, whether a server
    /// mod, the id it got, the API, the function and the arguments.
    calls: Vec<(String, bool, u64, String, String, String)>,
    /// Native APIs' events since then: API, name and data.
    events: Vec<(String, String, String)>,
}

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Horizon does.
#[allow(dead_code)]
impl Game<'_> {
    /// Publishes `value` under `key` of the native API `api`, for mods'
    /// `bridge.get` from the next tick's hooks on; `None` takes it away. A
    /// value stays until changed, across mod reloads. API @API:mod_bridge@.
    pub(super) fn native_bridge_publish(&mut self, api: &str, key: &str, value: Option<&str>) {
        let values = &mut self.mods.bridge.values;
        let key = (api.to_owned(), key.to_owned());
        match value {
            Some(v) => {
                values.insert(key, v.to_owned());
            }
            None => {
                values.remove(&key);
            }
        }
    }
    /// Tells every running mod with capability `bridge` an event of the
    /// native API `api` (`bridge-events.event`), before the next tick's
    /// hooks, in the order sent. API @API:mod_bridge@.
    pub(super) fn native_bridge_event(&mut self, api: &str, name: &str, data: &str) {
        let events = &mut self.mods.bridge.events;
        if events.len() == QUEUED {
            events.remove(0);
        }
        events.push((api.to_owned(), name.to_owned(), data.to_owned()));
    }
}

#[cfg(feature = "mods")]
impl Game<'_> {
    /// A mod's `bridge.call`, for the next tick's hooks.
    pub(super) fn queue_bridge_call(
        &mut self,
        mod_id: String,
        server: bool,
        id: u64,
        api: String,
        function: String,
        args: String,
    ) {
        let calls = &mut self.mods.bridge.calls;
        if calls.len() == QUEUED {
            calls.remove(0);
        }
        calls.push((mod_id, server, id, api, function, args));
    }

    /// Before a tick's mod hooks: what native mods serve now (their APIs and
    /// values), the calls since the last tick answered, and their events told.
    pub(super) fn serve_bridge(&mut self) {
        if !self.mods.mods.wants_bridge() {
            self.mods.bridge.calls.clear();
            self.mods.bridge.events.clear();
            return;
        }
        let mut apis = Vec::new();
        self.native_bridge_apis(&mut apis);
        let values = self.mods.bridge.values.clone();
        self.mods
            .mods
            .set_bridge(apis.into_iter().collect(), values);
        for (mod_id, server, id, api, function, args) in std::mem::take(&mut self.mods.bridge.calls)
        {
            let call = NativeBridgeCall {
                mod_id: &mod_id,
                server,
                api: &api,
                function: &function,
                args: &args,
            };
            let mut reply = None;
            self.native_bridge_call(&call, &mut reply);
            let answer = reply.unwrap_or_else(|| Err(horizon_mods::bridge_unserved(&api)));
            self.mods.mods.bridge_reply(
                &mod_id,
                id,
                &api,
                &function,
                answer.as_deref().map_err(String::as_str),
            );
        }
        for (api, name, data) in std::mem::take(&mut self.mods.bridge.events) {
            self.mods.mods.bridge_event(&api, &name, &data);
        }
    }
}

/// Without code mods nothing calls the bridge: its hook points stay for the
/// native mods' sake (`native_hooks.rs` holds no `cfg`).
#[cfg(not(feature = "mods"))]
#[allow(dead_code)]
fn unused_without_mods(game: &mut Game<'_>) {
    let call = NativeBridgeCall {
        mod_id: "",
        server: false,
        api: "",
        function: "",
        args: "",
    };
    game.native_bridge_apis(&mut Vec::new());
    game.native_bridge_call(&call, &mut None);
}
