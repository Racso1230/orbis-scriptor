//! Continent masks: the large-scale "where is land" field for each shape.
//!
//! A mask is a list of soft blobs (and, for the coastline shape, edges). The
//! height field is `mask × weight + noise`, so the mask decides the overall
//! arrangement and the noise makes the coast ragged and adds islands.
//!
//! Coordinates are in "aspect space": `x` runs `0..aspect` (aspect = width /
//! height) and `y` runs `0..1`, so a circle stays round on wide maps.

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

use crate::models::generator::LandShape;

/// A soft elliptical bump: 1 in the centre falling smoothly to 0 at the rim.
#[derive(Debug, Clone)]
struct Blob {
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    /// Peak value (negative blobs carve inland seas).
    weight: f64,
}

impl Blob {
    fn value(&self, x: f64, y: f64) -> f64 {
        let dx = (x - self.cx) / self.rx;
        let dy = (y - self.cy) / self.ry;
        let d2 = dx * dx + dy * dy;
        if d2 >= 1.0 {
            0.0
        } else {
            // (1 - d²)² is a cheap bell shape with a flat-ish top.
            let f = 1.0 - d2;
            self.weight * f * f
        }
    }
}

/// Which map edges carry land for `LandShape::Coastline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// A ready-to-sample mask, built once per generation from the shape rng.
#[derive(Debug, Clone)]
pub struct ShapeMask {
    aspect: f64,
    blobs: Vec<Blob>,
    /// Blobs subtracted after the max of `blobs` (inland seas).
    holes: Vec<Blob>,
    edges: Vec<Edge>,
    /// Depth of the sea channel carved where two blobs meet (0 = none), so
    /// twin continents stay apart.
    channel: f64,
    /// How strongly the mask counts against the noise.
    pub mask_weight: f64,
    pub noise_weight: f64,
    /// Base frequency of the height noise (features per map height).
    pub frequency: f64,
}

/// Uniform random number in `lo..hi`.
fn range(rng: &mut ChaCha8Rng, lo: f64, hi: f64) -> f64 {
    rng.random_range(lo..hi)
}

impl ShapeMask {
    pub fn new(shape: LandShape, aspect: f64, rng: &mut ChaCha8Rng) -> ShapeMask {
        let a = aspect;
        let mut mask = ShapeMask {
            aspect,
            blobs: Vec::new(),
            holes: Vec::new(),
            edges: Vec::new(),
            channel: 0.0,
            mask_weight: 1.0,
            noise_weight: 0.55,
            frequency: 3.0,
        };
        match shape {
            LandShape::Continent => {
                let cx = a * 0.5 + range(rng, -0.08, 0.08) * a;
                let cy = 0.5 + range(rng, -0.06, 0.06);
                let (rx, ry) = (0.44 * a.min(1.6), 0.44);
                mask.blobs.push(Blob {
                    cx,
                    cy,
                    rx,
                    ry,
                    weight: 1.0,
                });
                // A second overlapping lobe so continents are not plain ovals.
                let angle = range(rng, 0.0, std::f64::consts::TAU);
                mask.blobs.push(Blob {
                    cx: cx + angle.cos() * rx * 0.45,
                    cy: cy + angle.sin() * ry * 0.45,
                    rx: rx * 0.6,
                    ry: ry * 0.6,
                    weight: 0.95,
                });
                // A few offshore islands.
                let islands = rng.random_range(2..5);
                for _ in 0..islands {
                    let angle = range(rng, 0.0, std::f64::consts::TAU);
                    let dist = range(rng, 1.08, 1.25);
                    let r = range(rng, 0.05, 0.08);
                    mask.blobs.push(Blob {
                        cx: cx + angle.cos() * rx * dist,
                        cy: cy + angle.sin() * ry * dist,
                        rx: r,
                        ry: r,
                        weight: 0.85,
                    });
                }
            }
            LandShape::TwinContinents => {
                mask.channel = 0.9;
                let count = rng.random_range(2..4);
                let spots: &[(f64, f64, f64, f64)] = if count == 2 {
                    &[(0.26, 0.5, 0.21, 0.4), (0.74, 0.5, 0.21, 0.4)]
                } else {
                    &[
                        (0.22, 0.32, 0.18, 0.26),
                        (0.76, 0.3, 0.18, 0.26),
                        (0.48, 0.74, 0.2, 0.22),
                    ]
                };
                for &(x, y, rx, ry) in spots {
                    mask.blobs.push(Blob {
                        cx: (x + range(rng, -0.04, 0.04)) * a,
                        cy: y + range(rng, -0.06, 0.06),
                        rx: rx * a,
                        ry,
                        weight: 1.0,
                    });
                }
            }
            LandShape::Archipelago => {
                mask.mask_weight = 0.9;
                mask.noise_weight = 0.6;
                mask.frequency = 5.0;
                // A jittered grid of islands so they stay apart.
                let cols = ((6.0 * a / 1.33).round() as usize).clamp(3, 10);
                let rows = 4;
                let cw = a / cols as f64;
                let ch = 1.0 / rows as f64;
                for row in 0..rows {
                    for col in 0..cols {
                        if rng.random_bool(0.15) {
                            continue;
                        }
                        let r = range(rng, 0.28, 0.42) * cw.min(ch);
                        mask.blobs.push(Blob {
                            cx: (col as f64 + 0.5 + range(rng, -0.2, 0.2)) * cw,
                            cy: (row as f64 + 0.5 + range(rng, -0.2, 0.2)) * ch,
                            rx: r * range(rng, 0.8, 1.3),
                            ry: r,
                            weight: range(rng, 0.7, 1.0),
                        });
                    }
                }
            }
            LandShape::Pangaea => {
                mask.noise_weight = 0.45;
                mask.blobs.push(Blob {
                    cx: a * 0.5,
                    cy: 0.5,
                    rx: a * 0.6,
                    ry: 0.62,
                    weight: 1.0,
                });
                // Inland seas carved out of the interior.
                let holes = rng.random_range(2..4);
                for _ in 0..holes {
                    let r = range(rng, 0.07, 0.11);
                    mask.holes.push(Blob {
                        cx: a * range(rng, 0.3, 0.7),
                        cy: range(rng, 0.3, 0.7),
                        rx: r * range(rng, 1.0, 1.6),
                        ry: r,
                        weight: 0.75,
                    });
                }
            }
            LandShape::Coastline => {
                let all = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];
                let first = all[rng.random_range(0..4)];
                mask.edges.push(first);
                if rng.random_bool(0.5) {
                    // A second edge next to the first one (an L-shaped coast).
                    let next = match first {
                        Edge::Left | Edge::Right => [Edge::Top, Edge::Bottom],
                        Edge::Top | Edge::Bottom => [Edge::Left, Edge::Right],
                    };
                    mask.edges.push(next[rng.random_range(0..2)]);
                }
            }
        }
        mask
    }

    /// The mask value at aspect-space point `(x, y)`, roughly `0..1`.
    pub fn value(&self, x: f64, y: f64) -> f64 {
        let mut v: f64 = 0.0;
        for blob in &self.blobs {
            v = v.max(blob.value(x, y));
        }
        if self.channel > 0.0 {
            // Distances (in blob radii) to the nearest and second-nearest
            // blob centres. Where they are about equal we are on the line
            // halfway between two continents: carve sea there.
            let (mut d1, mut d2) = (f64::INFINITY, f64::INFINITY);
            for b in &self.blobs {
                let d = (((x - b.cx) / b.rx).powi(2) + ((y - b.cy) / b.ry).powi(2)).sqrt();
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                } else if d < d2 {
                    d2 = d;
                }
            }
            v -= self.channel * (1.0 - smoothstep(0.0, 0.45, d2 - d1));
        }
        for hole in &self.holes {
            v -= hole.value(x, y);
        }
        for edge in &self.edges {
            // Distance from that edge, 0 at the edge and 1 at the far side.
            let d = match edge {
                Edge::Left => x / self.aspect,
                Edge::Right => 1.0 - x / self.aspect,
                Edge::Top => y,
                Edge::Bottom => 1.0 - y,
            };
            v = v.max(1.0 - smoothstep(0.15, 0.7, d));
        }
        v
    }

    /// True when land may touch the map border (only the coastline shape).
    pub fn land_on_edges(&self) -> bool {
        !self.edges.is_empty()
    }
}

/// 0 below `lo`, 1 above `hi`, and a smooth S-curve between.
pub fn smoothstep(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::generator::rng::{Stage, stage_rng};

    #[test]
    fn continent_mask_is_high_in_the_middle_and_low_at_corners() {
        let mut rng = stage_rng(1, Stage::Shape);
        let mask = ShapeMask::new(LandShape::Continent, 1.33, &mut rng);
        assert!(mask.value(0.665, 0.5) > 0.5);
        assert!(mask.value(0.0, 0.0) < 0.05);
    }

    #[test]
    fn coastline_mask_touches_an_edge() {
        let mut rng = stage_rng(3, Stage::Shape);
        let mask = ShapeMask::new(LandShape::Coastline, 1.0, &mut rng);
        assert!(mask.land_on_edges());
        let edges = [(0.0, 0.5), (1.0, 0.5), (0.5, 0.0), (0.5, 1.0)];
        assert!(edges.iter().any(|&(x, y)| mask.value(x, y) > 0.9));
    }
}
