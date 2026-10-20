//! Settlements (Phase 14b): score every land cell, then place capitals,
//! towns and villages greedily, best score first, keeping them apart.
//!
//! The score rewards what real towns grew around: a coast or a river, flat
//! low land and a mild climate. Ice, high peaks, lava and deep desert are
//! ruled out entirely, so cold and dry worlds get fewer, smaller places.

use std::collections::VecDeque;

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

use super::coast::{land_labels, min_island_cells};
use super::fields::{TerrainGrid, distance_to_water};
use super::names::{NameGenerator, RegionSize};
use crate::models::generator::{Biome, GeneratorParams};

/// Most settlements a world gets (all tiers together).
pub const MAX_SETTLEMENTS: usize = 60;

/// How important a settlement is. Declared most important first, so
/// `Capital < Town < Village` when compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Capital,
    Town,
    Village,
}

impl Tier {
    /// The `kind` property of the location entity.
    pub fn kind(self) -> &'static str {
        match self {
            Tier::Capital => "city",
            Tier::Town => "town",
            Tier::Village => "village",
        }
    }

    /// Stamp size relative to the feature stamps.
    pub fn stamp_factor(self) -> f64 {
        match self {
            Tier::Capital => 1.5,
            Tier::Town => 1.2,
            Tier::Village => 0.9,
        }
    }
}

/// One placed settlement. The pure stage has no database ids: other parts
/// of the plan refer to a settlement by its index in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Settlement {
    pub name: String,
    pub tier: Tier,
    /// "city", "town" or "village" (the location's `kind` field).
    pub kind: &'static str,
    /// On an unfrozen sea coast (capitals and towns only).
    pub port: bool,
    /// Position in map pixels (a grid node).
    pub x: f64,
    pub y: f64,
    /// Grid node index.
    pub cell: usize,
    /// Land-mass label (see `coast::land_labels`).
    pub mass: usize,
    /// Name of the region (land mass) it lies in.
    pub region: String,
    /// Index of the naming culture (within this world's cultures).
    pub culture: usize,
    /// One sentence for the entity summary.
    pub summary: String,
}

/// The name of one land mass that has settlements.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionName {
    pub mass_index: usize,
    pub name: String,
    /// How a sentence refers to it, e.g. "on the Isle of Bryn".
    pub phrase: String,
}

/// Minimum distance (map px) kept by each tier to every earlier
/// settlement. Capitals may have been relaxed to fit them all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub capital: f64,
    pub town: f64,
    pub village: f64,
}

impl Spacing {
    pub fn for_tier(&self, tier: Tier) -> f64 {
        match tier {
            Tier::Capital => self.capital,
            Tier::Town => self.town,
            Tier::Village => self.village,
        }
    }
}

/// Everything about the land a settlement score needs, per grid node.
struct Survey {
    labels: Vec<u32>,
    sizes: Vec<usize>,
    land_cells: usize,
    /// Steps to any water / to unfrozen open sea / to a river.
    water: Vec<u32>,
    sea: Vec<u32>,
    river: Vec<u32>,
}

/// Grid steps (8-neighbour) from every node to the nearest node where
/// `source` is true, searching at most `limit` steps (farther = `u32::MAX`).
fn distance_to(grid: &TerrainGrid, source: impl Fn(usize) -> bool, limit: u32) -> Vec<u32> {
    let mut dist = vec![u32::MAX; grid.layout.len()];
    let mut queue = VecDeque::new();
    for (i, d) in dist.iter_mut().enumerate() {
        if source(i) {
            *d = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let next = dist[i] + 1;
        if next > limit {
            continue;
        }
        let ns: Vec<usize> = grid.neighbours8(i).collect();
        for n in ns {
            if dist[n] > next {
                dist[n] = next;
                queue.push_back(n);
            }
        }
    }
    dist
}

fn survey(grid: &TerrainGrid) -> Survey {
    let (labels, sizes) = land_labels(grid);
    let land_cells = sizes.iter().sum();
    Survey {
        labels,
        sizes,
        land_cells,
        water: distance_to_water(grid),
        sea: distance_to(grid, |i| grid.biome[i] == Biome::Ocean, 3),
        river: distance_to(grid, |i| grid.river[i], 3),
    }
}

/// How much a biome invites settlement on its own.
fn biome_base(b: Biome) -> f64 {
    match b {
        Biome::Grassland => 1.0,
        Biome::TemperateForest => 0.6,
        Biome::Savanna => 0.5,
        Biome::Hills => 0.1,
        Biome::Jungle => -0.3,
        Biome::Taiga => -0.4,
        Biome::Swamp => -0.8,
        Biome::AshPlain => -0.8,
        Biome::Desert => -1.0,
        Biome::Tundra => -1.2,
        Biome::Mountain => -1.8,
        _ => -2.0,
    }
}

/// Desert far from any river or coast: nobody lives there.
pub fn is_deep_desert(grid: &TerrainGrid, water_steps: u32, river_steps: u32, i: usize) -> bool {
    grid.biome[i] == Biome::Desert && water_steps > 2 && river_steps > 2
}

/// Biomes no settlement is ever placed on.
pub fn is_uninhabitable(b: Biome) -> bool {
    b.is_water() || matches!(b, Biome::IceSheet | Biome::SnowyPeaks | Biome::Lava)
}

/// The settlement score of every node; `None` where nobody can live.
fn scores(grid: &TerrainGrid, s: &Survey, rng: &mut ChaCha8Rng) -> Vec<Option<f64>> {
    let (cols, rows) = (grid.cols(), grid.rows());
    let min_mass = min_island_cells(grid);
    (0..grid.layout.len())
        .map(|i| {
            // Draw the jitter first, for every node, so the random stream
            // does not depend on which nodes are skipped.
            let jitter = rng.random_range(0.0..0.4);
            let (c, r) = grid.col_row(i);
            let b = grid.biome[i];
            let edge = c < 2 || r < 2 || c + 2 >= cols || r + 2 >= rows;
            if !grid.is_land(i)
                || is_uninhabitable(b)
                || edge
                || s.sizes[s.labels[i] as usize] < min_mass
                || is_deep_desert(grid, s.water[i], s.river[i], i)
            {
                return None;
            }
            let mut score = biome_base(b) + jitter;
            // Coast: open sea is best (harbours); lakes and frozen shores less.
            score += match (s.water[i], s.sea[i]) {
                (1, 1) => 1.0,
                (_, 1 | 2) => 0.8,
                (1 | 2, _) => 0.5,
                _ => 0.0,
            };
            score += match s.river[i] {
                0 => 1.2,
                1 => 1.5,
                2 => 0.9,
                _ => 0.0,
            };
            let e = grid.elevation(i);
            let slope = grid
                .neighbours8(i)
                .map(|n| (grid.elevation(n) - e).abs())
                .fold(0.0, f64::max);
            score += (1.0 - slope / 0.04).clamp(0.0, 1.0);
            score -= e;
            let t = grid.temperature[i];
            score += 0.8 * (1.0 - (t - 0.4).abs() / 0.5).clamp(-1.0, 1.0);
            let m = grid.moisture[i];
            score += 0.5 * (1.0 - (m - 0.5).abs() / 0.4).clamp(-1.0, 1.0);
            Some(score)
        })
        .collect()
}

/// A candidate site before naming.
#[derive(Debug, Clone, Copy)]
struct Site {
    cell: usize,
    tier: Tier,
    x: f64,
    y: f64,
    mass: usize,
}

fn far_enough(sites: &[Site], x: f64, y: f64, spacing: f64) -> bool {
    let min2 = spacing * spacing;
    sites
        .iter()
        .all(|s| (s.x - x).powi(2) + (s.y - y).powi(2) >= min2)
}

/// The ranked candidate nodes plus what `pick` needs to judge them.
struct Ranking<'a> {
    grid: &'a TerrainGrid,
    survey: &'a Survey,
    /// `(node, score)`, best first.
    ranked: Vec<(usize, f64)>,
}

impl Ranking<'_> {
    /// Adds up to `want` sites of `tier`, best score first. `mass_ok`
    /// decides which land masses may hold this tier.
    fn pick(
        &self,
        sites: &mut Vec<Site>,
        tier: Tier,
        want: usize,
        spacing: f64,
        mass_ok: impl Fn(&[Site], usize) -> bool,
    ) {
        let mut added = 0;
        for &(cell, _) in &self.ranked {
            if added >= want {
                break;
            }
            let (x, y) = self.grid.node_position(cell);
            let mass = self.survey.labels[cell] as usize;
            if far_enough(sites, x, y, spacing) && mass_ok(sites, mass) {
                sites.push(Site {
                    cell,
                    tier,
                    x,
                    y,
                    mass,
                });
                added += 1;
            }
        }
    }
}

/// How many towns and villages to aim for: 2× and 4× the capitals, scaled
/// by how much good land there is, capped at [`MAX_SETTLEMENTS`] in total.
fn targets(capitals: usize, good_share: f64) -> (usize, usize) {
    let factor = (good_share / 0.12).clamp(0.25, 1.5);
    let mut towns = ((2 * capitals) as f64 * factor).round() as usize;
    let mut villages = ((4 * capitals) as f64 * factor).round() as usize;
    let room = MAX_SETTLEMENTS.saturating_sub(capitals);
    villages = villages.min(room.saturating_sub(towns.min(room)));
    towns = towns.min(room);
    (towns, villages)
}

/// The size factor of settlement stamps, matching the feature stamps.
pub fn stamp_size(grid: &TerrainGrid) -> f64 {
    let r = super::scatter::spacing(grid.layout.width, grid.layout.height);
    (r / 80.0).clamp(1.0, 4.0)
}

/// Picks the sites of all tiers. Returns them (capitals first, then towns,
/// then villages) and the spacing each tier kept.
///
/// Capitals: `params.kingdoms` of them, at least 12 % of the map diagonal
/// apart. If they do not fit, the spacing shrinks step by step down to 6 %;
/// a world whose habitable land is tiny may still end up with fewer.
fn place_sites(
    grid: &TerrainGrid,
    params: &GeneratorParams,
    survey: &Survey,
    rng: &mut ChaCha8Rng,
) -> (Vec<Site>, Spacing) {
    let score = scores(grid, survey, rng);
    let mut ranked: Vec<(usize, f64)> = score
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|s| (i, s)))
        .collect();
    // Best first; ties broken by index so the order is fully determined.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let ranking = Ranking {
        grid,
        survey,
        ranked,
    };

    let diag = grid.layout.width.hypot(grid.layout.height);
    let size = stamp_size(grid);
    // Never closer than the stamps are wide (so they do not overlap).
    let floor = |tier: Tier| 64.0 * size * tier.stamp_factor() * 1.2;
    let land = survey.land_cells.max(1);
    let capital_mass = (min_island_cells(grid) * 2).max(land / 50);
    let town_mass = (min_island_cells(grid) * 2).max(land * 8 / 100);

    let want_capitals = usize::from(params.kingdoms);
    let mut capital_spacing = (0.12 * diag).max(floor(Tier::Capital));
    let min_capital_spacing = (0.06 * diag).max(floor(Tier::Capital));
    let mut sites = Vec::new();
    loop {
        sites.clear();
        ranking.pick(
            &mut sites,
            Tier::Capital,
            want_capitals,
            capital_spacing,
            |_, mass| survey.sizes[mass] >= capital_mass,
        );
        if sites.len() >= want_capitals || capital_spacing <= min_capital_spacing {
            break;
        }
        capital_spacing = (capital_spacing * 0.85).max(min_capital_spacing);
    }

    let good = score.iter().flatten().filter(|s| **s >= 2.0).count();
    let good_share = good as f64 / grid.layout.len().max(1) as f64;
    let (towns, villages) = targets(sites.len(), good_share);

    let spacing = Spacing {
        capital: capital_spacing,
        town: (0.05 * diag).max(floor(Tier::Town)),
        village: (0.03 * diag).max(floor(Tier::Village)),
    };
    // Towns: on a mass with a capital, or on a big island (≥ 8 % of the
    // land) of their own, where the first town is the local hub.
    ranking.pick(
        &mut sites,
        Tier::Town,
        towns,
        spacing.town,
        |sites, mass| {
            survey.sizes[mass] >= town_mass
                || sites
                    .iter()
                    .any(|s| s.mass == mass && s.tier == Tier::Capital)
        },
    );
    // Villages: only where a capital or town can be reached by road.
    ranking.pick(
        &mut sites,
        Tier::Village,
        villages,
        spacing.village,
        |sites, mass| sites.iter().any(|s| s.mass == mass),
    );
    (sites, spacing)
}

/// The summary sentence of a settlement.
fn summary(tier: Tier, port: bool, river: bool, coast: bool, phrase: &str) -> String {
    let what = match (tier, port) {
        (Tier::Capital, true) => "A port city and capital",
        (Tier::Capital, false) => "A capital city",
        (Tier::Town, true) => "A port town",
        (Tier::Town, false) if river => "A river town",
        (Tier::Town, false) => "A market town",
        (Tier::Village, _) if coast => "A fishing village",
        (Tier::Village, _) if river => "A riverside village",
        (Tier::Village, _) => "A village",
    };
    format!("{what} {phrase}.")
}

/// Places and names every settlement and names the regions they lie in.
pub fn place_settlements(
    grid: &TerrainGrid,
    params: &GeneratorParams,
    place_rng: &mut ChaCha8Rng,
    names: &mut NameGenerator,
) -> (Vec<Settlement>, Vec<RegionName>, Spacing) {
    let survey = survey(grid);
    let (sites, spacing) = place_sites(grid, params, &survey, place_rng);

    // Cultures: capitals take turns; everyone else follows the nearest
    // capital on the same land mass (or the mass's own lot otherwise).
    let cultures = names.culture_count();
    let capitals: Vec<(usize, &Site)> = sites
        .iter()
        .filter(|s| s.tier == Tier::Capital)
        .enumerate()
        .collect();
    let culture_of = |site: &Site| -> usize {
        capitals
            .iter()
            .filter(|(_, c)| c.mass == site.mass)
            .min_by(|a, b| {
                let da = (a.1.x - site.x).powi(2) + (a.1.y - site.y).powi(2);
                let db = (b.1.x - site.x).powi(2) + (b.1.y - site.y).powi(2);
                da.total_cmp(&db)
            })
            .map_or(site.mass % cultures, |(k, _)| k % cultures)
    };
    let site_cultures: Vec<usize> = sites.iter().map(culture_of).collect();

    // Regions, in order of their first (= most important) settlement.
    let largest = survey.sizes.iter().copied().max().unwrap_or(0);
    let mut regions: Vec<RegionName> = Vec::new();
    for (site, culture) in sites.iter().zip(&site_cultures) {
        if regions.iter().any(|r| r.mass_index == site.mass) {
            continue;
        }
        let cells = survey.sizes[site.mass];
        let size = if cells == largest || cells * 100 >= survey.land_cells * 15 {
            RegionSize::Large
        } else {
            RegionSize::Island
        };
        let label = names.region(*culture, size);
        regions.push(RegionName {
            mass_index: site.mass,
            name: label.name,
            phrase: label.phrase,
        });
    }

    let settlements = sites
        .iter()
        .zip(site_cultures)
        .map(|(site, culture)| {
            let i = site.cell;
            let coast = survey.water[i] <= 2;
            // Only capitals and towns become ports, and only on open sea.
            let port = site.tier != Tier::Village && survey.sea[i] <= 2;
            let river = survey.river[i] <= 1;
            let region = regions
                .iter()
                .find(|r| r.mass_index == site.mass)
                .map(|r| (r.name.clone(), r.phrase.clone()))
                .unwrap_or_default();
            Settlement {
                name: names.settlement(culture, port),
                tier: site.tier,
                kind: site.tier.kind(),
                port,
                x: site.x,
                y: site.y,
                cell: i,
                mass: site.mass,
                summary: summary(site.tier, port, river, coast, &region.1),
                region: region.0,
                culture,
            }
        })
        .collect();
    (settlements, regions, spacing)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn targets_scale_and_cap() {
        assert_eq!(targets(5, 0.12), (10, 20));
        assert_eq!(targets(5, 0.0), (3, 5));
        let (t, v) = targets(12, 1.0);
        assert_eq!(12 + t + v, MAX_SETTLEMENTS);
    }

    #[test]
    fn summaries_read_well() {
        assert_eq!(
            summary(Tier::Town, true, false, true, "on the Isle of Bryn"),
            "A port town on the Isle of Bryn."
        );
        assert_eq!(
            summary(Tier::Capital, false, true, false, "in Valdmark"),
            "A capital city in Valdmark."
        );
    }
}
