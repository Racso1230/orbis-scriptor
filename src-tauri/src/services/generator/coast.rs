//! Coastlines: marching squares on the height grid, then simplification.
//!
//! Marching squares looks at each square of four grid nodes, and where the
//! land/water status differs between two corners it puts a point on that
//! side of the square (interpolated so the coast sits where the height
//! crosses sea level). Joining those points square by square gives closed
//! outlines of every island and lake.

use std::collections::VecDeque;

use super::fields::TerrainGrid;
use super::geometry::{contains, round_point, signed_area, simplify_ring};
use crate::models::map::Point;
use crate::services::maps::MAX_POINTS;

/// One traced outline.
#[derive(Debug, Clone, PartialEq)]
pub struct CoastPolygon {
    /// Closed ring in map pixels (first point not repeated).
    pub ring: Vec<Point>,
    /// `true` for land (island/continent), `false` for a lake inside land.
    pub is_land: bool,
    /// Area in map pixels².
    pub area: f64,
}

/// Islands smaller than this many grid cells are left out of the vector
/// output (they are still drawn in the background image) and not counted
/// as land masses.
pub fn min_island_cells(grid: &TerrainGrid) -> usize {
    (grid.layout.len() / 5000).max(4)
}

const NONE: u32 = u32::MAX;

/// The padded grid: one extra ring of water nodes all around, so every
/// outline closes even when land touches the map border.
struct Padded<'a> {
    // `'a` is a lifetime: it says `Padded` borrows the grid and cannot
    // outlive it. The compiler checks this so we never read freed memory.
    grid: &'a TerrainGrid,
    w: usize,
    h: usize,
}

impl Padded<'_> {
    /// Height minus sea level (> 0 is land) of padded node `(c, r)`.
    fn value(&self, c: usize, r: usize) -> f64 {
        if c == 0 || r == 0 || c == self.w - 1 || r == self.h - 1 {
            return -1e-3;
        }
        let i = self.grid.index(c - 1, r - 1);
        self.grid.height[i] - self.grid.sea_level
    }

    /// Map-pixel position of padded node `(c, r)`.
    fn position(&self, c: usize, r: usize) -> (f64, f64) {
        let cell = self.grid.layout.cell;
        ((c as f64 - 1.0) * cell, (r as f64 - 1.0) * cell)
    }

    fn horizontal(&self, c: usize, r: usize) -> u32 {
        (2 * (r * self.w + c)) as u32
    }

    fn vertical(&self, c: usize, r: usize) -> u32 {
        (2 * (r * self.w + c) + 1) as u32
    }

    /// The two end nodes of an edge id.
    fn ends(&self, edge: u32) -> ((usize, usize), (usize, usize)) {
        let e = edge as usize;
        let node = e / 2;
        let (c, r) = (node % self.w, node / self.w);
        if e.is_multiple_of(2) {
            ((c, r), (c + 1, r))
        } else {
            ((c, r), (c, r + 1))
        }
    }

    /// Where sea level crosses this edge, clamped to the map rectangle, and
    /// how far (map px) that point is from the land end.
    fn crossing(&self, edge: u32) -> (Point, (f64, f64), f64) {
        let (a, b) = self.ends(edge);
        let (va, vb) = (self.value(a.0, a.1), self.value(b.0, b.1));
        let (pa, pb) = (self.position(a.0, a.1), self.position(b.0, b.1));
        let t = (va / (va - vb)).clamp(0.0, 1.0);
        let x = pa.0 + t * (pb.0 - pa.0);
        let y = pa.1 + t * (pb.1 - pa.1);
        let layout = &self.grid.layout;
        let p = round_point(x.clamp(0.0, layout.width), y.clamp(0.0, layout.height));
        let land = if va > 0.0 { pa } else { pb };
        let margin = ((land.0 - x).powi(2) + (land.1 - y).powi(2)).sqrt();
        (p, land, margin)
    }
}

/// Records that edges `a` and `b` are joined by a segment.
fn link(adj: &mut [[u32; 2]], a: u32, b: u32) {
    for (from, to) in [(a, b), (b, a)] {
        let slot = &mut adj[from as usize];
        if slot[0] == NONE {
            slot[0] = to;
        } else {
            slot[1] = to;
        }
    }
}

/// Traces every coastline and lake shore as raw (unsimplified) rings.
fn trace_rings(grid: &TerrainGrid) -> Vec<(Vec<Point>, bool)> {
    let pad = Padded {
        grid,
        w: grid.cols() + 2,
        h: grid.rows() + 2,
    };
    let mut adj = vec![[NONE; 2]; 2 * pad.w * pad.h];
    for r in 0..pad.h - 1 {
        for c in 0..pad.w - 1 {
            let tl = pad.value(c, r) > 0.0;
            let tr = pad.value(c + 1, r) > 0.0;
            let br = pad.value(c + 1, r + 1) > 0.0;
            let bl = pad.value(c, r + 1) > 0.0;
            let case = (tl as u8) << 3 | (tr as u8) << 2 | (br as u8) << 1 | bl as u8;
            let top = pad.horizontal(c, r);
            let bottom = pad.horizontal(c, r + 1);
            let left = pad.vertical(c, r);
            let right = pad.vertical(c + 1, r);
            // The saddle cases (two diagonal land corners) are resolved by
            // the average of the four corners: land in the middle joins them.
            let centre_land = (pad.value(c, r)
                + pad.value(c + 1, r)
                + pad.value(c + 1, r + 1)
                + pad.value(c, r + 1))
                > 0.0;
            match case {
                1 | 14 => link(&mut adj, left, bottom),
                2 | 13 => link(&mut adj, bottom, right),
                3 | 12 => link(&mut adj, left, right),
                4 | 11 => link(&mut adj, top, right),
                6 | 9 => link(&mut adj, top, bottom),
                7 | 8 => link(&mut adj, left, top),
                5 => {
                    if centre_land {
                        link(&mut adj, left, top);
                        link(&mut adj, bottom, right);
                    } else {
                        link(&mut adj, left, bottom);
                        link(&mut adj, top, right);
                    }
                }
                10 => {
                    if centre_land {
                        link(&mut adj, top, right);
                        link(&mut adj, left, bottom);
                    } else {
                        link(&mut adj, left, top);
                        link(&mut adj, bottom, right);
                    }
                }
                _ => {}
            }
        }
    }

    let mut visited = vec![false; adj.len()];
    let mut rings = Vec::new();
    for start in 0..adj.len() {
        if visited[start] || adj[start][0] == NONE {
            continue;
        }
        let mut edges = vec![start as u32];
        visited[start] = true;
        let (mut prev, mut cur) = (start as u32, adj[start][0]);
        while cur != start as u32 && cur != NONE {
            visited[cur as usize] = true;
            edges.push(cur);
            let [a, b] = adj[cur as usize];
            let next = if a != prev { a } else { b };
            prev = cur;
            cur = next;
        }
        if edges.len() < 3 {
            continue;
        }
        let mut ring = Vec::with_capacity(edges.len());
        let mut best = (f64::NEG_INFINITY, (0.0, 0.0));
        for &e in &edges {
            let (p, land, margin) = pad.crossing(e);
            if margin > best.0 {
                best = (margin, land);
            }
            ring.push(p);
        }
        // The ring separates a land node from a water node on each edge;
        // if that land node is inside the ring, the ring outlines land.
        let land_pos = best.1;
        let is_land = contains(&ring, land_pos.0, land_pos.1);
        rings.push((ring, is_land));
    }
    rings
}

/// Traces, filters and simplifies all coast polygons, largest first.
pub fn coast_polygons(grid: &TerrainGrid) -> Vec<CoastPolygon> {
    let cell = grid.layout.cell;
    let min_area = min_island_cells(grid) as f64 * cell * cell;
    let mut out = Vec::new();
    for (ring, is_land) in trace_rings(grid) {
        let area = signed_area(&ring).abs();
        if area < min_area {
            continue;
        }
        let mut tolerance = cell * 0.5;
        let mut simple = simplify_ring(&ring, tolerance);
        while simple.len() > MAX_POINTS {
            tolerance *= 1.5;
            simple = simplify_ring(&ring, tolerance);
        }
        if simple.len() >= 3 {
            out.push(CoastPolygon {
                ring: simple,
                is_land,
                area,
            });
        }
    }
    // Largest first, so lakes and islands-in-lakes draw on top of the land
    // they sit in (objects draw in creation order).
    out.sort_by(|a, b| b.area.total_cmp(&a.area));
    out
}

/// Labels every land node with its connected region (4-neighbour). Returns
/// the label per node (`u32::MAX` for water) and the size of each region.
pub fn land_labels(grid: &TerrainGrid) -> (Vec<u32>, Vec<usize>) {
    let (cols, rows) = (grid.cols(), grid.rows());
    let mut labels = vec![u32::MAX; grid.layout.len()];
    let mut sizes = Vec::new();
    for start in 0..grid.layout.len() {
        if labels[start] != u32::MAX || !grid.is_land(start) {
            continue;
        }
        let label = sizes.len() as u32;
        labels[start] = label;
        let mut queue = VecDeque::from([start]);
        let mut size = 0;
        while let Some(i) = queue.pop_front() {
            size += 1;
            let (c, r) = grid.col_row(i);
            let mut ns = [None; 4];
            if c > 0 {
                ns[0] = Some(i - 1);
            }
            if c + 1 < cols {
                ns[1] = Some(i + 1);
            }
            if r > 0 {
                ns[2] = Some(i - cols);
            }
            if r + 1 < rows {
                ns[3] = Some(i + cols);
            }
            // `.flatten()` skips the `None`s and unwraps the `Some`s.
            for n in ns.into_iter().flatten() {
                if labels[n] == u32::MAX && grid.is_land(n) {
                    labels[n] = label;
                    queue.push_back(n);
                }
            }
        }
        sizes.push(size);
    }
    (labels, sizes)
}

/// Sizes (in nodes) of the connected land regions, largest first.
pub fn land_components(grid: &TerrainGrid) -> Vec<usize> {
    let (_, mut sizes) = land_labels(grid);
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    sizes
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::generator::Biome;
    use crate::services::generator::fields::GridLayout;

    /// A grid with a square island of land in the middle and a lake in it.
    fn island_grid() -> TerrainGrid {
        let layout = GridLayout::with_cell(160, 160, 8);
        let n = layout.len();
        let mut height = vec![-1.0; n];
        for r in 0..layout.rows {
            for c in 0..layout.cols {
                let land = (4..=16).contains(&c) && (4..=16).contains(&r);
                let lake = (9..=11).contains(&c) && (9..=11).contains(&r);
                if land && !lake {
                    height[r * layout.cols + c] = 1.0;
                }
            }
        }
        TerrainGrid {
            layout,
            height,
            sea_level: 0.0,
            min_height: -1.0,
            max_height: 1.0,
            moisture: vec![0.5; n],
            temperature: vec![0.5; n],
            biome: vec![Biome::Ocean; n],
            river: vec![false; n],
        }
    }

    #[test]
    fn traces_an_island_and_its_lake() {
        let grid = island_grid();
        let polys = coast_polygons(&grid);
        assert_eq!(polys.len(), 2);
        assert!(polys[0].is_land);
        assert!(!polys[1].is_land);
        // The island spans nodes 4..=16 (plus half a cell each side): ~104 px.
        assert!(
            (polys[0].area.sqrt() - 104.0).abs() < 8.0,
            "{}",
            polys[0].area
        );
        assert_eq!(land_components(&grid), vec![13 * 13 - 9]);
    }
}
