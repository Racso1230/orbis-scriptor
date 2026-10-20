//! Feature stamps scattered with Poisson-disk sampling.
//!
//! Poisson-disk sampling (Bridson's algorithm) gives random points that are
//! never closer than a minimum distance, so stamps look hand-placed rather
//! than clumped like plain random points.

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

use super::fields::TerrainGrid;
use super::stamps::is_known_stamp;
use crate::models::generator::{Biome, Climate};

/// A stamp to place.
#[derive(Debug, Clone, PartialEq)]
pub struct StampPlacement {
    pub x: f64,
    pub y: f64,
    pub icon: &'static str,
    pub scale: f64,
    pub rotation: f64,
    pub biome: Biome,
}

/// Upper bound on sample points (and therefore stamps), to keep the map
/// light for Konva.
const MAX_SAMPLES: usize = 8000;

/// Minimum spacing between stamps in map px. Stamps are 64 px at scale 1;
/// very large maps space them further apart (and draw them bigger) so the
/// count stays reasonable.
pub fn spacing(width: f64, height: f64) -> f64 {
    (width * height / 2500.0).sqrt().max(80.0)
}

/// Bridson's algorithm over the whole map.
fn poisson_disk(width: f64, height: f64, r: f64, rng: &mut ChaCha8Rng) -> Vec<(f64, f64)> {
    let cell = r / std::f64::consts::SQRT_2;
    let cols = (width / cell).ceil() as usize + 1;
    let rows = (height / cell).ceil() as usize + 1;
    // Background grid: at most one sample per cell, so checking neighbours
    // only needs the 5×5 block of cells around a candidate.
    let mut slots: Vec<Option<usize>> = vec![None; cols * rows];
    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut active: Vec<usize> = Vec::new();
    let slot_of = |x: f64, y: f64| ((y / cell) as usize) * cols + (x / cell) as usize;

    let first = (rng.random_range(0.0..width), rng.random_range(0.0..height));
    slots[slot_of(first.0, first.1)] = Some(0);
    points.push(first);
    active.push(0);

    while !active.is_empty() && points.len() < MAX_SAMPLES {
        let pick = rng.random_range(0..active.len());
        let (px, py) = points[active[pick]];
        let mut placed = false;
        for _ in 0..20 {
            let angle = rng.random_range(0.0..std::f64::consts::TAU);
            let dist = rng.random_range(r..2.0 * r);
            let (x, y) = (px + angle.cos() * dist, py + angle.sin() * dist);
            if x < 0.0 || y < 0.0 || x >= width || y >= height {
                continue;
            }
            let (gc, gr) = ((x / cell) as usize, (y / cell) as usize);
            let mut ok = true;
            'check: for nr in gr.saturating_sub(2)..(gr + 3).min(rows) {
                for nc in gc.saturating_sub(2)..(gc + 3).min(cols) {
                    if let Some(j) = slots[nr * cols + nc] {
                        let (qx, qy) = points[j];
                        if (qx - x).powi(2) + (qy - y).powi(2) < r * r {
                            ok = false;
                            break 'check;
                        }
                    }
                }
            }
            if ok {
                slots[slot_of(x, y)] = Some(points.len());
                active.push(points.len());
                points.push((x, y));
                placed = true;
                break;
            }
        }
        if !placed {
            active.swap_remove(pick);
        }
    }
    points
}

/// Chance of a stamp at a sample point, and which icon, for a biome.
fn choose_icon(
    biome: Biome,
    climate: Climate,
    near_river: bool,
    elev: f64,
    rng: &mut ChaCha8Rng,
) -> Option<&'static str> {
    // Always draw the two numbers, so one biome's choice cannot shift the
    // random stream for the next point differently from another biome's.
    let roll: f64 = rng.random_range(0.0..1.0);
    let pick: f64 = rng.random_range(0.0..1.0);
    let (chance, icon) = match biome {
        Biome::Ocean | Biome::Lake => (0.0, ""),
        Biome::SeaIce => (0.12, "ice-floe"),
        Biome::IceSheet => (0.2, "glacier"),
        Biome::Tundra => (0.2, "tundra"),
        Biome::Taiga => (0.55, "pine-forest"),
        Biome::SnowyPeaks => (0.85, "snowy-mountain"),
        Biome::Mountain => {
            let volcano = climate == Climate::Volcanic && pick < 0.5;
            (0.85, if volcano { "volcano" } else { "mountain" })
        }
        Biome::Hills => (0.45, "hills"),
        Biome::TemperateForest => (0.55, "forest"),
        Biome::Grassland => (0.08, "forest"),
        Biome::Desert => {
            if near_river {
                (0.6, "oasis")
            } else if elev > 0.3 {
                (0.35, "mesa")
            } else if pick < 0.6 {
                (0.35, "dunes")
            } else {
                (0.35, "cactus")
            }
        }
        Biome::Savanna => (0.15, "palm"),
        Biome::Jungle => (0.6, if pick < 0.8 { "jungle" } else { "palm" }),
        Biome::Swamp => (0.45, "swamp"),
        Biome::AshPlain => (0.1, "lava-field"),
        Biome::Lava => (0.5, "lava-field"),
    };
    (roll < chance && is_known_stamp(icon)).then_some(icon)
}

/// Scatters stamps over the map, sorted top to bottom so lower stamps
/// overlap the ones behind them.
pub fn scatter_stamps(
    grid: &TerrainGrid,
    climate: Climate,
    rng: &mut ChaCha8Rng,
) -> Vec<StampPlacement> {
    let (w, h) = (grid.layout.width, grid.layout.height);
    let r = spacing(w, h);
    let size = (r / 80.0).clamp(1.0, 4.0);
    let mut out = Vec::new();
    for (x, y) in poisson_disk(w, h, r, rng) {
        let i = grid.nearest(x, y);
        let biome = grid.biome[i];
        let near_river = grid.river[i] || grid.neighbours8(i).any(|n| grid.river[n]);
        let icon = choose_icon(biome, climate, near_river, grid.elevation(i), rng);
        let scale = size * rng.random_range(0.6..1.4);
        let rotation = rng.random_range(-8.0..8.0);
        if let Some(icon) = icon {
            out.push(StampPlacement {
                x: (x * 10.0).round() / 10.0,
                y: (y * 10.0).round() / 10.0,
                icon,
                scale: (scale * 100.0).round() / 100.0,
                rotation: (rotation * 10.0_f64).round() / 10.0,
                biome,
            });
        }
    }
    out.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::generator::rng::{Stage, stage_rng};

    #[test]
    fn poisson_points_keep_their_distance() {
        let mut rng = stage_rng(9, Stage::Scatter);
        let pts = poisson_disk(800.0, 600.0, 80.0, &mut rng);
        assert!(pts.len() > 30);
        for (i, a) in pts.iter().enumerate() {
            for b in &pts[i + 1..] {
                let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                assert!(d >= 80.0 - 1e-9);
            }
        }
    }
}
