//! Renders the terrain grid to an RGB background image (PNG bytes).
//!
//! Colours are worked out once per grid node (biome colour, elevation and
//! hillshade for land; depth and ice for water), then every pixel blends the
//! four surrounding nodes (bilinear interpolation). The land/water decision
//! is made per pixel from the interpolated height, so the shoreline stays
//! crisp even though the grid is coarse.

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbImage};

use super::climate::{Palette, Rgb, SEA_ICE_BELOW, biome_color, mix, palette};
use super::fields::TerrainGrid;
use crate::error::{AppError, AppResult};
use crate::models::generator::{Biome, Climate};

/// Backgrounds larger than this many pixels are rendered at a lower
/// resolution (the map view stretches the image to the map size anyway).
/// 16 MP covers 4096 × 4096, so a 4000 × 3000 map is rendered 1:1.
pub const MAX_BACKGROUND_PIXELS: u64 = 16_777_216;

/// Size of the background image for a map: the map size, scaled down to fit
/// `MAX_BACKGROUND_PIXELS`.
pub fn background_size(width: u32, height: u32) -> (u32, u32) {
    let pixels = u64::from(width) * u64::from(height);
    if pixels <= MAX_BACKGROUND_PIXELS {
        return (width, height);
    }
    let k = (MAX_BACKGROUND_PIXELS as f64 / pixels as f64).sqrt();
    let w = ((f64::from(width) * k).floor() as u32).max(1);
    let h = ((f64::from(height) * k).floor() as u32).max(1);
    (w, h)
}

/// Floating-point colour for blending.
type Col = [f64; 3];

fn col(c: Rgb) -> Col {
    [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]
}

/// Per-node colours: what the node looks like as land, and as water, plus
/// the hillshade factor (kept separate so blurring the colours does not
/// blur the relief).
struct NodeColours {
    land: Vec<Col>,
    water: Vec<Col>,
    shade: Vec<f64>,
}

fn node_colours(grid: &TerrainGrid, climate: Climate, pal: &Palette) -> NodeColours {
    let (cols, rows) = (grid.cols(), grid.rows());
    let n = grid.layout.len();
    let mut land = vec![[0.0; 3]; n];
    let mut water = vec![[0.0; 3]; n];
    let mut shade = vec![1.0; n];
    // Hillshade exaggeration: elevation is 0..1 over the whole map, so the
    // per-node slope is tiny; scale it so that mountains cast visible shade.
    let z = (cols.max(rows) as f64) / 12.0;
    let elev = |c: usize, r: usize| grid.elevation(grid.index(c, r));
    // Depth relative to the 30% deepest point, so the pushed-down map border
    // does not show as a dark frame: most open sea reaches full depth colour.
    let deep = {
        let mut water: Vec<f64> = grid
            .height
            .iter()
            .copied()
            .filter(|h| *h <= grid.sea_level)
            .collect();
        water.sort_by(f64::total_cmp);
        water
            .get(water.len() * 3 / 10)
            .copied()
            .unwrap_or(grid.min_height)
    };
    let depth_range = (grid.sea_level - deep).max(1e-9);
    for r in 0..rows {
        for c in 0..cols {
            let i = grid.index(c, r);
            let biome = grid.biome[i];
            if grid.is_land(i) {
                let e = grid.elevation(i);
                let mut base = col(biome_color(climate, biome));
                // Higher ground is a little lighter and browner.
                let high = col(mix(biome_color(climate, biome), [170, 150, 120], 0.5));
                for k in 0..3 {
                    base[k] = base[k] * (1.0 - e * 0.35) + high[k] * e * 0.35;
                }
                let (cl, cr) = (c.saturating_sub(1), (c + 1).min(cols - 1));
                let (ru, rd) = (r.saturating_sub(1), (r + 1).min(rows - 1));
                let dx = (elev(cr, r) - elev(cl, r)) * z;
                let dy = (elev(c, rd) - elev(c, ru)) * z;
                // Light from the north-west: slopes facing it brighten.
                shade[i] = (1.0 + (-dx - dy) * 0.5).clamp(0.6, 1.35);
                land[i] = base;
                water[i] = col(pal.shallow_water);
            } else {
                land[i] = col(pal.beach);
                // Sea ice is decided per pixel from the temperature (see
                // `render_image`), so here ice nodes get their water colour.
                water[i] = match biome {
                    Biome::Lake => col(pal.lake),
                    _ => {
                        let d = ((grid.sea_level - grid.height[i]) / depth_range)
                            .clamp(0.0, 1.0)
                            .powf(0.6);
                        col(mix(pal.shallow_water, pal.deep_water, d))
                    }
                };
            }
        }
    }
    blur_land(grid, &mut land);
    // Soften the relief a little too (one pass), reusing the colour blur.
    let mut shade3: Vec<Col> = shade.iter().map(|s| [*s; 3]).collect();
    blur_land(grid, &mut shade3);
    let shade = shade3.iter().map(|s| s[0]).collect();
    NodeColours { land, water, shade }
}

/// One 3 × 3 box-blur pass over the land colours (land nodes only), so
/// biome borders become soft gradients instead of grid-aligned steps.
fn blur_land(grid: &TerrainGrid, land: &mut [Col]) {
    let (cols, rows) = (grid.cols(), grid.rows());
    let source = land.to_vec();
    for r in 0..rows {
        for c in 0..cols {
            let i = grid.index(c, r);
            if !grid.is_land(i) {
                continue;
            }
            let (mut sum, mut count) = ([0.0; 3], 0.0);
            for nr in r.saturating_sub(1)..(r + 2).min(rows) {
                for nc in c.saturating_sub(1)..(c + 2).min(cols) {
                    let j = grid.index(nc, nr);
                    if grid.is_land(j) {
                        for k in 0..3 {
                            sum[k] += source[j][k];
                        }
                        count += 1.0;
                    }
                }
            }
            land[i] = [sum[0] / count, sum[1] / count, sum[2] / count];
        }
    }
}

/// Renders the grid into an `out_w × out_h` image.
pub fn render_image(grid: &TerrainGrid, climate: Climate, out_w: u32, out_h: u32) -> RgbImage {
    let pal = palette(climate);
    let colours = node_colours(grid, climate, &pal);
    let (cols, rows) = (grid.cols(), grid.rows());
    let cell = grid.layout.cell;
    // Map pixels per output pixel.
    let sx = grid.layout.width / f64::from(out_w);
    let sy = grid.layout.height / f64::from(out_h);
    let sea = grid.sea_level;
    let land_range = (grid.max_height - sea).max(1e-9);
    let beach = col(pal.beach);
    let line = col(pal.coast_line);
    let ice = col(pal.ice);
    let mut img = RgbImage::new(out_w, out_h);

    for py in 0..out_h {
        let gy = ((f64::from(py) + 0.5) * sy / cell).clamp(0.0, (rows - 1) as f64);
        let r0 = (gy.floor() as usize).min(rows.saturating_sub(2));
        let fy = gy - r0 as f64;
        for px in 0..out_w {
            let gx = ((f64::from(px) + 0.5) * sx / cell).clamp(0.0, (cols - 1) as f64);
            let c0 = (gx.floor() as usize).min(cols.saturating_sub(2));
            let fx = gx - c0 as f64;
            let i00 = r0 * cols + c0;
            let (i10, i01, i11) = (i00 + 1, i00 + cols, i00 + cols + 1);
            let w00 = (1.0 - fx) * (1.0 - fy);
            let w10 = fx * (1.0 - fy);
            let w01 = (1.0 - fx) * fy;
            let w11 = fx * fy;
            let hs = &grid.height;
            let h = hs[i00] * w00 + hs[i10] * w10 + hs[i01] * w01 + hs[i11] * w11;
            // Height gradient (per map px) to find how far we are from the coast.
            let gxh = ((hs[i10] - hs[i00]) * (1.0 - fy) + (hs[i11] - hs[i01]) * fy) / cell;
            let gyh = ((hs[i01] - hs[i00]) * (1.0 - fx) + (hs[i11] - hs[i10]) * fx) / cell;
            let slope = (gxh * gxh + gyh * gyh).sqrt().max(1e-12);
            // Distance to the shoreline in output pixels.
            let shore = (h - sea).abs() / slope / sx;

            // Only squares with both land and water corners have a coast.
            let corners = [hs[i00], hs[i10], hs[i01], hs[i11]];
            let coastal = corners.iter().any(|v| *v > sea) && corners.iter().any(|v| *v <= sea);
            let set = if h > sea {
                &colours.land
            } else {
                &colours.water
            };
            let mut c = [0.0; 3];
            for (k, v) in c.iter_mut().enumerate() {
                *v = set[i00][k] * w00 + set[i10][k] * w10 + set[i01][k] * w01 + set[i11][k] * w11;
            }
            if h > sea {
                let sh = &colours.shade;
                let s = sh[i00] * w00 + sh[i10] * w10 + sh[i01] * w01 + sh[i11] * w11;
                for v in &mut c {
                    *v *= s;
                }
            } else {
                // Sea ice where the (interpolated) water temperature is
                // below freezing, with a short soft edge.
                let ts = &grid.temperature;
                let t = ts[i00] * w00 + ts[i10] * w10 + ts[i01] * w01 + ts[i11] * w11;
                let frozen = ((SEA_ICE_BELOW - t) / 0.03 + 0.5).clamp(0.0, 1.0);
                if frozen > 0.0 {
                    for k in 0..3 {
                        c[k] = c[k] * (1.0 - frozen) + ice[k] * frozen;
                    }
                }
            }
            if h > sea {
                // A strip of beach just above sea level.
                let e = (h - sea) / land_range;
                let t = (1.0 - e / 0.02).clamp(0.0, 1.0) * 0.8;
                for k in 0..3 {
                    c[k] = c[k] * (1.0 - t) + beach[k] * t;
                }
            } else if coastal && shore < 6.0 {
                // Lighter surf near the shore.
                let t = (1.0 - shore / 6.0) * 0.25;
                for v in &mut c {
                    *v += (255.0 - *v) * t;
                }
            }
            if coastal && shore < 1.2 {
                let t = (1.0 - shore / 1.2) * 0.85;
                for k in 0..3 {
                    c[k] = c[k] * (1.0 - t) + line[k] * t;
                }
            }
            let rgb = [
                c[0].clamp(0.0, 255.0) as u8,
                c[1].clamp(0.0, 255.0) as u8,
                c[2].clamp(0.0, 255.0) as u8,
            ];
            img.put_pixel(px, py, image::Rgb(rgb));
        }
    }
    img
}

/// Encodes an image as PNG bytes.
pub fn encode_png(img: &RgbImage) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| AppError::Internal(format!("could not encode PNG: {e}")))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_is_scaled_only_when_huge() {
        assert_eq!(background_size(4000, 3000), (4000, 3000));
        let (w, h) = background_size(16384, 16384);
        assert!(u64::from(w) * u64::from(h) <= MAX_BACKGROUND_PIXELS);
        assert_eq!(w, h);
    }
}
