//! Horizon: a resource pack's animated backgrounds behind screens, off unless a pack gives one.
//! A pack puts a GIF (or an animated strip with its `.png.mcmeta`, as vanilla animates
//! textures) at `assets/minecraft/textures/gui/sprites/horizon/background/<screen>`: `menu`
//! behind menus, `inventory`, `creative`, a workstation's name (`crafting_table`, `furnace`...)
//! or `container` behind every container screen. The pack overlay turns a GIF into a strip
//! and its `.mcmeta` ([`gif_to_strip`]); the GUI atlas keeps one frame's region and the frames
//! ([`Frames`]), and the HUD copies the current one in as they turn.
use std::io::Cursor;

/// Where the backgrounds are, under `textures/`.
pub const DIR: &str = "gui/sprites/horizon/background/";
/// The largest frame the GUI atlas takes (`HudAtlas::build`'s limit for GUI textures).
pub const MAX_WIDTH: u32 = 1024;
pub const MAX_HEIGHT: u32 = 512;
/// The frames' memory, all frames together, and the most frames kept: a long GIF is scaled
/// down, then cut.
pub const MAX_BYTES: u64 = 48 << 20;
pub const MAX_FRAMES: usize = 240;

/// An animated background's frames, each `width x height` RGBA, and how many ticks each shows.
#[derive(Debug)]
pub struct Frames {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Vec<u8>>,
    pub ticks: Vec<u32>,
}

impl Frames {
    /// The frame shown at `tick` (the animation loops).
    pub fn frame_at(&self, tick: u64) -> usize {
        let total: u64 = self.ticks.iter().map(|&t| u64::from(t)).sum();
        if total == 0 {
            return 0;
        }
        let mut t = tick % total;
        for (i, &ticks) in self.ticks.iter().enumerate() {
            if t < u64::from(ticks) {
                return i;
            }
            t -= u64::from(ticks);
        }
        0
    }
}

/// A GIF's delay as ticks: 50 ms each, at least one (the HUD animates on ticks).
fn ticks_of(millis: u32) -> u32 {
    ((millis + 25) / 50).max(1)
}

/// The size frames of `w x h` are scaled to so that `count` of them fit [`MAX_WIDTH`],
/// [`MAX_HEIGHT`] and [`MAX_BYTES`], keeping their shape.
fn fit(w: u32, h: u32, count: usize) -> (u32, u32) {
    let by_side = (f64::from(MAX_WIDTH) / f64::from(w)).min(f64::from(MAX_HEIGHT) / f64::from(h));
    let bytes = f64::from(w) * f64::from(h) * 4.0 * count as f64;
    let by_bytes = (MAX_BYTES as f64 / bytes).sqrt();
    let scale = by_side.min(by_bytes).min(1.0);
    (
        ((f64::from(w) * scale).floor() as u32).max(1),
        ((f64::from(h) * scale).floor() as u32).max(1),
    )
}

/// A GIF as a vertical strip of its frames (PNG) and the `.png.mcmeta` that animates it, its
/// frames scaled to fit the GUI atlas and the memory budget.
pub fn gif_to_strip(bytes: &[u8]) -> Result<(Vec<u8>, String), String> {
    use image::AnimationDecoder;
    let decoder =
        image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let frames: Vec<image::Frame> = decoder
        .into_frames()
        .take(MAX_FRAMES)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let first = frames.first().ok_or("a GIF without frames")?;
    let (w, h) = first.buffer().dimensions();
    let (fw, fh) = fit(w, h, frames.len());
    let mut strip = Vec::with_capacity((fw * fh * 4) as usize * frames.len());
    let mut list = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let image = if (fw, fh) == (w, h) {
            frame.buffer().clone()
        } else {
            image::imageops::resize(
                frame.buffer(),
                fw,
                fh,
                image::imageops::FilterType::Triangle,
            )
        };
        strip.extend_from_slice(image.as_raw());
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let millis = numerator.checked_div(denominator.max(1)).unwrap_or(100);
        list.push(serde_json::json!({"index": i, "time": ticks_of(millis)}));
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, fw, fh * frames.len() as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&strip).map_err(|e| e.to_string())?;
    }
    let mcmeta = serde_json::json!({"animation": {"width": fw, "height": fh, "frames": list}});
    Ok((png, mcmeta.to_string()))
}

/// A strip (`w x h` RGBA) and its `.mcmeta` read back into frames: the frame size the mcmeta
/// names (the strip's width, square, by default), the frames in its list or in order, and
/// their ticks (`frametime`, 1 by default). None when it has no animation or doesn't fit.
pub fn from_strip(w: u32, h: u32, pixels: &[u8], mcmeta: &str) -> Option<Frames> {
    let meta: serde_json::Value = serde_json::from_str(mcmeta).ok()?;
    let animation = meta.get("animation")?;
    let size = |key: &str| animation.get(key).and_then(serde_json::Value::as_u64);
    let fw = size("width").map_or(w, |v| v as u32);
    let fh = size("height").map_or(fw.min(h), |v| v as u32);
    if fw == 0 || fh == 0 || fw != w || !h.is_multiple_of(fh) || fw > MAX_WIDTH || fh > MAX_HEIGHT {
        return None;
    }
    let count = (h / fh) as usize;
    let frame_bytes = (fw * fh * 4) as usize;
    if pixels.len() < frame_bytes * count {
        return None;
    }
    let default_ticks = size("frametime").map_or(1, |t| (t as u32).max(1));
    let order: Vec<(usize, u32)> = match animation.get("frames").and_then(|f| f.as_array()) {
        Some(list) => list
            .iter()
            .filter_map(|f| match f {
                serde_json::Value::Number(n) => Some((n.as_u64()? as usize, default_ticks)),
                f => Some((
                    f.get("index")?.as_u64()? as usize,
                    f.get("time")
                        .and_then(|t| t.as_u64())
                        .map_or(default_ticks, |t| (t as u32).max(1)),
                )),
            })
            .filter(|(i, _)| *i < count)
            .take(MAX_FRAMES)
            .collect(),
        None => (0..count.min(MAX_FRAMES))
            .map(|i| (i, default_ticks))
            .collect(),
    };
    if order.is_empty() {
        return None;
    }
    let frame = |i: usize| pixels[i * frame_bytes..(i + 1) * frame_bytes].to_vec();
    Some(Frames {
        width: fw,
        height: fh,
        frames: order.iter().map(|&(i, _)| frame(i)).collect(),
        ticks: order.iter().map(|&(_, t)| t).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-frame 4 x 2 GIF: red for 100 ms, then blue for 20 ms.
    fn gif() -> Vec<u8> {
        use image::codecs::gif::GifEncoder;
        let mut out = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut out);
            for (color, millis) in [([255u8, 0, 0, 255], 100), ([0, 0, 255, 255], 20)] {
                let buffer = image::RgbaImage::from_pixel(4, 2, image::Rgba(color));
                let delay = image::Delay::from_numer_denom_ms(millis, 1);
                encoder
                    .encode_frame(image::Frame::from_parts(buffer, 0, 0, delay))
                    .unwrap();
            }
        }
        out
    }

    #[test]
    fn a_gif_becomes_a_strip_that_animates_back_frame_by_frame() {
        let (png, mcmeta) = gif_to_strip(&gif()).unwrap();
        let mut decoder = png::Decoder::new(Cursor::new(png));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (4, 4));
        let frames = from_strip(info.width, info.height, &pixels, &mcmeta).unwrap();
        assert_eq!((frames.width, frames.height), (4, 2));
        // 100 ms is 2 ticks; 20 ms rounds to none, and shows for one.
        assert_eq!(frames.ticks, [2, 1]);
        assert_eq!(&frames.frames[0][..4], &[255, 0, 0, 255]);
        assert_eq!(&frames.frames[1][..4], &[0, 0, 255, 255]);
        assert_eq!([0, 1, 2, 3].map(|t| frames.frame_at(t)), [0, 0, 1, 0]);
    }

    #[test]
    fn big_or_long_gifs_are_scaled_to_the_atlas_and_the_budget() {
        assert_eq!(fit(800, 400, 1), (800, 400));
        assert_eq!(fit(2048, 1024, 1), (1024, 512));
        // 60 frames of 1024 x 512 is 120 MiB: scaled to about 48.
        let (w, h) = fit(1024, 512, 60);
        assert!(
            u64::from(w * h) * 4 * 60 <= MAX_BYTES && w > 600,
            "{w} x {h}"
        );
        assert_eq!(w / h, 2);
    }

    #[test]
    fn a_vanilla_style_strip_animates_with_its_frametime() {
        let pixels: Vec<u8> = (0..4 * 4 * 4).map(|i| (i / 16) as u8).collect();
        let frames =
            from_strip(2, 8, &pixels, r#"{"animation":{"frametime":3,"height":2}}"#).unwrap();
        assert_eq!(frames.frames.len(), 4);
        assert_eq!(frames.ticks, [3, 3, 3, 3]);
        assert!(from_strip(2, 8, &pixels, r#"{"texture":{"blur":true}}"#).is_none());
    }
}
