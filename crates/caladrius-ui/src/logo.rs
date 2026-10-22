//! The Caladrius logo (white bird on green, drawn by Amina; `assets/logo/` holds the SVGs). The
//! interface and the window use PNG renderings of it from `assets/icons/`, which
//! `assets/icons/render.sh` regenerates from the SVGs: the full drawing at 256 px for large sizes,
//! the simplified one at 64 px for the top bar.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

/// The full drawing, 256 × 256: the welcome page and the window icon.
pub const ICON_256: &[u8] = include_bytes!("../../../assets/icons/caladrius-icon-256.png");
/// The simplified drawing, 64 × 64: the top bar.
pub const ICON_64: &[u8] = include_bytes!("../../../assets/icons/caladrius-icon-64.png");

/// A decoded image: 8-bit RGBA rows, not premultiplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes a PNG to RGBA. Every icon of `assets/icons/` is 8-bit RGBA; any other layout is an
/// error that names it.
pub fn decode(png_bytes: &[u8]) -> Result<Pixels, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("the logo image is unreadable: {e}"))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "the logo image is too large".to_owned())?;
    let mut rgba = vec![0_u8; size];
    let info = reader
        .next_frame(&mut rgba)
        .map_err(|e| format!("the logo image is unreadable: {e}"))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "the logo image is {:?} {:?}, not 8-bit RGBA",
            info.color_type, info.bit_depth
        ));
    }
    rgba.truncate(info.buffer_size());
    Ok(Pixels {
        width: info.width,
        height: info.height,
        rgba,
    })
}

/// The two renderings, loaded once as textures.
pub struct Logo {
    /// The full drawing.
    pub large: TextureHandle,
    /// The simplified drawing for small sizes.
    pub small: TextureHandle,
}

impl Logo {
    /// Uploads both renderings to `ctx`. `None` if an image cannot be decoded: the interface then
    /// shows the name without the picture.
    pub fn load(ctx: &Context) -> Option<Logo> {
        let texture = |name: &str, bytes: &[u8]| {
            let p = decode(bytes).ok()?;
            let size = [
                usize::try_from(p.width).ok()?,
                usize::try_from(p.height).ok()?,
            ];
            let image = ColorImage::from_rgba_unmultiplied(size, &p.rgba);
            Some(ctx.load_texture(name, image, TextureOptions::LINEAR))
        };
        Some(Logo {
            large: texture("caladrius-logo-256", ICON_256)?,
            small: texture("caladrius-logo-64", ICON_64)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_renderings_decode_as_square_rgba_with_round_corners() {
        for (bytes, side) in [(ICON_256, 256), (ICON_64, 64)] {
            let p = decode(bytes).unwrap();
            assert_eq!((p.width, p.height), (side, side));
            assert_eq!(p.rgba.len(), (side * side * 4) as usize);
            // The corner is transparent (rounded); the middle of the top edge is the logo green.
            assert_eq!(&p.rgba[..4], &[0, 0, 0, 0]);
            let top_middle = (side / 2 * 4) as usize + 3 * (side * 4) as usize;
            assert_eq!(
                &p.rgba[top_middle..top_middle + 4],
                &[0x83, 0xd3, 0x83, 0xff]
            );
        }
    }

    #[test]
    fn a_broken_image_says_so() {
        let e = decode(b"not a png").unwrap_err();
        assert!(e.contains("unreadable"), "{e}");
    }
}
