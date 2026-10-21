//! Paged image (PIMG) preview: page layout, a downscaled overview and single
//! pages at the resolution the view needs.

use gw2dat::formats::{image, paged_image};
use gw2dat::fourcc;
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, image_response, session};
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerDto {
    index: u32,
    page_width: u32,
    page_height: u32,
    format: String,
    /// `[min x, min y, max x, max y]` of the layer's pages.
    bounds: Option<[u32; 4]>,
    /// Per page: grid x, grid y, texture file id (0 for a solid page) and
    /// RGBA colour packed as `0xRRGGBBAA`.
    xs: Vec<u32>,
    ys: Vec<u32>,
    files: Vec<u32>,
    colors: Vec<u32>,
}

#[tauri::command]
pub async fn paged_image_info(app: AppHandle, mft: u32) -> CmdResult<Vec<LayerDto>> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let image = paged_image::decode(&session.read(mft).map_err(err)?).map_err(err)?;
        Ok(image
            .layers
            .iter()
            .enumerate()
            .map(|(i, layer)| {
                let pages: Vec<_> = image.pages.iter().filter(|p| p.layer == i as u32).collect();
                LayerDto {
                    index: i as u32,
                    page_width: layer.page_size[0],
                    page_height: layer.page_size[1],
                    format: fourcc::display(layer.format),
                    bounds: paged_image::bounds(&image, i as u32),
                    xs: pages.iter().map(|p| p.x).collect(),
                    ys: pages.iter().map(|p| p.y).collect(),
                    files: pages.iter().map(|p| p.file.unwrap_or(0)).collect(),
                    colors: pages.iter().map(|p| u32::from_be_bytes(p.color)).collect(),
                }
            })
            .collect())
    })
    .await
}

fn load_page(session: &Session, file_id: u32) -> Option<image::RgbaImage> {
    let mft = session.archive.entry_for_id(file_id)?;
    image::decode(&session.archive.read(mft).ok()?).ok()
}

/// Pixels per page so the whole layer fits into `max_size` pixels.
fn page_pixels(image: &paged_image::PagedImage, layer: u32, max_size: u32) -> CmdResult<u32> {
    let [x0, y0, x1, y1] = paged_image::bounds(image, layer).ok_or("The layer has no pages")?;
    let cells = (x1 - x0 + 1).max(y1 - y0 + 1);
    let page = image.layers.get(layer as usize).map_or(512, |l| l.page_size[0].max(1));
    Ok((max_size / cells).clamp(1, page))
}

/// The whole layer, scaled so its longer side is at most `max_size` pixels.
#[tauri::command]
pub async fn paged_image_overview(app: AppHandle, mft: u32, layer: u32, max_size: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let image = paged_image::decode(&session.read(mft).map_err(err)?).map_err(err)?;
        let pixels = page_pixels(&image, layer, max_size)?;
        let composed = paged_image::compose(&image, layer, pixels, |id| load_page(&session, id))
            .ok_or("The layer has no pages")?;
        Ok(image_response(composed))
    })
    .await
}

/// One page texture scaled down to at most `size` pixels per edge.
#[tauri::command]
pub async fn paged_image_tile(app: AppHandle, file_id: u32, size: u32) -> CmdResult<Response> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let page = load_page(&session, file_id).ok_or_else(|| format!("Page texture {file_id} could not be read"))?;
        let scale = (size as f32 / page.width.max(page.height) as f32).min(1.0);
        let (w, h) = (((page.width as f32 * scale) as u32).max(1), ((page.height as f32 * scale) as u32).max(1));
        Ok(image_response(paged_image::resize(&page, w, h)))
    })
    .await
}

/// Writes a layer as PNG with `page_pixels` pixels per page.
#[tauri::command]
pub async fn export_paged_image(app: AppHandle, mft: u32, layer: u32, page_pixels: u32, path: String) -> CmdResult<()> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        let image = paged_image::decode(&session.read(mft).map_err(err)?).map_err(err)?;
        let composed = paged_image::compose(&image, layer, page_pixels, |id| load_page(&session, id))
            .ok_or("The layer has no pages")?;
        std::fs::write(&path, composed.to_png().map_err(err)?).map_err(err)
    })
    .await
}
