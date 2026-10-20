//! Renders models (shaded, z-buffered, 3/4 view) to PNG: render <Gw2.dat> <outdir> <id>...
use gw2dat::formats::{image, image::RgbaImage, model};

fn main() -> gw2dat::Result<()> {
    let mut args = std::env::args().skip(1);
    let archive = gw2dat::Archive::open(args.next().unwrap())?;
    let out = std::path::PathBuf::from(args.next().unwrap());
    std::fs::create_dir_all(&out)?;
    for id in args.map(|a| a.parse::<u32>().unwrap()) {
        let mft = archive.entry_for_id(id).expect("id");
        let raw = archive.read(mft)?;
        let m = match model::decode(&raw) {
            Ok(m) => m,
            Err(e) => {
                println!("{id}: {e}");
                continue;
            }
        };
        let mats = model::materials(&raw).unwrap_or_default();
        let diffuse: Vec<Option<RgbaImage>> = m
            .meshes
            .iter()
            .map(|mesh| {
                let tex = mats.get(mesh.material_index as usize)?.texture(model::TextureSlot::Diffuse)?;
                image::decode(&archive.read(archive.entry_for_id(tex)?).ok()?).ok()
            })
            .collect();
        let (lo, hi) = m.bounds().unwrap_or(([0.0; 3], [1.0; 3]));
        let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let size = (0..3).map(|i| hi[i] - lo[i]).fold(0.0f32, f32::max).max(1e-6);
        let (w, h) = (400usize, 400usize);
        let mut img =
            RgbaImage { width: w as u32, height: h as u32, pixels: vec![30; w * h * 4], format: String::new() };
        let mut depth = vec![f32::INFINITY; w * h];
        // Rotate 35° around Y, tilt 20° for a 3/4 view.
        let (ay, ax) = (35f32.to_radians(), 20f32.to_radians());
        let project = |p: [f32; 3]| {
            let (x, y, z) = (p[0] - center[0], p[1] - center[1], p[2] - center[2]);
            let (x, z) = (x * ay.cos() + z * ay.sin(), -x * ay.sin() + z * ay.cos());
            let (y, z) = (y * ax.cos() - z * ax.sin(), y * ax.sin() + z * ax.cos());
            let s = 0.85 * w as f32 / size;
            [w as f32 / 2.0 + x * s, h as f32 / 2.0 - y * s, z]
        };
        for (mi, mesh) in m.meshes.iter().enumerate() {
            let tex = diffuse[mi].as_ref().filter(|_| !mesh.uvs.is_empty());
            let pts: Vec<[f32; 3]> = mesh.positions.iter().map(|&p| project(p)).collect();
            for t in mesh.indices.as_chunks::<3>().0 {
                let (a, b, c) = (pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]);
                let n = {
                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
                    [n[0] / l, n[1] / l, n[2] / l]
                };
                let shade = (0.25 + 0.75 * n[2].abs()) * 255.0;
                let (minx, maxx) =
                    (a[0].min(b[0]).min(c[0]).max(0.0) as usize, (a[0].max(b[0]).max(c[0]).ceil() as usize).min(w - 1));
                let (miny, maxy) =
                    (a[1].min(b[1]).min(c[1]).max(0.0) as usize, (a[1].max(b[1]).max(c[1]).ceil() as usize).min(h - 1));
                let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                if area.abs() < 1e-9 {
                    continue;
                }
                for y in miny..=maxy {
                    for x in minx..=maxx {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
                        let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                        let i = y * w + x;
                        if z < depth[i] {
                            let color = match tex {
                                Some(tex) => {
                                    let (ua, ub, uc) =
                                        (mesh.uvs[t[0] as usize], mesh.uvs[t[1] as usize], mesh.uvs[t[2] as usize]);
                                    let u = w0 * ua[0] + w1 * ub[0] + w2 * uc[0];
                                    let v = w0 * ua[1] + w1 * ub[1] + w2 * uc[1];
                                    let tx = ((u.rem_euclid(1.0)) * tex.width as f32) as usize % tex.width as usize;
                                    let ty = ((v.rem_euclid(1.0)) * tex.height as f32) as usize % tex.height as usize;
                                    let p = (ty * tex.width as usize + tx) * 4;
                                    if tex.pixels[p + 3] < 80 {
                                        continue;
                                    }
                                    let k = shade / 255.0;
                                    [
                                        (tex.pixels[p] as f32 * k) as u8,
                                        (tex.pixels[p + 1] as f32 * k) as u8,
                                        (tex.pixels[p + 2] as f32 * k) as u8,
                                        255,
                                    ]
                                }
                                None => [(shade * 0.9) as u8, (shade * 0.75) as u8, (shade * 0.6) as u8, 255],
                            };
                            depth[i] = z;
                            img.pixels[i * 4..i * 4 + 4].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
        std::fs::write(out.join(format!("{id}.png")), img.to_png()?)?;
        println!("{id}: {} meshes, {} verts, {} tris", m.meshes.len(), m.vertex_count(), m.triangle_count());
    }
    Ok(())
}
