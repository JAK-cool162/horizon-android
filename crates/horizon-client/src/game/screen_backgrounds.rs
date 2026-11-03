//! Resource packs' backgrounds behind container screens (`horizon_client::gui_backgrounds`):
//! the screen's own (`inventory`, `crafting_table`, `chest`, a workstation's...), else
//! `container`. Nothing is drawn unless a pack gives one.
use super::{Game, stations::Kind};
use horizon_client::{containers::BlockEntity, hud::HudBatch};

impl Game<'_> {
    /// The name of the open container screen's background.
    fn screen_background(&self) -> &'static str {
        if let Some(s) = &self.station {
            return match s.kind {
                Kind::Enchanting => "enchanting_table",
                Kind::Anvil => "anvil",
                Kind::Grindstone => "grindstone",
                Kind::Smithing => "smithing_table",
                Kind::Stonecutter => "stonecutter",
                Kind::Merchant => "villager",
                Kind::Beacon => "beacon",
                Kind::Cartography => "cartography_table",
                Kind::Loom => "loom",
            };
        }
        match self.container_kind() {
            Some(BlockEntity::Chest { .. }) => "chest",
            Some(BlockEntity::Furnace { .. }) => "furnace",
            Some(BlockEntity::Brewing { .. }) => "brewing_stand",
            Some(BlockEntity::Crafter { .. }) => "crafter",
            Some(_) => "container",
            None if self.craft_size == 3 => "crafting_table",
            None => "inventory",
        }
    }
    /// Over the dark fill behind the open container screen: a pack's background for it.
    pub(super) fn draw_screen_background(&self, batch: &mut HudBatch) {
        self.draw_named_background(batch, &[self.screen_background(), "container"]);
    }
    /// A pack's background for the first of `screens` it has, over the whole screen.
    pub(super) fn draw_named_background(&self, batch: &mut HudBatch, screens: &[&str]) {
        let (w, h) = (batch.width, batch.height);
        horizon_client::ui::pack_background(batch, &self.gui.atlas, screens, 0.0, 0.0, w, h);
    }
}
