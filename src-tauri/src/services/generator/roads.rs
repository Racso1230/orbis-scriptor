//! Roads between settlements: A* search over a terrain cost grid.
//!
//! Capitals are joined by a minimum spanning tree, each town to the nearest
//! capital or earlier town, each village to the nearest capital or town.
//! Land that a road already crosses is cheaper, so later roads join the
//! existing network instead of running beside it; the shared stretches are
//! only drawn once.

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashSet};

use super::fields::TerrainGrid;
use super::geometry::{round_point, simplify_with};
use super::places::{Settlement, Tier};
use crate::models::generator::Biome;
use crate::models::map::Point;
use crate::services::maps::MAX_POINTS;

/// A drawn road: a polyline in map pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Road {
    pub points: Vec<Point>,
    /// The tier of the connection it was built for (capital roads are the
    /// widest).
    pub tier: Tier,
    /// Grid nodes along the road (unsimplified), first to last.
    pub nodes: Vec<usize>,
}

/// One requested connection between two settlements (by index) that A*
/// found a way for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    pub from: usize,
    pub to: usize,
}

/// Cost of entering a node, or `None` for water (impassable).
fn node_cost(grid: &TerrainGrid, i: usize) -> Option<f64> {
    if !grid.is_land(i) {
        return None;
    }
    let biome = match grid.biome[i] {
        Biome::Grassland => 1.0,
        Biome::Savanna => 1.1,
        Biome::Tundra => 1.3,
        Biome::AshPlain => 1.4,
        Biome::TemperateForest | Biome::Desert => 1.5,
        Biome::Taiga => 1.6,
        Biome::Hills => 1.8,
        Biome::Jungle => 2.5,
        Biome::Swamp => 3.0,
        Biome::Mountain => 5.0,
        Biome::IceSheet | Biome::Lava => 6.0,
        Biome::SnowyPeaks => 8.0,
        // Water biomes never reach here (checked above).
        _ => 10.0,
    };
    let e = grid.elevation(i);
    let slope = grid
        .neighbours8(i)
        .map(|n| (grid.elevation(n) - e).abs())
        .fold(0.0, f64::max);
    // Crossing a river means a ford or a bridge: allowed, but not free.
    let river = if grid.river[i] { 4.0 } else { 0.0 };
    Some(biome * (1.0 + slope * 40.0) + river)
}

/// How much cheaper a node already on a road is.
const ROAD_DISCOUNT: f64 = 0.35;

/// A* priority: lowest `f` first. `BinaryHeap` is a max-heap, so entries
/// are wrapped in `Reverse`. The node index breaks ties deterministically.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    f: f64,
    node: usize,
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.f.total_cmp(&other.f).then(self.node.cmp(&other.node))
    }
}

/// Reusable A* state: the arrays are allocated once and "cleared" by
/// bumping `round`, so each search does not touch all 190k nodes again.
struct Search<'a> {
    grid: &'a TerrainGrid,
    cost: Vec<Option<f64>>,
    on_road: Vec<bool>,
    g: Vec<f64>,
    came_from: Vec<usize>,
    seen: Vec<u32>,
    closed: Vec<u32>,
    round: u32,
}

impl<'a> Search<'a> {
    fn new(grid: &'a TerrainGrid) -> Search<'a> {
        let n = grid.layout.len();
        Search {
            grid,
            cost: (0..n).map(|i| node_cost(grid, i)).collect(),
            on_road: vec![false; n],
            g: vec![0.0; n],
            came_from: vec![usize::MAX; n],
            seen: vec![0; n],
            closed: vec![0; n],
            round: 0,
        }
    }

    fn entry_cost(&self, i: usize) -> Option<f64> {
        let c = self.cost[i]?;
        Some(if self.on_road[i] {
            c * ROAD_DISCOUNT
        } else {
            c
        })
    }

    /// Octile distance in grid steps (the exact length of an 8-direction
    /// path on an empty grid), scaled down a little to stay optimistic.
    fn heuristic(&self, a: usize, b: usize) -> f64 {
        let (ac, ar) = self.grid.col_row(a);
        let (bc, br) = self.grid.col_row(b);
        let dx = ac.abs_diff(bc) as f64;
        let dy = ar.abs_diff(br) as f64;
        let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
        (hi - lo + lo * std::f64::consts::SQRT_2) * 0.8
    }

    /// The cheapest node chain from `start` to `goal`, both included.
    fn path(&mut self, start: usize, goal: usize) -> Option<Vec<usize>> {
        self.round += 1;
        let round = self.round;
        let cols = self.grid.cols() as isize;
        let rows = self.grid.rows() as isize;
        let mut open = BinaryHeap::new();
        self.g[start] = 0.0;
        self.seen[start] = round;
        self.came_from[start] = usize::MAX;
        open.push(Reverse(Entry {
            f: self.heuristic(start, goal),
            node: start,
        }));
        while let Some(Reverse(Entry { node, .. })) = open.pop() {
            if self.closed[node] == round {
                continue;
            }
            if node == goal {
                let mut chain = vec![goal];
                let mut at = goal;
                while self.came_from[at] != usize::MAX {
                    at = self.came_from[at];
                    chain.push(at);
                }
                chain.reverse();
                return Some(chain);
            }
            self.closed[node] = round;
            let (c, r) = self.grid.col_row(node);
            let (c, r) = (c as isize, r as isize);
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let (nc, nr) = (c + dx, r + dy);
                if nc < 0 || nr < 0 || nc >= cols || nr >= rows {
                    continue;
                }
                let next = (nr * cols + nc) as usize;
                if self.closed[next] == round {
                    continue;
                }
                let Some(enter) = self.entry_cost(next) else {
                    continue;
                };
                let diagonal = dx != 0 && dy != 0;
                // A diagonal step must not squeeze between two water
                // nodes, or the road would cut across a strait.
                if diagonal {
                    let side_a = (r * cols + nc) as usize;
                    let side_b = (nr * cols + c) as usize;
                    if self.cost[side_a].is_none() || self.cost[side_b].is_none() {
                        continue;
                    }
                }
                let step = if diagonal {
                    std::f64::consts::SQRT_2
                } else {
                    1.0
                };
                let here = self.entry_cost(node).unwrap_or(enter);
                let g = self.g[node] + step * (here + enter) / 2.0;
                if self.seen[next] != round || g < self.g[next] {
                    self.seen[next] = round;
                    self.g[next] = g;
                    self.came_from[next] = node;
                    open.push(Reverse(Entry {
                        f: g + self.heuristic(next, goal),
                        node: next,
                    }));
                }
            }
        }
        None
    }
}

/// True if the straight segment `a`–`b` stays on land all the way, checked
/// at quarter-cell steps against the nearest grid node.
pub fn segment_on_land(grid: &TerrainGrid, a: Point, b: Point) -> bool {
    let len = (b.x - a.x).hypot(b.y - a.y);
    let steps = (len / (grid.layout.cell * 0.25)).ceil().max(1.0) as usize;
    (0..=steps).all(|k| {
        let t = k as f64 / steps as f64;
        grid.is_land(grid.nearest(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t))
    })
}

/// Node chain → simplified polyline that never cuts across water.
fn polyline(grid: &TerrainGrid, nodes: &[usize]) -> Vec<Point> {
    let raw: Vec<Point> = nodes
        .iter()
        .map(|&i| {
            let (x, y) = grid.node_position(i);
            round_point(x, y)
        })
        .collect();
    let mut tolerance = grid.layout.cell * 0.6;
    let mut points = simplify_with(&raw, tolerance, |a, b| segment_on_land(grid, a, b));
    while points.len() > MAX_POINTS {
        tolerance *= 2.0;
        points = simplify_with(&raw, tolerance, |a, b| segment_on_land(grid, a, b));
    }
    points
}

fn dist2(a: &Settlement, b: &Settlement) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

/// Which pairs of settlements to connect, in building order.
fn connections(settlements: &[Settlement]) -> Vec<(usize, usize, Tier)> {
    let of_tier = |t: Tier| -> Vec<usize> {
        (0..settlements.len())
            .filter(|&i| settlements[i].tier == t)
            .collect()
    };
    let capitals = of_tier(Tier::Capital);
    let mut out = Vec::new();

    // Capitals: Kruskal's minimum spanning tree over same-mass pairs.
    // Union-find: `parent[k]` points towards the representative of k's set.
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (a, &i) in capitals.iter().enumerate() {
        for &j in &capitals[a + 1..] {
            if settlements[i].mass == settlements[j].mass {
                pairs.push((dist2(&settlements[i], &settlements[j]), i, j));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut parent: Vec<usize> = (0..settlements.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for (_, i, j) in pairs {
        let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
        if ri != rj {
            parent[ri] = rj;
            out.push((i, j, Tier::Capital));
        }
    }

    // Nearest settlement among `candidates` on the same land mass.
    let nearest = |i: usize, candidates: &[usize]| -> Option<usize> {
        candidates
            .iter()
            .copied()
            .filter(|&j| j != i && settlements[j].mass == settlements[i].mass)
            .min_by(|&a, &b| {
                dist2(&settlements[i], &settlements[a])
                    .total_cmp(&dist2(&settlements[i], &settlements[b]))
                    .then(a.cmp(&b))
            })
    };

    // Towns closest to a capital first, so every town links into the
    // network that already reaches a capital.
    let mut towns = of_tier(Tier::Town);
    let to_capital = |i: usize| {
        nearest(i, &capitals).map_or(f64::INFINITY, |c| dist2(&settlements[i], &settlements[c]))
    };
    towns.sort_by(|&a, &b| to_capital(a).total_cmp(&to_capital(b)).then(a.cmp(&b)));
    let mut hubs = capitals.clone();
    for &t in &towns {
        if let Some(j) = nearest(t, &hubs) {
            out.push((t, j, Tier::Town));
        }
        hubs.push(t);
    }
    for v in of_tier(Tier::Village) {
        if let Some(j) = nearest(v, &hubs) {
            out.push((v, j, Tier::Village));
        }
    }
    out
}

/// Builds the road network. Returns the drawn roads and the connections
/// that were made.
pub fn build_roads(grid: &TerrainGrid, settlements: &[Settlement]) -> (Vec<Road>, Vec<Route>) {
    let mut search = Search::new(grid);
    // Grid edges already drawn, as (smaller node, larger node).
    let mut drawn: HashSet<(usize, usize)> = HashSet::new();
    let mut roads = Vec::new();
    let mut routes = Vec::new();
    for (from, to, tier) in connections(settlements) {
        let Some(chain) = search.path(settlements[from].cell, settlements[to].cell) else {
            continue;
        };
        routes.push(Route { from, to });
        for &n in &chain {
            search.on_road[n] = true;
        }
        // Split the chain into runs of edges that are not drawn yet.
        let mut run: Vec<usize> = Vec::new();
        for pair in chain.windows(2) {
            let edge = (pair[0].min(pair[1]), pair[0].max(pair[1]));
            if drawn.insert(edge) {
                if run.is_empty() {
                    run.push(pair[0]);
                }
                run.push(pair[1]);
            } else if !run.is_empty() {
                // `std::mem::take` moves the vector out, leaving an empty one.
                let nodes = std::mem::take(&mut run);
                roads.push(Road {
                    points: polyline(grid, &nodes),
                    tier,
                    nodes,
                });
            }
        }
        if run.len() >= 2 {
            roads.push(Road {
                points: polyline(grid, &run),
                tier,
                nodes: run,
            });
        }
    }
    (roads, routes)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::generator::fields::GridLayout;

    /// A 21×21 land grid with a wall of water down the middle that has one
    /// gap at the bottom.
    fn walled_grid() -> TerrainGrid {
        let layout = GridLayout::with_cell(160, 160, 8);
        let n = layout.len();
        let mut grid = TerrainGrid {
            layout,
            height: vec![1.0; n],
            sea_level: 0.0,
            min_height: -1.0,
            max_height: 1.0,
            moisture: vec![0.5; n],
            temperature: vec![0.4; n],
            biome: vec![Biome::Grassland; n],
            river: vec![false; n],
        };
        for r in 0..18 {
            let i = grid.index(10, r);
            grid.height[i] = -1.0;
            grid.biome[i] = Biome::Ocean;
        }
        grid
    }

    #[test]
    fn a_star_goes_around_water() {
        let grid = walled_grid();
        let mut search = Search::new(&grid);
        let (a, b) = (grid.index(5, 2), grid.index(15, 2));
        let chain = search.path(a, b).unwrap();
        assert_eq!((chain[0], *chain.last().unwrap()), (a, b));
        assert!(chain.iter().all(|&i| grid.is_land(i)));
        // It had to go down to the gap (row 18 or below).
        assert!(chain.iter().any(|&i| grid.col_row(i).1 >= 18));
        let line = polyline(&grid, &chain);
        assert!(line.len() < chain.len());
        for w in line.windows(2) {
            assert!(segment_on_land(&grid, w[0], w[1]));
        }
    }

    #[test]
    fn no_path_across_closed_water() {
        let mut grid = walled_grid();
        for r in 18..grid.rows() {
            let i = grid.index(10, r);
            grid.height[i] = -1.0;
        }
        let mut search = Search::new(&grid);
        assert!(search.path(grid.index(5, 2), grid.index(15, 2)).is_none());
        // The search state is reusable after a failed search.
        let near = search.path(grid.index(2, 2), grid.index(6, 6)).unwrap();
        assert_eq!(near.len(), 5);
    }
}
