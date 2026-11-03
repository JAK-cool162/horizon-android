//! Catalog thumbnails: a project's icon fetched once, made a square of the GUI atlas's icon slot
//! size, and which of the launcher's slots shows which. No JNI: the host content tests cover it.

/// A thumbnail's side, the atlas's pack icon slot (`hud::PACK_ICON_SIZE`).
pub const SIZE: u32 = 128;
/// The atlas's pack icon slots the launcher uses (of `hud::PACK_ICON_SLOTS`): more than a page
/// of rows, so scrolling back rarely writes a slot again.
pub const SLOTS: usize = 24;
/// The largest icon file fetched.
pub const MAX_BYTES: u64 = 2 << 20;
/// Icons fetched at once.
pub const PARALLEL: usize = 4;

/// An icon (PNG, JPEG, WebP, or a GIF's first frame) as a `SIZE` square of RGBA: its centre
/// cropped to a square, then scaled.
pub fn square(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let side = w.min(h);
    let cropped =
        image::imageops::crop_imm(&image, (w - side) / 2, (h - side) / 2, side, side).to_image();
    Some(
        image::imageops::resize(&cropped, SIZE, SIZE, image::imageops::FilterType::Triangle)
            .into_raw(),
    )
}

/// Which slot shows which icon, by its URL: the slot already holding it, else a free one, else the
/// one used longest ago.
#[derive(Default)]
pub struct Slots {
    held: Vec<(String, u64)>,
    clock: u64,
}

impl Slots {
    /// The slot for `url`, and whether its pixels have to be written.
    pub fn slot(&mut self, url: &str) -> (usize, bool) {
        self.clock += 1;
        if let Some(i) = self.held.iter().position(|(u, _)| u == url) {
            self.held[i].1 = self.clock;
            return (i, false);
        }
        if self.held.len() < SLOTS {
            self.held.push((url.to_owned(), self.clock));
            return (self.held.len() - 1, true);
        }
        let oldest = (0..self.held.len())
            .min_by_key(|&i| self.held[i].1)
            .unwrap_or(0);
        self.held[oldest] = (url.to_owned(), self.clock);
        (oldest, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icons_become_squares_of_their_centre() {
        // 4 x 2: red on the left quarter, green in the middle, blue on the right quarter.
        let mut image = image::RgbaImage::new(4, 2);
        for (x, _, p) in image.enumerate_pixels_mut() {
            *p = image::Rgba(match x {
                0 => [255, 0, 0, 255],
                3 => [0, 0, 255, 255],
                _ => [0, 255, 0, 255],
            });
        }
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let icon = square(&png).unwrap();
        assert_eq!(icon.len(), (SIZE * SIZE * 4) as usize);
        // The centre (green) only: the sides were cropped away.
        assert_eq!(&icon[..4], &[0, 255, 0, 255]);
        assert!(square(b"not an image").is_none());
    }
    #[test]
    fn slots_are_reused_and_the_oldest_is_given_up() {
        let mut slots = Slots::default();
        assert_eq!(slots.slot("a"), (0, true));
        assert_eq!(slots.slot("b"), (1, true));
        assert_eq!(slots.slot("a"), (0, false));
        for i in 2..SLOTS {
            slots.slot(&format!("u{i}"));
        }
        // Full: "b" was used longest ago ("a" was used again after it).
        assert_eq!(slots.slot("new"), (1, true));
        assert_eq!(slots.slot("a"), (0, false));
    }
}
