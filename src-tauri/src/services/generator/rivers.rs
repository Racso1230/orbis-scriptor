//! Rivers: drainage on the height grid, traced from the wettest catchments.
//!
//! 1. "Priority flood": starting from all water, flood the land in order of
//!    height. Each land node remembers the node it was reached from (its
//!    `receiver`, i.e. where its water flows). Pits are flooded over, so
//!    every land node drains to the sea or a lake: no river gets stuck.
//! 2. Flow accumulation: every node adds its moisture (rain) to its
//!    receiver; big totals mark big rivers.
//! 3. Tracing: from each river mouth, walk upstream along the biggest
//!    tributary (the main stem); side branches become their own rivers that
//!    end where they join their parent.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

use super::fields::TerrainGrid;
use super::geometry::{round_point, simplify};
use crate::models::map::Point;

/// At most this many river objects per map.
pub const MAX_RIVERS: usize = 40;

/// A traced river in map pixels, flowing from `points[0]` (source) to the
/// last point (mouth in water, or the join with its parent river).
#[derive(Debug, Clone, PartialEq)]
pub struct River {
    pub points: Vec<Point>,
    /// Grid nodes along the river, source to mouth (unsimplified).
    pub nodes: Vec<usize>,
    /// 1 (small) ..= 4 (great river), from the flow at the mouth.
    pub order: u8,
    /// Index of the river this one flows into, if it is a tributary.
    pub parent: Option<usize>,
}

/// An f64 that can live in a `BinaryHeap` (which needs a total order).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Key(f64);

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Receiver of every land node (`None` for water) plus the order in which
/// land nodes were reached (downstream first).
fn drainage(grid: &TerrainGrid) -> (Vec<Option<usize>>, Vec<usize>) {
    let n = grid.layout.len();
    let mut receiver = vec![None; n];
    let mut filled = grid.height.clone();
    let mut done = vec![false; n];
    // `Reverse` turns Rust's max-heap into a min-heap. Ties are broken by
    // the node index so the result never depends on heap internals.
    let mut heap = BinaryHeap::new();
    for (i, d) in done.iter_mut().enumerate() {
        if !grid.is_land(i) {
            *d = true;
            heap.push(Reverse((Key(filled[i]), i)));
        }
    }
    let mut order = Vec::new();
    while let Some(Reverse((Key(h), i))) = heap.pop() {
        let ns: Vec<usize> = grid.neighbours8(i).collect();
        for nb in ns {
            if done[nb] {
                continue;
            }
            done[nb] = true;
            receiver[nb] = Some(i);
            // A tiny slope keeps flooded pits draining in a definite direction.
            filled[nb] = filled[nb].max(h + 1e-7);
            order.push(nb);
            heap.push(Reverse((Key(filled[nb]), nb)));
        }
    }
    (receiver, order)
}

/// Finds rivers and marks `grid.river`. `rng` only adds meanders.
pub fn trace_rivers(grid: &mut TerrainGrid, rng: &mut ChaCha8Rng) -> Vec<River> {
    let n = grid.layout.len();
    let (receiver, order) = drainage(grid);

    // Flow: rain from each node plus everything upstream. Going through
    // `order` backwards visits upstream nodes before downstream ones. Rain
    // is mostly moisture, with a floor so dry worlds still get a few rivers.
    let mut flow: Vec<f64> = (0..n)
        .map(|i| {
            if grid.is_land(i) {
                0.3 + grid.moisture[i]
            } else {
                0.0
            }
        })
        .collect();
    for &i in order.iter().rev() {
        // A "let chain": the `&&` part only runs if the pattern matched,
        // and can already use `r`.
        if let Some(r) = receiver[i]
            && grid.is_land(r)
        {
            flow[r] += flow[i];
        }
    }
    let land = order.len();
    if land == 0 {
        return Vec::new();
    }
    // A river needs a catchment of roughly 1/120 of the land (rain-weighted).
    let threshold = (land as f64 / 100.0).max(12.0);

    // upstream[i] = river-sized nodes that drain into i, biggest first.
    let mut upstream: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        if let Some(r) = receiver[i]
            && flow[i] >= threshold
        {
            upstream[r].push(i);
        }
    }
    for list in &mut upstream {
        list.sort_by(|a, b| flow[*b].total_cmp(&flow[*a]).then(a.cmp(b)));
    }
    let max_flow = flow.iter().copied().fold(0.0, f64::max).max(1e-9);
    let min_len = 6;

    // Candidates: (flow, start node, the node it ends in, parent river).
    // Mouths end in their water receiver; tributaries in the parent's node.
    let mut pending: BinaryHeap<(Key, Reverse<usize>, usize, Option<usize>)> = BinaryHeap::new();
    for i in 0..n {
        if let Some(r) = receiver[i]
            && flow[i] >= threshold
            && !grid.is_land(r)
        {
            pending.push((Key(flow[i]), Reverse(i), r, None));
        }
    }

    let mut rivers: Vec<River> = Vec::new();
    while let Some((Key(f), Reverse(start), end, parent)) = pending.pop() {
        if rivers.len() >= MAX_RIVERS {
            break;
        }
        // Walk upstream along the biggest branch, remembering side branches.
        let mut nodes = vec![end, start];
        let mut branches = Vec::new();
        let mut cur = start;
        while let Some(&next) = upstream[cur].first() {
            for &side in &upstream[cur][1..] {
                branches.push((side, cur));
            }
            nodes.push(next);
            cur = next;
        }
        if nodes.len() < min_len {
            continue;
        }
        nodes.reverse(); // source → mouth
        let index = rivers.len();
        for (side, join) in branches {
            pending.push((Key(flow[side]), Reverse(side), join, Some(index)));
        }
        let ratio = f / max_flow;
        let order = if ratio > 0.5 {
            4
        } else if ratio > 0.2 {
            3
        } else if ratio > 0.07 {
            2
        } else {
            1
        };
        let points = river_points(grid, &nodes, rng);
        rivers.push(River {
            points,
            nodes,
            order,
            parent,
        });
    }
    for river in &rivers {
        for &i in &river.nodes {
            if grid.is_land(i) {
                grid.river[i] = true;
            }
        }
    }
    rivers
}

/// Node chain → simplified map-pixel polyline with small meanders. The two
/// ends are never moved, so a river still ends exactly in the water (or on
/// its parent).
fn river_points(grid: &TerrainGrid, nodes: &[usize], rng: &mut ChaCha8Rng) -> Vec<Point> {
    let jitter = grid.layout.cell * 0.3;
    let last = nodes.len() - 1;
    let raw: Vec<Point> = nodes
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            let (x, y) = grid.node_position(i);
            if k == 0 || k == last {
                round_point(x, y)
            } else {
                let dx = rng.random_range(-jitter..jitter);
                let dy = rng.random_range(-jitter..jitter);
                let (w, h) = (grid.layout.width, grid.layout.height);
                round_point((x + dx).clamp(0.0, w), (y + dy).clamp(0.0, h))
            }
        })
        .collect();
    simplify(&raw, grid.layout.cell * 0.4)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::generator::Biome;
    use crate::services::generator::fields::GridLayout;
    use crate::services::generator::rng::{Stage, stage_rng};

    /// A tilted plane: land rising to the right, sea on the left.
    #[test]
    fn rivers_flow_downhill_to_the_sea() {
        let layout = GridLayout::with_cell(400, 200, 8);
        let n = layout.len();
        let mut height = vec![0.0; n];
        for r in 0..layout.rows {
            for c in 0..layout.cols {
                let wobble = ((r as f64) * 0.7).sin() * 0.3;
                height[r * layout.cols + c] = c as f64 - 5.0 + wobble;
            }
        }
        let mut grid = TerrainGrid {
            layout,
            height,
            sea_level: 0.0,
            min_height: -5.0,
            max_height: 50.0,
            moisture: vec![1.0; n],
            temperature: vec![0.5; n],
            biome: vec![Biome::Grassland; n],
            river: vec![false; n],
        };
        let rivers = trace_rivers(&mut grid, &mut stage_rng(1, Stage::Rivers));
        assert!(!rivers.is_empty());
        for river in &rivers {
            let mouth = *river.nodes.last().unwrap();
            if river.parent.is_none() {
                assert!(!grid.is_land(mouth));
            }
        }
    }
}
