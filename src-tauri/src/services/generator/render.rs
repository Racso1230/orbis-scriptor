//! Renders the terrain grid to an RGB background image (PNG bytes).
//!
//! The terrain grid is coarse (a node every ~8 map pixels), so the renderer
//! works in two steps:
//!
//! 1. Per grid node it works out a land colour (biome + elevation), a water
//!    colour (depth) and the smooth slope of the terrain.
//! 2. Per pixel it interpolates those, then adds fine detail that the grid
//!    cannot hold: a small deterministic value noise on the height (a wiggly,
//!    hand-drawn coastline that never moves more than ~3 px from the traced
//!    vector coast), a per-pixel hillshade lit from the top-left, warped and
//!    dithered biome borders, a dark coast line with a sand/foam band, a
//!    shallow-water glow and a faint paper grain.
//!
//! Every pixel only reads shared, immutable data, so rows are rendered in
//! parallel bands; the result does not depend on the number of threads.

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

/// Floating-point colour for blending. `f32` is plenty for 8-bit output and
/// keeps the per-pixel loop fast.
type Col = [f32; 3];

fn col(c: Rgb) -> Col {
    [f32::from(c[0]), f32::from(c[1]), f32::from(c[2])]
}

fn lerp_col(a: Col, b: Col, t: f32) -> Col {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// `0` below `lo`, `1` above `hi`, smooth in between.
fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// Noise
// ---------------------------------------------------------------------------

/// Integer hash of a lattice point: the same inputs always give the same
/// bits, which is what makes the noise (and so the PNG) deterministic.
fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    // `as u32` reinterprets the bits of a negative `i32`; `wrapping_mul`
    // lets the multiplication overflow on purpose instead of panicking.
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    h
}

/// A lattice value in `-1..1`.
fn lattice(x: i32, y: i32, seed: u32) -> f32 {
    (hash2(x, y, seed) >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
}

/// Value noise at `(x, y)` (lattice units) with its partial derivatives:
/// `(value, d/dx, d/dy)`. The quintic fade makes the derivative continuous,
/// so the hillshade computed from it has no lattice seams.
fn value_noise_d(x: f32, y: f32, seed: u32) -> (f32, f32, f32) {
    let (xf, yf) = (x.floor(), y.floor());
    let (ix, iy) = (xf as i32, yf as i32);
    let (tx, ty) = (x - xf, y - yf);
    let a = lattice(ix, iy, seed);
    let b = lattice(ix + 1, iy, seed);
    let c = lattice(ix, iy + 1, seed);
    let d = lattice(ix + 1, iy + 1, seed);
    let u = tx * tx * tx * (tx * (tx * 6.0 - 15.0) + 10.0);
    let v = ty * ty * ty * (ty * (ty * 6.0 - 15.0) + 10.0);
    let du = 30.0 * tx * tx * (tx - 1.0) * (tx - 1.0);
    let dv = 30.0 * ty * ty * (ty - 1.0) * (ty - 1.0);
    let k = a - b - c + d;
    let value = a + (b - a) * u + (c - a) * v + k * u * v;
    (value, du * ((b - a) + k * v), dv * ((c - a) + k * u))
}

/// Value noise without derivatives (smoothstep fade; cheaper).
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (ix, iy) = (xf as i32, yf as i32);
    let (tx, ty) = (x - xf, y - yf);
    let u = tx * tx * (3.0 - 2.0 * tx);
    let v = ty * ty * (3.0 - 2.0 * ty);
    let a = lattice(ix, iy, seed);
    let b = lattice(ix + 1, iy, seed);
    let c = lattice(ix, iy + 1, seed);
    let d = lattice(ix + 1, iy + 1, seed);
    let top = a + (b - a) * u;
    let bottom = c + (d - c) * u;
    top + (bottom - top) * v
}

/// Height-detail octaves: (wavelength in map px, amplitude in elevation
/// units, where 1.0 is the full sea-level-to-summit range).
const DETAIL_OCTAVES: [(f32, f32); 4] =
    [(46.0, 0.010), (19.0, 0.006), (8.0, 0.0035), (3.5, 0.0012)];

/// Rotation of each detail octave, in radians.
const OCTAVE_ANGLES: [f32; 4] = [0.52, 1.71, 2.93, 4.07];

/// The noise seed. `render_image` only receives the grid, so the seed is a
/// hash of the terrain heights (which are fixed by `params.seed` and the
/// land settings, but not by the climate).
fn grid_seed(grid: &TerrainGrid) -> u32 {
    // FNV-1a over a sample of the heights.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    // A closure that captures `h` mutably (`FnMut`): it may change `h`.
    let mut eat = |bits: u64| {
        h ^= bits;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    eat(grid.cols() as u64);
    eat(grid.rows() as u64);
    eat(grid.sea_level.to_bits());
    let step = (grid.height.len() / 61).max(1);
    for v in grid.height.iter().step_by(step) {
        eat(v.to_bits());
    }
    (h ^ (h >> 32)) as u32
}

// ---------------------------------------------------------------------------
// Per-node data
// ---------------------------------------------------------------------------

/// Per-node values that every pixel interpolates.
struct Nodes {
    /// What the node looks like as land. Water nodes next to land take the
    /// average of their land neighbours, so warped biome borders near the
    /// coast do not pull in an unrelated colour.
    land: Vec<Col>,
    /// What the node looks like as water (depth colour, lake colour).
    water: Vec<Col>,
    height: Vec<f32>,
    temperature: Vec<f32>,
    /// Smooth gradient per map pixel: `[elev_x, elev_y, height_x, height_y]`.
    /// Elevation (0 at sea level, clamped) drives the hillshade; the raw
    /// height gives the distance to the coast.
    grad: Vec<[f32; 4]>,
}

fn node_data(grid: &TerrainGrid, climate: Climate, pal: &Palette) -> Nodes {
    let (cols, rows) = (grid.cols(), grid.rows());
    let n = grid.layout.len();
    let cell = grid.layout.cell;
    let mut land = vec![[0.0; 3]; n];
    let mut water = vec![[0.0; 3]; n];
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
    for i in 0..n {
        let biome = grid.biome[i];
        if grid.is_land(i) {
            let e = grid.elevation(i);
            let base = biome_color(climate, biome);
            // Higher ground is a little lighter and browner.
            let high = mix(base, [170, 150, 120], 0.5);
            land[i] = col(mix(base, high, e * 0.35));
            water[i] = col(pal.shallow_water);
        } else {
            land[i] = col(pal.beach);
            // Sea ice is decided per pixel from the temperature, so ice
            // nodes get their water colour here.
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
    blur_land(grid, &mut land);

    let mut grad = vec![[0.0; 4]; n];
    for r in 0..rows {
        for c in 0..cols {
            let (cl, cr) = (c.saturating_sub(1), (c + 1).min(cols - 1));
            let (ru, rd) = (r.saturating_sub(1), (r + 1).min(rows - 1));
            // Central differences, divided by the distance actually spanned
            // (one cell at the edges, two inside).
            let spanx = (cr - cl).max(1) as f64 * cell;
            let spany = (rd - ru).max(1) as f64 * cell;
            let (il, ir) = (grid.index(cl, r), grid.index(cr, r));
            let (iu, id) = (grid.index(c, ru), grid.index(c, rd));
            grad[grid.index(c, r)] = [
                ((grid.elevation(ir) - grid.elevation(il)) / spanx) as f32,
                ((grid.elevation(id) - grid.elevation(iu)) / spany) as f32,
                ((grid.height[ir] - grid.height[il]) / spanx) as f32,
                ((grid.height[id] - grid.height[iu]) / spany) as f32,
            ];
        }
    }
    Nodes {
        land,
        water,
        height: grid.height.iter().map(|v| *v as f32).collect(),
        temperature: grid.temperature.iter().map(|v| *v as f32).collect(),
        grad,
    }
}

/// One 3 × 3 box-blur pass over the land colours (half strength), so single-node biome
/// specks soften. Water nodes next to land take the average of their land
/// neighbours; water nodes far from land keep the beach colour.
fn blur_land(grid: &TerrainGrid, land: &mut [Col]) {
    let (cols, rows) = (grid.cols(), grid.rows());
    let source = land.to_vec();
    for r in 0..rows {
        for c in 0..cols {
            let i = grid.index(c, r);
            let (mut sum, mut count) = ([0.0f32; 3], 0.0f32);
            for nr in r.saturating_sub(1)..(r + 2).min(rows) {
                for nc in c.saturating_sub(1)..(c + 2).min(cols) {
                    let j = grid.index(nc, nr);
                    if grid.is_land(j) {
                        for (s, v) in sum.iter_mut().zip(source[j]) {
                            *s += v;
                        }
                        count += 1.0;
                    }
                }
            }
            if count == 0.0 {
                continue;
            }
            let avg = [sum[0] / count, sum[1] / count, sum[2] / count];
            // Land nodes keep half their own colour, so biome patches stay
            // distinct (the per-pixel warp and dither soften their edges).
            land[i] = if grid.is_land(i) {
                lerp_col(source[i], avg, 0.5)
            } else {
                avg
            };
        }
    }
}

// ---------------------------------------------------------------------------
// Per-pixel rendering
// ---------------------------------------------------------------------------

/// Everything a pixel needs, shared (read-only) by all render threads.
struct Renderer {
    nodes: Nodes,
    cols: usize,
    rows: usize,
    cell: f32,
    /// Map pixels per output pixel.
    sx: f32,
    sy: f32,
    sea: f32,
    land_range: f32,
    seed: u32,
    /// Detail octaves with their weight for this output scale (octaves
    /// finer than ~3 output pixels fade out, e.g. in the preview).
    octaves: [(f32, f32); 4],
    /// `(sin, cos)` of each octave's rotation.
    rotations: [(f32, f32); 4],
    /// Largest possible |detail| in elevation units.
    detail_amp: f32,
    /// Hillshade exaggeration of the smooth terrain slope.
    relief: f32,
    beach: Col,
    line: Col,
    ice: Col,
    foam: Col,
    glow: Col,
}

/// One point of the detailed terrain (see `Renderer::sample`).
struct Sample {
    cell: Cell,
    /// Height with the fine detail.
    height: f32,
    /// Smooth gradient, as in `Nodes::grad`.
    grad: [f32; 4],
    /// Smooth slope of the raw height per map pixel.
    slope: f32,
    /// Detail noise value and gradient (elevation units).
    noise: (f32, f32, f32),
}

/// Bilinear corner (top-left index) and weights of a grid position.
struct Cell {
    i00: usize,
    w: [f32; 4],
}

fn weights(fx: f32, fy: f32) -> [f32; 4] {
    [
        (1.0 - fx) * (1.0 - fy),
        fx * (1.0 - fy),
        (1.0 - fx) * fy,
        fx * fy,
    ]
}

impl Renderer {
    fn new(grid: &TerrainGrid, climate: Climate, out_w: u32, out_h: u32) -> Renderer {
        let pal = palette(climate);
        let nodes = node_data(grid, climate, &pal);
        let sx = (grid.layout.width / f64::from(out_w)) as f32;
        let sy = (grid.layout.height / f64::from(out_h)) as f32;
        let scale = sx.max(sy);
        let mut octaves = DETAIL_OCTAVES;
        let mut detail_amp = 0.0;
        for (wavelength, amp) in &mut octaves {
            *amp *= ((*wavelength / scale - 1.5) / 1.5).clamp(0.0, 1.0);
            detail_amp += *amp;
        }
        let shallow = col(pal.shallow_water);
        Renderer {
            cols: grid.cols(),
            rows: grid.rows(),
            cell: grid.layout.cell as f32,
            sx,
            sy,
            sea: grid.sea_level as f32,
            land_range: (grid.max_height - grid.sea_level).max(1e-9) as f32,
            seed: grid_seed(grid),
            octaves,
            rotations: OCTAVE_ANGLES.map(f32::sin_cos),
            detail_amp: detail_amp.max(1e-9),
            // Elevation spans 0..1 over the whole map, so the slope per map
            // pixel is tiny; scale it with the map size so relief looks the
            // same on a small and a large map.
            relief: (grid.layout.width.max(grid.layout.height) / 9.0) as f32,
            beach: col(pal.beach),
            line: col(pal.coast_line),
            ice: col(pal.ice),
            foam: lerp_col(shallow, [255.0; 3], 0.45),
            glow: lerp_col(shallow, [255.0; 3], 0.12),
            nodes,
        }
    }

    /// Bilinear corner and weights for grid coordinates `(gx, gy)`, plus
    /// the fractional position inside the cell.
    fn cell_at(&self, gx: f32, gy: f32) -> (Cell, f32, f32) {
        let gx = gx.clamp(0.0, (self.cols - 1) as f32);
        let gy = gy.clamp(0.0, (self.rows - 1) as f32);
        let c0 = (gx as usize).min(self.cols.saturating_sub(2));
        let r0 = (gy as usize).min(self.rows.saturating_sub(2));
        let (fx, fy) = (gx - c0 as f32, gy - r0 as f32);
        let cell = Cell {
            i00: r0 * self.cols + c0,
            w: weights(fx, fy),
        };
        (cell, fx, fy)
    }

    fn corners(&self, c: &Cell) -> [usize; 4] {
        let i = c.i00;
        [i, i + 1, i + self.cols, i + self.cols + 1]
    }

    fn bil(&self, c: &Cell, v: &[f32]) -> f32 {
        let k = self.corners(c);
        v[k[0]] * c.w[0] + v[k[1]] * c.w[1] + v[k[2]] * c.w[2] + v[k[3]] * c.w[3]
    }

    /// Bilinear blend of any per-node array of `N` floats.
    fn bil_n<const N: usize>(&self, c: &Cell, v: &[[f32; N]]) -> [f32; N] {
        // `const N: usize` is a const generic: the function works for
        // arrays of any fixed length, chosen at compile time.
        let k = self.corners(c);
        let mut out = [0.0; N];
        for (ch, o) in out.iter_mut().enumerate() {
            *o = v[k[0]][ch] * c.w[0]
                + v[k[1]][ch] * c.w[1]
                + v[k[2]][ch] * c.w[2]
                + v[k[3]][ch] * c.w[3];
        }
        out
    }

    /// Sum of the detail octaves at a map position, in elevation units,
    /// with its gradient per map pixel.
    fn detail(&self, mx: f32, my: f32) -> (f32, f32, f32) {
        let (mut v, mut dx, mut dy) = (0.0, 0.0, 0.0);
        for (o, &(wavelength, amp)) in self.octaves.iter().enumerate() {
            if amp <= 0.0 {
                continue;
            }
            let f = 1.0 / wavelength;
            let seed = self.seed.wrapping_add(o as u32 * 7919);
            // Each octave is rotated, so the lattice of value noise does not
            // line up into visible horizontal/vertical streaks.
            let (sin, cos) = self.rotations[o];
            let (u, w) = (cos * mx - sin * my, sin * mx + cos * my);
            let (n, nu, nw) = value_noise_d(u * f, w * f, seed);
            v += n * amp;
            // Rotate the derivative back into map x/y.
            dx += (nu * cos + nw * sin) * amp * f;
            dy += (nw * cos - nu * sin) * amp * f;
        }
        (v, dx, dy)
    }

    /// Land colour with warped, dithered biome borders: the node colours are
    /// sampled at a slightly displaced position and blended with sharpened
    /// weights, so borders are crisp but not grid-aligned.
    fn land_colour(&self, mx: f32, my: f32, dither: f32) -> Col {
        let f = 1.0 / (self.cell * 2.7);
        let f2 = f * 3.1;
        let s = self.seed ^ 0x5bd1_e995;
        let wx = value_noise(mx * f, my * f, s) * 0.6 + value_noise(mx * f2, my * f2, s ^ 1) * 0.2;
        let wy =
            value_noise(mx * f, my * f, s ^ 2) * 0.6 + value_noise(mx * f2, my * f2, s ^ 3) * 0.2;
        let (mut c, fx, fy) = self.cell_at(mx / self.cell + wx, my / self.cell + wy);
        let fx = smoothstep(0.15, 0.85, fx + dither);
        let fy = smoothstep(0.15, 0.85, fy - dither);
        c.w = weights(fx, fy);
        self.bil_n(&c, &self.nodes.land)
    }

    /// The height at a map position with the fine detail added. Near the
    /// coast the detail is capped by the local slope, so the shoreline moves
    /// at most ~3 map px from the plain (bilinear) one that the vector coast
    /// follows.
    fn sample(&self, mx: f32, my: f32) -> Sample {
        let (cell, fx, fy) = self.cell_at(mx / self.cell, my / self.cell);
        let nodes = &self.nodes;
        let h = self.bil(&cell, &nodes.height);
        let g = self.bil_n(&cell, &nodes.grad);
        let slope = (g[2] * g[2] + g[3] * g[3]).sqrt().max(1e-9);
        // The exact slope of the bilinear surface in this cell: on saddles
        // and flats it can be much smaller than the smooth slope, and the
        // cap must use the smaller one.
        let k = self.corners(&cell);
        let hs = &nodes.height;
        let bx = ((hs[k[1]] - hs[k[0]]) * (1.0 - fy) + (hs[k[3]] - hs[k[2]]) * fy) / self.cell;
        let by = ((hs[k[2]] - hs[k[0]]) * (1.0 - fx) + (hs[k[3]] - hs[k[1]]) * fx) / self.cell;
        let cap_slope = (bx * bx + by * by).sqrt().min(slope);

        let (n, ndx, ndy) = self.detail(mx, my);
        let shore0 = (h - self.sea).abs() / cap_slope.max(1e-9);
        let limit = cap_slope * (2.0 + 0.3 * shore0);
        let k_coast = (limit / (self.detail_amp * self.land_range)).min(1.0);
        Sample {
            cell,
            height: h + n * self.land_range * k_coast,
            grad: g,
            slope,
            noise: (n, ndx, ndy),
        }
    }

    fn pixel(&self, px: u32, py: u32) -> [u8; 3] {
        let mx = (px as f32 + 0.5) * self.sx;
        let my = (py as f32 + 0.5) * self.sy;
        let Sample {
            cell,
            height: hd,
            grad: g,
            slope,
            noise: (n, ndx, ndy),
            ..
        } = self.sample(mx, my);
        let nodes = &self.nodes;
        let is_land = hd > self.sea;
        // Distance to the shoreline in output pixels.
        let shore = (hd - self.sea).abs() / slope / self.sx;

        // Per-pixel dither for the biome borders.
        let grain = hash2(px as i32, py as i32, self.seed ^ 0x9e37_79b9);
        let grain_b = (grain >> 16) as f32 / 65_535.0 - 0.5;

        let mut c = if is_land {
            let e = ((hd - self.sea) / self.land_range).clamp(0.0, 1.0);
            let mut c = self.land_colour(mx, my, grain_b * 0.3);
            // Hillshade from a per-pixel normal of the detailed surface,
            // lit from the top-left. Relief is stronger on high ground.
            let rough = (0.4 + 1.8 * e).min(1.5);
            let detail_k = 170.0 * rough;
            let hx = g[0] * self.relief + ndx * detail_k;
            let hy = g[1] * self.relief + ndy * detail_k;
            // Normal (-hx, -hy, 1) against the light direction (-1, -1, 1.3),
            // relative to flat ground (which gives 1.0).
            let dot = (hx + hy + 1.3) / ((hx * hx + hy * hy + 1.0).sqrt() * 1.3);
            let strength = 0.55 + 0.7 * e;
            let shade = (1.0 + (dot - 1.0) * strength * 1.6).clamp(0.62, 1.35);
            // Peaks get extra contrast: shadows deepen on high ground.
            let shade = if shade < 1.0 {
                shade - (1.0 - shade) * e * 0.35
            } else {
                shade
            };
            for v in &mut c {
                *v *= shade;
            }
            // A sand band just above the waterline.
            let t = (1.0 - smoothstep(0.9, 2.4, shore)) * 0.85;
            lerp_col(c, self.beach, t)
        } else {
            let mut c = self.bil_n(&cell, &nodes.water);
            // A faint ripple so open water is not a flat smear.
            for v in &mut c {
                *v *= 1.0 + n * 2.5;
            }
            // Sea ice where the water is below freezing, with a ragged edge.
            let t = self.bil(&cell, &nodes.temperature) + n * 0.8;
            let frozen = ((SEA_ICE_BELOW as f32 - t) / 0.02 + 0.5).clamp(0.0, 1.0);
            if frozen > 0.0 {
                c = lerp_col(c, self.ice, frozen);
            }
            let open = 1.0 - frozen;
            // Shallow-water glow fading out from the coast...
            let glow = (1.0 - shore / 22.0).max(0.0);
            c = lerp_col(c, self.glow, glow * glow * 0.6 * open);
            // ...and a thin foam band right next to the coast line.
            let foam = smoothstep(0.6, 1.1, shore) * (1.0 - smoothstep(1.6, 2.6, shore));
            lerp_col(c, self.foam, foam * 0.7 * open)
        };
        // The coast line itself, about 1.5 px wide, antialiased.
        let t = (1.0 - smoothstep(0.35, 0.85, shore)) * 0.9;
        c = lerp_col(c, self.line, t);

        // Paper grain: a soft mottle. (A per-pixel speck looks nice but
        // makes the PNG ~4x bigger, since noise does not compress.)
        let speck = value_noise(mx / 70.0, my / 70.0, self.seed ^ 0xa5a5) * 4.0
            + value_noise(mx / 9.0, my / 9.0, self.seed ^ 0x5a5a) * 1.5;
        [
            (c[0] + speck).clamp(0.0, 255.0) as u8,
            (c[1] + speck).clamp(0.0, 255.0) as u8,
            (c[2] + speck * 0.9).clamp(0.0, 255.0) as u8,
        ]
    }
}

/// Renders the grid into an `out_w × out_h` image.
pub fn render_image(grid: &TerrainGrid, climate: Climate, out_w: u32, out_h: u32) -> RgbImage {
    let mut img = RgbImage::new(out_w, out_h);
    let row_bytes = out_w as usize * 3;
    if row_bytes == 0 || out_h == 0 {
        return img;
    }
    let renderer = Renderer::new(grid, climate, out_w, out_h);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 16);
    // Bands of rows; each thread fills its own slice of the pixel buffer,
    // so the output is the same whichever thread renders which band.
    let band_rows = (out_h as usize).div_ceil(threads * 4).max(1);
    let bands: Vec<(usize, &mut [u8])> = img
        .as_mut()
        .chunks_mut(band_rows * row_bytes)
        .enumerate()
        .collect();
    // A simple work queue: threads take bands until none are left.
    let queue = std::sync::Mutex::new(bands);
    // `thread::scope` lets the threads borrow `renderer` and the image
    // buffer; every thread is joined before the scope returns.
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    // A poisoned lock means another thread panicked; stop.
                    let next = match queue.lock() {
                        Ok(mut q) => q.pop(),
                        Err(_) => None,
                    };
                    let Some((band, buf)) = next else { break };
                    for (row, line) in buf.chunks_mut(row_bytes).enumerate() {
                        let py = (band * band_rows + row) as u32;
                        for (px, out) in line.chunks_mut(3).enumerate() {
                            out.copy_from_slice(&renderer.pixel(px as u32, py));
                        }
                    }
                }
            });
        }
    });
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

    #[test]
    fn noise_is_deterministic_and_bounded() {
        for i in 0..200 {
            let (x, y) = (i as f32 * 0.37 - 20.0, i as f32 * 0.91 - 50.0);
            let (v, _, _) = value_noise_d(x, y, 7);
            assert!((-1.0..=1.0).contains(&v));
            assert_eq!(value_noise_d(x, y, 7), value_noise_d(x, y, 7));
            assert!((-1.0..=1.0).contains(&value_noise(x, y, 3)));
        }
        assert_ne!(hash2(1, 2, 3), hash2(2, 1, 3));
    }

    #[test]
    fn noise_derivative_matches_finite_difference() {
        let (x, y, e) = (3.3f32, 7.6f32, 1e-3f32);
        let (_, dx, dy) = value_noise_d(x, y, 11);
        let fx = (value_noise_d(x + e, y, 11).0 - value_noise_d(x - e, y, 11).0) / (2.0 * e);
        let fy = (value_noise_d(x, y + e, 11).0 - value_noise_d(x, y - e, 11).0) / (2.0 * e);
        assert!((dx - fx).abs() < 1e-2, "{dx} vs {fx}");
        assert!((dy - fy).abs() < 1e-2, "{dy} vs {fy}");
    }

    #[test]
    fn detail_moves_the_shoreline_by_at_most_a_few_pixels() {
        use crate::models::generator::GeneratorParams;
        use crate::services::generator::fields::{GridLayout, build_grid};

        let (w, h) = (400i32, 300i32);
        let params = GeneratorParams {
            seed: 42,
            width: w as u32,
            height: h as u32,
            ..GeneratorParams::default()
        };
        let grid = build_grid(&params, GridLayout::for_map(w as u32, h as u32));
        let r = Renderer::new(&grid, Climate::Temperate, w as u32, h as u32);
        // Land/water per pixel, without and with the detail noise.
        let mut plain = vec![false; (w * h) as usize];
        let mut detail = vec![false; (w * h) as usize];
        for py in 0..h {
            for px in 0..w {
                let s = r.sample(px as f32 + 0.5, py as f32 + 0.5);
                let i = (py * w + px) as usize;
                plain[i] = r.bil(&s.cell, &r.nodes.height) > r.sea;
                detail[i] = s.height > r.sea;
            }
        }
        let mut changed = 0;
        for py in 0..h {
            for px in 0..w {
                let i = (py * w + px) as usize;
                if plain[i] == detail[i] {
                    continue;
                }
                changed += 1;
                // A pixel that flipped must be within 3 px of a plain pixel
                // that already had its new state (i.e. of the old coast).
                let near = (-3..=3).any(|dy: i32| {
                    (-3..=3).any(|dx: i32| {
                        let (x, y) = (px + dx, py + dy);
                        x >= 0
                            && y >= 0
                            && x < w
                            && y < h
                            && plain[(y * w + x) as usize] == detail[i]
                    })
                });
                assert!(near, "pixel ({px}, {py}) moved too far");
            }
        }
        assert!(changed > 0, "the detail should roughen the coastline");
    }
}
