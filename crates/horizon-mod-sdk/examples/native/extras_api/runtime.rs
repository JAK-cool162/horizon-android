//! Extras API: the native API `extras` for sandboxed `.hmod` mods (capability `bridge`), which
//! the game gets as `crate::game::mod_extras_api`. Version 1:
//!
//! - function `command`, server mods: runs one vanilla command line (with or without its slash)
//!   as the server's console does, at permission level 4. The answer is empty when it ran, an
//!   error when it failed.
//! - values `tps` and `mspt`, on the server: the ticks a second it keeps up and the milliseconds
//!   its ticks take, with one decimal, as `/tick query` reports them.

/// The mod's id (its settings folder), name and version.
pub const ID: &str = "extras_api";
pub const NAME: &str = "Extras API";
pub const VERSION: &str = "1.0.0";
pub const DESCRIPTION: &str = "Gives sandboxed mods the API extras: vanilla commands as the server's console, and the server's TPS. Mods using it can do what a server operator can.";
/// The API's name and version, as mods ask for them.
pub const API: &str = "extras";
pub const API_VERSION: u32 = 1;

/// The ticks a second the server keeps up: its rate, or 1000 / milliseconds when that is lower.
pub fn tps(rate: f32, mspt: f64) -> f64 {
    let rate = f64::from(rate);
    if mspt > 0.0 { rate.min(1000.0 / mspt) } else { rate }
}

/// `command`'s argument as the command line to run.
pub fn command_line(args: &str) -> Result<&str, String> {
    let line = args.trim();
    let line = line.strip_prefix('/').unwrap_or(line);
    if line.is_empty() {
        return Err("command takes a command line, such as give Alex minecraft:bread 8".into());
    }
    if line.contains(['\n', '\r']) {
        return Err("command runs one line per call".into());
    }
    Ok(line)
}

/// A mod's call to `extras`: its answer.
pub fn serve(
    game: &mut crate::game::Game<'_>,
    call: &crate::game::native_hooks::NativeBridgeCall<'_>,
) -> Result<String, String> {
    match call.function {
        "command" if !call.server => Err("command is for server mods".into()),
        "command" => {
            let line = command_line(call.args)?;
            if game.native_run_command(line) {
                Ok(String::new())
            } else {
                Err(format!("/{line} failed"))
            }
        }
        other => Err(format!("{API} has no function {other}")),
    }
}
