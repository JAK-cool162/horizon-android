//! Example Horizon server mod on API level 8: right-click a ripe crop and its loot drops
//! (`world.break-blocks`) and it is replanted (`world.set-blocks`). A sandboxed `.hmod`: the
//! game loads it at runtime, with no rebuild.
use horizon_mod_sdk::server::{BlockChange, BlockPos, Registration, ServerMod, world};

struct RightHarvest;

/// The age at which each crop is ripe.
fn ripe_age(block: &str) -> Option<u32> {
    match block {
        "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" => Some(7),
        "minecraft:beetroots" | "minecraft:nether_wart" => Some(3),
        "minecraft:cocoa" => Some(2),
        _ => None,
    }
}

/// `minecraft:cocoa[age=2,facing=north]` replanted: the same state with `age=0`, when it is a
/// ripe crop.
fn replanted(state: &str) -> Option<String> {
    let (block, properties) = state.split_once('[')?;
    let properties = properties.strip_suffix(']')?;
    let ripe = ripe_age(block)?;
    let mut ripe_now = false;
    let replanted: Vec<String> = properties
        .split(',')
        .map(|p| match p.split_once('=') {
            Some(("age", age)) => {
                ripe_now = age.parse() == Ok(ripe);
                "age=0".to_owned()
            }
            _ => p.to_owned(),
        })
        .collect();
    ripe_now.then(|| format!("{block}[{}]", replanted.join(",")))
}

impl ServerMod for RightHarvest {
    fn init() -> Result<(Self, Registration), String> {
        Ok((RightHarvest, Registration::new().on_blocks()))
    }

    fn block_used(
        &mut self,
        _player: &str,
        dimension: &str,
        pos: BlockPos,
        state: &str,
        _item: Option<&str>,
    ) {
        // `state` is the block when it was clicked; the game, another mod or a player may have
        // changed it since (the event arrives a tick later), so act on what is there now.
        let now = world::get_block(dimension, pos.x, pos.y, pos.z);
        let Some(young) = replanted(now.as_deref().unwrap_or(state)) else {
            return;
        };
        // In this order: the ripe crop drops its loot, then the seedling takes its place.
        let _ = world::break_blocks(dimension, &[(pos.x, pos.y, pos.z)], true);
        let _ = world::set_blocks(
            dimension,
            &[BlockChange {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                state: young,
            }],
        );
    }
}

horizon_mod_sdk::export_server_mod!(RightHarvest);

#[cfg(test)]
mod tests {
    use super::replanted;
    #[test]
    fn only_ripe_crops_are_replanted_keeping_their_other_properties() {
        assert_eq!(
            replanted("minecraft:wheat[age=7]").as_deref(),
            Some("minecraft:wheat[age=0]")
        );
        assert_eq!(
            replanted("minecraft:cocoa[age=2,facing=north]").as_deref(),
            Some("minecraft:cocoa[age=0,facing=north]")
        );
        assert_eq!(replanted("minecraft:wheat[age=6]"), None);
        assert_eq!(replanted("minecraft:stone"), None);
        assert_eq!(replanted("minecraft:oak_sapling[stage=1]"), None);
    }
}
