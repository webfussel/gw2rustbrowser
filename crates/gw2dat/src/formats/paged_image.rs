//! Paged images (PF `PIMG`, chunk `PGTB`): large images cut into pages.
//!
//! The world maps of the game are stored this way (the largest has 80×112
//! pages of 512×512 pixels), as are the maps of single regions. Each layer
//! has its own page size and pixel format; a page sits at a grid coordinate
//! and is either a texture file (ATEP) or a solid colour.

use rayon::prelude::*;

use super::image::RgbaImage;
use super::pf::PackFile;
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const PIMG: u32 = fcc(b"PIMG");
const PGTB: u32 = fcc(b"PGTB");

#[derive(Clone, Debug)]
pub struct PagedImage {
    pub layers: Vec<Layer>,
    pub pages: Vec<Page>,
    pub flags: u32,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub raw_size: [u32; 2],
    /// Size of one page in pixels.
    pub page_size: [u32; 2],
    pub raw_format: u32,
    pub format: u32,
    pub disk_format: u32,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub layer: u32,
    /// Grid column and row; x grows to the right, y downwards.
    pub x: u32,
    pub y: u32,
    /// Texture of the page; `None` for a solid page.
    pub file: Option<u32>,
    pub flags: u32,
    /// Colour of a solid page, RGBA.
    pub color: [u8; 4],
    /// From the raw (not stripped) page list.
    pub raw: bool,
}

/// Grid bounds of a layer's pages: `[min x, min y, max x, max y]`.
pub fn bounds(image: &PagedImage, layer: u32) -> Option<[u32; 4]> {
    image.pages.iter().filter(|p| p.layer == layer).fold(None, |b, p| {
        Some(match b {
            None => [p.x, p.y, p.x, p.y],
            Some([x0, y0, x1, y1]) => [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
        })
    })
}

pub fn decode(data: &[u8]) -> Result<PagedImage> {
    let file = PackFile::parse(data)?;
    if file.file_type != PIMG {
        return Err(Error::format("paged image", "not a PIMG file"));
    }
    let chunk = file.require_chunk(PGTB)?;
    if chunk.version != 3 {
        return Err(Error::Unsupported(format!("PGTB chunk version {}", chunk.version)));
    }
    let mut c = chunk.cursor();
    let (count, mut item) = c.array()?;
    let mut layers = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        layers.push(Layer {
            raw_size: [item.u32()?, item.u32()?],
            page_size: [item.u32()?, item.u32()?],
            raw_format: item.u32()?,
            format: item.u32()?,
            disk_format: item.u32()?,
        });
    }
    let mut pages = Vec::new();
    for raw in [true, false] {
        let (count, mut item) = c.array()?;
        pages.reserve(count.min(1 << 20));
        for _ in 0..count {
            let layer = item.u32()?;
            let x = item.u32()?;
            let y = item.u32()?;
            let file = item.file_ref()?;
            let flags = item.u32()?;
            let [b, g, r, a]: [u8; 4] = item.take(4)?.try_into().unwrap();
            pages.push(Page { layer, x, y, file, flags, color: [r, g, b, a], raw });
        }
    }
    Ok(PagedImage { layers, pages, flags: c.u32()? })
}

/// Scales an image to `width`×`height` pixels.
pub fn resize(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    if image.width == width && image.height == height {
        return image.clone();
    }
    let source = image::RgbaImage::from_raw(image.width, image.height, image.pixels.clone())
        .expect("pixel buffer matches the size");
    let scaled = image::imageops::resize(&source, width, height, image::imageops::FilterType::Triangle);
    RgbaImage { width, height, pixels: scaled.into_raw(), format: image.format.clone() }
}

/// Assembles one layer with `page_pixels` pixels per page edge, cropped to the
/// layer's pages. `load` returns the texture of a page file; pages run in
/// parallel. Grid cells without a page stay transparent.
pub fn compose(
    image: &PagedImage,
    layer: u32,
    page_pixels: u32,
    load: impl Fn(u32) -> Option<RgbaImage> + Sync,
) -> Option<RgbaImage> {
    let [x0, y0, x1, y1] = bounds(image, layer)?;
    let (columns, rows) = (x1 - x0 + 1, y1 - y0 + 1);
    let n = page_pixels.max(1);
    let (width, height) = (columns * n, rows * n);
    let tiles: Vec<(u32, u32, Vec<u8>)> = image
        .pages
        .par_iter()
        .filter(|p| p.layer == layer)
        .filter_map(|p| {
            let pixels = match p.file {
                Some(id) => resize(&load(id)?, n, n).pixels,
                None => p.color.repeat((n * n) as usize),
            };
            Some((p.x - x0, p.y - y0, pixels))
        })
        .collect();
    let mut out = vec![0u8; width as usize * height as usize * 4];
    let row_bytes = n as usize * 4;
    for (x, y, pixels) in tiles {
        for line in 0..n as usize {
            let at = ((y * n) as usize + line) * width as usize * 4 + (x * n) as usize * 4;
            out[at..at + row_bytes].copy_from_slice(&pixels[line * row_bytes..(line + 1) * row_bytes]);
        }
    }
    Some(RgbaImage { width, height, pixels: out, format: "paged image".into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> PagedImage {
        let page = |x, y, file, color| Page { layer: 0, x, y, file, flags: 0, color, raw: false };
        PagedImage {
            layers: vec![],
            pages: vec![page(3, 5, Some(1), [0; 4]), page(4, 5, None, [10, 20, 30, 255]), page(3, 6, Some(2), [0; 4])],
            flags: 0,
        }
    }

    #[test]
    fn composes_pages_into_their_grid_cells() {
        let image = image();
        assert_eq!(bounds(&image, 0), Some([3, 5, 4, 6]));
        let tile = |value: u8| RgbaImage { width: 4, height: 4, pixels: vec![value; 64], format: String::new() };
        let out = compose(&image, 0, 2, |id| Some(tile(id as u8 * 100))).unwrap();
        assert_eq!((out.width, out.height), (4, 4));
        let px = |x: usize, y: usize| &out.pixels[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(px(0, 0), [100; 4]);
        assert_eq!(px(3, 1), [10, 20, 30, 255]);
        assert_eq!(px(1, 3), [200; 4]);
        // No page at (4, 6): transparent.
        assert_eq!(px(3, 3), [0; 4]);
    }
}
