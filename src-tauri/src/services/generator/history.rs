//! Dynasties, relationships and timeline events (Phase 14c).
//!
//! Every kingdom gets a ruling house of three generations: the founder
//! (dead), the founder's child (the heir, usually dead) and the heir's child
//! who rules now, plus the ruler's spouse and one or two children. Dates are
//! built backwards from the present so ages always fit: the ruler's age
//! first, then each parent born 20–35 years before their child, and every
//! reign starting when the ruler is an adult.
//!
//! Events: the founding of each capital, the founding of each kingdom, the
//! current ruler's coronation, and one or two wars between every pair of
//! hostile neighbours, always after both kingdoms exist and before either
//! current ruler was crowned. All dates are [`HistDate`]s (years); the
//! writer turns them into day numbers.

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

use super::names::NameGenerator;
use super::places::{Settlement, Tier};
use super::plan::{
    EventKind, GeneratedPlaces, HistDate, HistEvent, Kingdom, PRESENT_YEAR, Person, PersonRole,
    Ref, Rel,
};
use super::politics::{Realms, UNCLAIMED};

/// Species of every generated person.
pub const SPECIES: &str = "Human";

/// The people and key years of one kingdom's ruling house.
#[derive(Debug, Clone, PartialEq)]
pub struct Dynasty {
    /// Indices into the character list.
    pub founder: usize,
    pub heir: usize,
    pub ruler: usize,
    pub spouse: usize,
    pub children: Vec<usize>,
    /// The house's family name, e.g. "Varn" (the house is "House Varn").
    pub family: String,
    pub capital_founded: i64,
    /// Year the founder founded the kingdom (and was crowned).
    pub founded: i64,
    /// Year the heir took the throne (the founder died that year).
    pub heir_crowned: i64,
    /// Year the current ruler was crowned.
    pub crowned: i64,
}

impl Dynasty {
    /// Who ruled in `year`.
    pub fn ruler_in(&self, year: i64) -> usize {
        if year < self.heir_crowned {
            self.founder
        } else if year < self.crowned {
            self.heir
        } else {
            self.ruler
        }
    }
}

/// Everything [`build`] produces besides the plan's `GeneratedHistory`
/// fields; the lore stage reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Chronicle {
    pub kingdoms: Vec<Kingdom>,
    pub dynasties: Vec<Dynasty>,
    pub characters: Vec<Person>,
    pub relationships: Vec<Rel>,
    /// Oldest first.
    pub events: Vec<HistEvent>,
}

/// "Kingdom of X" → "the Kingdom of X"; "The X Realm" → "the X Realm";
/// "X Dominion" → "the X Dominion". For use inside a sentence.
pub fn with_article(name: &str) -> String {
    match name.strip_prefix("The ") {
        Some(rest) => format!("the {rest}"),
        None => format!("the {name}"),
    }
}

/// Upper-cases the first letter ("the Kingdom" → "The Kingdom").
pub fn sentence_case(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn date(rng: &mut ChaCha8Rng, year: i64) -> HistDate {
    // Rounded to 1/1000 so the plan prints and compares cleanly.
    let part = (rng.random_range(0.0..1.0f64) * 1000.0).floor() / 1000.0;
    HistDate { year, part }
}

fn plural_years(n: i64) -> String {
    if n == 1 {
        "1 year".into()
    } else {
        format!("{n} years")
    }
}

/// Builds kingdoms, dynasties, relationships and events. Also sets
/// `kingdom` on every claimed settlement and mentions its kingdom in its
/// summary.
pub fn build(
    places: &mut GeneratedPlaces,
    realms: &Realms,
    names: &mut NameGenerator,
    rng: &mut ChaCha8Rng,
) -> Chronicle {
    // ---- Kingdoms and their settlements ----
    let mut kingdoms: Vec<Kingdom> = Vec::with_capacity(realms.realms.len());
    let mut families: Vec<String> = Vec::with_capacity(realms.realms.len());
    for realm in &realms.realms {
        let capital = &places.settlements[realm.capital];
        let culture = capital.culture;
        let name = names.kingdom(culture);
        let family = names.family(culture);
        let region = places
            .regions
            .iter()
            .find(|r| r.mass_index == capital.mass)
            .map(|r| r.phrase.clone())
            .unwrap_or_default();
        let summary = format!(
            "A kingdom {region}, ruled by House {family} from {}.",
            capital.name
        );
        kingdoms.push(Kingdom {
            name,
            house: format!("House {family}"),
            colour: realm.colour.clone(),
            capital: realm.capital,
            settlements: Vec::new(),
            cells: realm.cells,
            neighbours: realm.neighbours.clone(),
            allies: realm.allies.clone(),
            enemies: realm.enemies.clone(),
            culture,
            ruler: 0,
            founded: HistDate { year: 0, part: 0.0 },
            summary,
        });
        families.push(family);
    }
    for (i, s) in places.settlements.iter_mut().enumerate() {
        let owner = realms.owner[s.cell];
        if owner == UNCLAIMED {
            continue;
        }
        let k = owner as usize;
        s.kingdom = Some(k);
        kingdoms[k].settlements.push(i);
        s.summary = format!("{} Part of {}.", s.summary, with_article(&kingdoms[k].name));
    }

    // ---- Dynasties ----
    let mut characters: Vec<Person> = Vec::new();
    let mut dynasties: Vec<Dynasty> = Vec::new();
    let p = PRESENT_YEAR;
    for (k, kingdom) in kingdoms.iter_mut().enumerate() {
        let culture = kingdom.culture;
        let family = families[k].clone();
        let realm = with_article(&kingdom.name);

        let ruler_age: i64 = rng.random_range(22..=60);
        let ruler_born = p - ruler_age;
        let crowned = p - rng.random_range(1..=(ruler_age - 18).min(30));
        let heir_born = ruler_born - rng.random_range(20..=35);
        // `.max(..)`: nobody takes the throne before turning 18.
        let heir_crowned = (crowned - rng.random_range(10..=35)).max(heir_born + 18);
        let founder_born = heir_born - rng.random_range(20..=35);
        let founded = (heir_crowned - rng.random_range(10..=35)).max(founder_born + 18);
        let capital_founded = (founded - rng.random_range(20..=200)).max(0);
        let heir_alive = p - heir_born <= 85 && rng.random_bool(0.3);

        let mut person = |name: String,
                          role: PersonRole,
                          born: i64,
                          died: Option<i64>,
                          summary: String|
         -> usize {
            characters.push(Person {
                kingdom: k,
                name,
                role,
                alive: died.is_none(),
                born,
                died,
                age: if died.is_none() {
                    Some((p - born) as u32)
                } else {
                    None
                },
                species: SPECIES,
                summary,
            });
            characters.len() - 1
        };

        let founder_name = format!("{} {family}", names.given(culture));
        let heir_name = format!("{} {family}", names.given(culture));
        let ruler_name = format!("{} {family}", names.given(culture));
        let founder = person(
            founder_name.clone(),
            PersonRole::Founder,
            founder_born,
            Some(heir_crowned),
            format!(
                "Founder of {realm} and of House {family}, who reigned for {}.",
                plural_years(heir_crowned - founded)
            ),
        );
        let heir_summary = if heir_alive {
            format!(
                "Second ruler of {realm}, child of {founder_name}; gave up the throne to {ruler_name} after {}.",
                plural_years(crowned - heir_crowned)
            )
        } else {
            format!(
                "Second ruler of {realm}, child of {founder_name}; reigned for {}.",
                plural_years(crowned - heir_crowned)
            )
        };
        let heir = person(
            heir_name,
            PersonRole::Heir,
            heir_born,
            if heir_alive { None } else { Some(crowned) },
            heir_summary,
        );
        let ruler = person(
            ruler_name.clone(),
            PersonRole::Ruler,
            ruler_born,
            None,
            format!(
                "Ruler of {realm} of House {family}, on the throne for {}.",
                plural_years(p - crowned)
            ),
        );
        // The spouse comes from another family of the same culture.
        let spouse_age = (ruler_age + rng.random_range(-6..=6)).max(18);
        let spouse_name = format!("{} {}", names.given(culture), names.family(culture));
        let spouse = person(
            spouse_name.clone(),
            PersonRole::Spouse,
            p - spouse_age,
            None,
            format!("Spouse of {ruler_name}, ruler of {realm}."),
        );
        // Children: at least 16 years younger than both parents.
        let max_child_age = (ruler_age.min(spouse_age) - 16).clamp(1, 30);
        let mut ages = vec![rng.random_range(1..=max_child_age)];
        if max_child_age > 1 && rng.random_bool(0.6) {
            let mut second = rng.random_range(1..=max_child_age);
            if second == ages[0] {
                second = if second > 1 { second - 1 } else { second + 1 };
            }
            ages.push(second);
        }
        // Eldest first.
        ages.sort_unstable_by(|a, b| b.cmp(a));
        let mut children = Vec::new();
        for (n, age) in ages.into_iter().enumerate() {
            let summary = if n == 0 {
                format!("Eldest child of {ruler_name} and {spouse_name}, and heir to {realm}.")
            } else {
                format!("Child of {ruler_name} and {spouse_name}.")
            };
            let name = format!("{} {family}", names.given(culture));
            children.push(person(name, PersonRole::Child, p - age, None, summary));
        }
        kingdom.ruler = ruler;
        dynasties.push(Dynasty {
            founder,
            heir,
            ruler,
            spouse,
            children,
            family,
            capital_founded,
            founded,
            heir_crowned,
            crowned,
        });
    }

    // ---- Relationships ----
    let mut relationships = Vec::new();
    let mut rel = |source: Ref, target: Ref, kind: &'static str| {
        relationships.push(Rel {
            source,
            target,
            kind,
            label: None,
        });
    };
    for (k, (kingdom, d)) in kingdoms.iter().zip(&dynasties).enumerate() {
        let c = Ref::Character;
        let faction = Ref::Kingdom(k);
        let capital = Ref::Settlement(kingdom.capital);
        rel(c(d.founder), c(d.heir), "parent_of");
        rel(c(d.heir), c(d.ruler), "parent_of");
        rel(c(d.ruler), c(d.spouse), "spouse_of");
        for &child in &d.children {
            rel(c(d.ruler), c(child), "parent_of");
            rel(c(d.spouse), c(child), "parent_of");
        }
        if let &[a, b] = d.children.as_slice() {
            rel(c(a), c(b), "sibling_of");
        }
        rel(c(d.ruler), faction, "leader_of");
        let mut members = vec![d.founder, d.heir, d.ruler, d.spouse];
        members.extend(&d.children);
        for &m in &members {
            rel(c(m), faction, "member_of");
        }
        // The living royals live at court, in the capital.
        for &m in &members {
            if characters[m].alive {
                rel(c(m), capital, "located_in");
            }
        }
        // The capital is the kingdom's seat; it rules all its settlements.
        rel(faction, capital, "located_in");
        for &s in &kingdom.settlements {
            rel(faction, Ref::Settlement(s), "rules");
        }
    }
    // Symmetric kinds are stored once: only from the lower index.
    for (a, kingdom) in kingdoms.iter().enumerate() {
        for &b in kingdom.allies.iter().filter(|&&b| b > a) {
            rel(Ref::Kingdom(a), Ref::Kingdom(b), "ally_of");
        }
        for &b in kingdom.enemies.iter().filter(|&&b| b > a) {
            rel(Ref::Kingdom(a), Ref::Kingdom(b), "enemy_of");
        }
    }

    // ---- Events ----
    let settlements = &places.settlements;
    let mut events = Vec::new();
    for (k, (kingdom, d)) in kingdoms.iter_mut().zip(&dynasties).enumerate() {
        let capital = &settlements[kingdom.capital];
        let realm = with_article(&kingdom.name);
        events.push(HistEvent {
            name: format!("Founding of {}", capital.name),
            kind: EventKind::CapitalFounding,
            start: date(rng, d.capital_founded),
            end: None,
            importance: 3,
            participants: vec![Ref::Settlement(kingdom.capital)],
            summary: format!(
                "Settlers found {} {}.",
                capital.name,
                region_of(places, capital)
            ),
        });
        let founded = date(rng, d.founded);
        kingdom.founded = founded;
        events.push(HistEvent {
            name: format!("Founding of {realm}"),
            kind: EventKind::KingdomFounding,
            start: founded,
            end: None,
            importance: 3,
            participants: vec![
                Ref::Kingdom(k),
                Ref::Character(d.founder),
                Ref::Settlement(kingdom.capital),
            ],
            summary: format!(
                "{} unites the lands around {} as {realm}.",
                characters[d.founder].name, capital.name
            ),
        });
        events.push(HistEvent {
            name: format!("Coronation of {}", characters[d.ruler].name),
            kind: EventKind::Coronation,
            start: date(rng, d.crowned),
            end: None,
            importance: 2,
            participants: vec![
                Ref::Character(d.ruler),
                Ref::Kingdom(k),
                Ref::Settlement(kingdom.capital),
            ],
            summary: format!(
                "{} is crowned in {} as ruler of {realm}.",
                characters[d.ruler].name, capital.name
            ),
        });
    }
    let mut contested_used: Vec<usize> = Vec::new();
    for a in 0..kingdoms.len() {
        let rivals: Vec<usize> = kingdoms[a]
            .enemies
            .iter()
            .copied()
            .filter(|&b| b > a)
            .collect();
        for b in rivals {
            let (da, db) = (&dynasties[a], &dynasties[b]);
            // Strictly after both foundings, and over before either current
            // ruler was crowned.
            let mut lo = da.founded.max(db.founded) + 1;
            let hi = da.crowned.min(db.crowned) - 1;
            let wars = rng.random_range(1..=2);
            // A second war is fought over the same place as the first.
            let mut place = None;
            for n in 0..wars {
                let room = hi - lo;
                if room < 1 {
                    break;
                }
                let length = rng.random_range(2..=8).min(room);
                let start = rng.random_range(lo..=hi - length);
                let end = start + length;
                lo = end + 2;
                let place = *place.get_or_insert_with(|| {
                    contested(settlements, &kingdoms[a], &kingdoms[b], &contested_used)
                });
                contested_used.push(place);
                let s = &settlements[place];
                let name = match (n, rng.random_range(0..3)) {
                    (1, _) => format!("The Second War of {}", s.name),
                    (_, 0) if s.tier != Tier::Village => format!("The Siege of {}", s.name),
                    (_, 1) => format!("The {} Campaign", s.name),
                    _ => format!("The War of {}", s.name),
                };
                let winner = if rng.random_bool(0.5) { a } else { b };
                let (ra, rb) = (da.ruler_in(start), db.ruler_in(start));
                events.push(HistEvent {
                    name,
                    kind: EventKind::War,
                    start: date(rng, start),
                    end: Some(date(rng, end)),
                    importance: rng.random_range(4..=5),
                    participants: vec![
                        Ref::Kingdom(a),
                        Ref::Kingdom(b),
                        Ref::Character(ra),
                        Ref::Character(rb),
                        Ref::Settlement(place),
                    ],
                    summary: format!(
                        "{} and {} fight over {} for {}; {} prevails.",
                        sentence_case(&with_article(&kingdoms[a].name)),
                        with_article(&kingdoms[b].name),
                        s.name,
                        plural_years(length),
                        with_article(&kingdoms[winner].name),
                    ),
                });
            }
        }
    }
    // Oldest first. `sort_by` is stable: same-date events keep their order.
    events.sort_by(|x, y| {
        x.start
            .year
            .cmp(&y.start.year)
            .then(x.start.part.total_cmp(&y.start.part))
    });

    Chronicle {
        kingdoms,
        dynasties,
        characters,
        relationships,
        events,
    }
}

/// "in Valdmark" (the region phrase of a settlement), or "" if unknown.
fn region_of(places: &GeneratedPlaces, s: &Settlement) -> String {
    places
        .regions
        .iter()
        .find(|r| r.mass_index == s.mass)
        .map(|r| r.phrase.clone())
        .unwrap_or_default()
}

/// The settlement two kingdoms fight over: the non-capital settlement of
/// either side closest to the other side's capital (one not fought over
/// yet if possible), else the second kingdom's capital.
fn contested(settlements: &[Settlement], a: &Kingdom, b: &Kingdom, used: &[usize]) -> usize {
    let dist = |s: usize, capital: usize| {
        let (p, q) = (&settlements[s], &settlements[capital]);
        (p.x - q.x).hypot(p.y - q.y)
    };
    let mut candidates: Vec<(f64, usize)> = Vec::new();
    for (own, other) in [(a, b), (b, a)] {
        for &s in &own.settlements {
            if s != own.capital {
                candidates.push((dist(s, other.capital), s));
            }
        }
    }
    candidates.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    candidates
        .iter()
        .find(|(_, s)| !used.contains(s))
        .or(candidates.first())
        .map_or(b.capital, |(_, s)| *s)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeSet, HashSet, VecDeque};

    use serde_json::Value;

    use super::*;
    use crate::models::generator::{GenerationScope, GeneratorParams, LandShape};
    use crate::models::map::{MapGeometry, Point};
    use crate::models::relationship;
    use crate::services::generator::coast::land_labels;
    use crate::services::generator::geometry::contains;
    use crate::services::generator::plan::{
        GeneratedHistory, GeneratedMap, LAYER_KINGDOMS, LoreInline,
    };
    use crate::services::generator::{ObjectRole, generate_map};
    use crate::services::maps::{MAX_POINTS, MAX_STYLE_BYTES};

    fn history_params(seed: u32, shape: LandShape) -> GeneratorParams {
        GeneratorParams {
            seed,
            width: 800,
            height: 600,
            land_shape: shape,
            scope: GenerationScope::History,
            ..GeneratorParams::default()
        }
    }

    /// A few small worlds of different shapes, generated once.
    fn worlds() -> &'static Vec<GeneratedMap> {
        static WORLDS: std::sync::OnceLock<Vec<GeneratedMap>> = std::sync::OnceLock::new();
        WORLDS.get_or_init(|| {
            [
                (1, LandShape::Continent),
                (42, LandShape::Continent),
                (7, LandShape::TwinContinents),
                (3, LandShape::Archipelago),
                (99, LandShape::Pangaea),
            ]
            .into_iter()
            .map(|(seed, shape)| generate_map(&history_params(seed, shape)).unwrap())
            .collect()
        })
    }

    fn hist(map: &GeneratedMap) -> &GeneratedHistory {
        map.history.as_ref().unwrap()
    }

    #[test]
    fn every_capital_has_a_kingdom_and_claims_are_exclusive() {
        for map in worlds() {
            let h = hist(map);
            let settlements = &map.places.as_ref().unwrap().settlements;
            let capitals: Vec<usize> = (0..settlements.len())
                .filter(|&i| settlements[i].tier == Tier::Capital)
                .collect();
            assert!(!h.kingdoms.is_empty());
            assert_eq!(h.kingdoms.len(), capitals.len());
            let (labels, _) = land_labels(&map.grid);
            let capital_masses: HashSet<u32> = capitals
                .iter()
                .map(|&c| labels[settlements[c].cell])
                .collect();
            // `territory` holds ONE owner per node, so no node can belong to
            // two kingdoms; check it claims exactly the land with a capital.
            for (i, &owner) in h.territory.iter().enumerate() {
                let should_claim = map.grid.is_land(i) && capital_masses.contains(&labels[i]);
                assert_eq!(owner != UNCLAIMED, should_claim, "node {i}");
                if owner != UNCLAIMED {
                    assert!((owner as usize) < h.kingdoms.len());
                }
            }
            for (k, kingdom) in h.kingdoms.iter().enumerate() {
                assert_eq!(settlements[kingdom.capital].tier, Tier::Capital);
                assert_eq!(h.territory[settlements[kingdom.capital].cell], k as u32);
                assert!(kingdom.settlements.contains(&kingdom.capital));
                let cells = h.territory.iter().filter(|&&o| o == k as u32).count();
                assert_eq!(cells, kingdom.cells);
            }
            for (i, s) in settlements.iter().enumerate() {
                let owner = h.territory[s.cell];
                let expected = (owner != UNCLAIMED).then_some(owner as usize);
                assert_eq!(s.kingdom, expected);
                if let Some(k) = s.kingdom {
                    assert!(h.kingdoms[k].settlements.contains(&i));
                }
            }
        }
    }

    #[test]
    fn kingdoms_are_contiguous() {
        for map in worlds() {
            let h = hist(map);
            let grid = &map.grid;
            let settlements = &map.places.as_ref().unwrap().settlements;
            for (k, kingdom) in h.kingdoms.iter().enumerate() {
                // Flood fill from the capital through this kingdom's nodes
                // (4-neighbour): it must reach every one of them.
                let start = settlements[kingdom.capital].cell;
                let mut seen = vec![false; grid.layout.len()];
                seen[start] = true;
                let mut queue = VecDeque::from([start]);
                let mut reached = 0;
                while let Some(i) = queue.pop_front() {
                    reached += 1;
                    let (c, r) = grid.col_row(i);
                    let mut ns = Vec::new();
                    if c > 0 {
                        ns.push(i - 1);
                    }
                    if c + 1 < grid.cols() {
                        ns.push(i + 1);
                    }
                    if r > 0 {
                        ns.push(i - grid.cols());
                    }
                    if r + 1 < grid.rows() {
                        ns.push(i + grid.cols());
                    }
                    for n in ns {
                        if !seen[n] && h.territory[n] == k as u32 {
                            seen[n] = true;
                            queue.push_back(n);
                        }
                    }
                }
                assert_eq!(reached, kingdom.cells, "kingdom {k} is split");
            }
        }
    }

    #[test]
    fn kingdom_areas_are_valid_and_hold_their_capitals() {
        for map in worlds() {
            let h = hist(map);
            let settlements = &map.places.as_ref().unwrap().settlements;
            let layer = map
                .layers
                .iter()
                .find(|l| l.name == LAYER_KINGDOMS)
                .unwrap();
            assert!(!layer.objects.is_empty());
            let mut areas_of: Vec<Vec<&Vec<Point>>> = vec![Vec::new(); h.kingdoms.len()];
            for obj in &layer.objects {
                let ObjectRole::Kingdom { index } = obj.role else {
                    panic!("unexpected role {:?}", obj.role);
                };
                let MapGeometry::Area { points } = &obj.geometry else {
                    panic!("kingdoms are areas");
                };
                assert!((3..=MAX_POINTS).contains(&points.len()));
                assert!(obj.style.to_string().len() <= MAX_STYLE_BYTES);
                assert_eq!(obj.style["fill"], h.kingdoms[index].colour.as_str());
                assert_eq!(obj.style["fillOpacity"], 0.18);
                areas_of[index].push(points);
            }
            for (k, kingdom) in h.kingdoms.iter().enumerate() {
                let cap = &settlements[kingdom.capital];
                let reach = map.grid.layout.cell * 1.5;
                let holds = |ring: &&Vec<Point>| {
                    contains(ring, cap.x, cap.y)
                        || ring
                            .iter()
                            .any(|p| (p.x - cap.x).hypot(p.y - cap.y) <= reach)
                };
                assert!(
                    areas_of[k].iter().any(holds),
                    "capital of kingdom {k} is outside its areas"
                );
            }
            let colours: HashSet<&str> = h.kingdoms.iter().map(|k| k.colour.as_str()).collect();
            assert_eq!(colours.len(), h.kingdoms.len());
        }
    }

    #[test]
    fn relationships_follow_the_catalog() {
        let catalog = relationship::all();
        for map in worlds() {
            let h = hist(map);
            let mut seen = BTreeSet::new();
            for rel in &h.relationships {
                let def = catalog
                    .iter()
                    .find(|d| d.key == rel.kind)
                    .unwrap_or_else(|| panic!("unknown kind {}", rel.kind));
                let ok = |types: &[String], r: Ref| {
                    types.is_empty() || types.iter().any(|t| t == r.entity_type())
                };
                assert!(ok(&def.source_types, rel.source), "{rel:?}");
                assert!(ok(&def.target_types, rel.target), "{rel:?}");
                assert_ne!(rel.source, rel.target);
                // Symmetric kinds once per pair, whatever the direction.
                let key = if def.symmetric {
                    (
                        rel.source.min(rel.target),
                        rel.source.max(rel.target),
                        rel.kind,
                    )
                } else {
                    (rel.source, rel.target, rel.kind)
                };
                assert!(seen.insert(key), "duplicate {rel:?}");
            }
            for (k, kingdom) in h.kingdoms.iter().enumerate() {
                let has = |source: Ref, target: Ref, kind: &str| {
                    h.relationships
                        .iter()
                        .any(|r| r.source == source && r.target == target && r.kind == kind)
                };
                assert!(has(
                    Ref::Character(kingdom.ruler),
                    Ref::Kingdom(k),
                    "leader_of"
                ));
                for &s in &kingdom.settlements {
                    assert!(has(Ref::Kingdom(k), Ref::Settlement(s), "rules"));
                }
                for &a in &kingdom.allies {
                    assert!(kingdom.neighbours.contains(&a));
                    let (x, y) = (k.min(a), k.max(a));
                    assert!(has(Ref::Kingdom(x), Ref::Kingdom(y), "ally_of"));
                }
                for &e in &kingdom.enemies {
                    assert!(kingdom.neighbours.contains(&e));
                    assert!(!kingdom.allies.contains(&e));
                }
            }
        }
    }

    #[test]
    fn dynasties_have_consistent_ages() {
        for map in worlds() {
            let h = hist(map);
            for (k, kingdom) in h.kingdoms.iter().enumerate() {
                let people: Vec<&Person> = h.characters.iter().filter(|p| p.kingdom == k).collect();
                let one = |role: PersonRole| -> &Person {
                    let found: Vec<&Person> =
                        people.iter().copied().filter(|p| p.role == role).collect();
                    assert_eq!(found.len(), 1, "{role:?}");
                    found[0]
                };
                let founder = one(PersonRole::Founder);
                let heir = one(PersonRole::Heir);
                let ruler = one(PersonRole::Ruler);
                let spouse = one(PersonRole::Spouse);
                assert_eq!(&h.characters[kingdom.ruler], ruler);
                let children: Vec<&Person> = people
                    .iter()
                    .copied()
                    .filter(|p| p.role == PersonRole::Child)
                    .collect();
                assert!((1..=2).contains(&children.len()));

                assert!(!founder.alive && founder.died.is_some());
                assert!(ruler.alive && spouse.alive);
                assert!(founder.born + 16 <= heir.born);
                assert!(heir.born + 16 <= ruler.born);
                assert!(founder.born < kingdom.founded.year);
                assert!(founder.died.unwrap() > kingdom.founded.year);
                for p in &people {
                    assert_eq!(p.alive, p.died.is_none());
                    assert_eq!(p.alive, p.age.is_some());
                    assert_eq!(p.species, SPECIES);
                    if let Some(age) = p.age {
                        assert_eq!(i64::from(age), PRESENT_YEAR - p.born);
                        assert!(age <= 100, "{p:?}");
                    }
                    if let Some(died) = p.died {
                        assert!(died > p.born && died <= PRESENT_YEAR);
                    }
                }
                for c in &children {
                    assert!(c.alive);
                    assert!(c.born >= ruler.born + 16 && c.born >= spouse.born + 16);
                    assert!(c.age.unwrap() < ruler.age.unwrap());
                }
            }
        }
    }

    fn key(d: HistDate) -> (i64, i64) {
        (d.year, (d.part * 1000.0) as i64)
    }

    #[test]
    fn events_are_in_order_and_wars_are_between_enemy_neighbours() {
        let mut wars_seen = 0;
        for map in worlds() {
            let h = hist(map);
            for pair in h.events.windows(2) {
                assert!(key(pair[0].start) <= key(pair[1].start));
            }
            for e in &h.events {
                assert!((1..=5).contains(&e.importance));
                assert!(e.start.year >= 0 && e.start.year <= PRESENT_YEAR);
                assert!((0.0..1.0).contains(&e.start.part));
                if let Some(end) = e.end {
                    assert!(key(end) > key(e.start));
                }
                assert!(!e.participants.is_empty());
            }
            let find = |kind: EventKind, who: Ref| {
                h.events
                    .iter()
                    .find(|e| e.kind == kind && e.participants.contains(&who))
                    .unwrap()
            };
            for (k, kingdom) in h.kingdoms.iter().enumerate() {
                let capital = find(EventKind::CapitalFounding, Ref::Settlement(kingdom.capital));
                let founding = find(EventKind::KingdomFounding, Ref::Kingdom(k));
                let coronation = find(EventKind::Coronation, Ref::Kingdom(k));
                assert_eq!(capital.importance, 3);
                assert_eq!(founding.importance, 3);
                assert_eq!(coronation.importance, 2);
                assert!(key(capital.start) <= key(founding.start));
                assert!(key(founding.start) < key(coronation.start));
                assert_eq!(founding.start, kingdom.founded);
                assert!(
                    coronation
                        .participants
                        .contains(&Ref::Character(kingdom.ruler))
                );
            }
            for war in h.events.iter().filter(|e| e.kind == EventKind::War) {
                wars_seen += 1;
                assert!((4..=5).contains(&war.importance));
                let sides: Vec<usize> = war
                    .participants
                    .iter()
                    .filter_map(|r| match r {
                        Ref::Kingdom(k) => Some(*k),
                        _ => None,
                    })
                    .collect();
                assert_eq!(sides.len(), 2);
                let (a, b) = (sides[0], sides[1]);
                assert!(h.kingdoms[a].enemies.contains(&b));
                assert!(h.kingdoms[a].neighbours.contains(&b));
                let end = war.end.unwrap();
                assert!((1..=8).contains(&(end.year - war.start.year)));
                for side in [a, b] {
                    let coronation = find(EventKind::Coronation, Ref::Kingdom(side));
                    assert!(key(war.start) > key(h.kingdoms[side].founded));
                    assert!(key(end) < key(coronation.start));
                }
                assert!(
                    war.participants
                        .iter()
                        .any(|r| matches!(r, Ref::Settlement(_)))
                );
            }
        }
        assert!(wars_seen > 0, "no wars in any test world");
    }

    #[test]
    fn lore_stubs_are_tiptap_with_mentions() {
        for map in worlds() {
            let h = hist(map);
            assert_eq!(h.documents.len(), 3 * h.kingdoms.len());
            for doc in &h.documents {
                let (content, plain) = doc
                    .render(
                        |r| Ok(format!("id-{r:?}")),
                        |r| format!("name-{r:?}"),
                        |d| format!("{} AF", d.year + 1),
                    )
                    .unwrap();
                assert_eq!(content["type"], "doc");
                let blocks = content["content"].as_array().unwrap();
                assert_eq!(blocks[0]["type"], "heading");
                assert!((3..=4).contains(&blocks.len()), "{content}");
                let mut mentions = 0;
                for block in &blocks[1..] {
                    assert_eq!(block["type"], "paragraph");
                    for node in block["content"].as_array().unwrap() {
                        match node["type"].as_str().unwrap() {
                            "text" => assert!(!node["text"].as_str().unwrap().is_empty()),
                            "mention" => {
                                let id = node["attrs"]["id"].as_str().unwrap();
                                assert!(id.starts_with("id-"));
                                mentions += 1;
                            }
                            other => panic!("unexpected node {other}"),
                        }
                    }
                }
                assert!(mentions >= 1);
                assert!(plain.starts_with(&doc.heading));
                assert!(plain.contains("name-"));
                // Round-trips as JSON text (what `documents::save` stores).
                let back: Value = serde_json::from_str(&content.to_string()).unwrap();
                assert_eq!(back, content);
            }
            // A kingdom's lore mentions its capital (so backlinks show it).
            for (k, kingdom) in h.kingdoms.iter().enumerate() {
                let doc = h
                    .documents
                    .iter()
                    .find(|d| d.target == Ref::Kingdom(k))
                    .unwrap();
                let capital = LoreInline::Mention(Ref::Settlement(kingdom.capital));
                assert!(doc.paragraphs.iter().flatten().any(|p| *p == capital));
            }
        }
    }

    #[test]
    fn history_is_deterministic_and_varies_by_seed() {
        let params = history_params(42, LandShape::Continent);
        let a = generate_map(&params).unwrap();
        let b = generate_map(&params).unwrap();
        assert_eq!(a.history, b.history);
        assert_eq!(a.places, b.places);
        assert_eq!(a.layers, b.layers);
        let c = generate_map(&history_params(43, LandShape::Continent)).unwrap();
        assert_ne!(hist(&a).kingdoms, hist(&c).kingdoms);
        assert_ne!(hist(&a).characters, hist(&c).characters);
        // History does not change the places themselves (names included).
        let places_only = generate_map(&GeneratorParams {
            scope: GenerationScope::Places,
            ..params
        })
        .unwrap();
        let strip = |m: &GeneratedMap| -> Vec<(String, f64, f64)> {
            m.places
                .as_ref()
                .unwrap()
                .settlements
                .iter()
                .map(|s| (s.name.clone(), s.x, s.y))
                .collect()
        };
        assert_eq!(strip(&a), strip(&places_only));
    }

    #[test]
    fn people_and_kingdom_names_are_unique() {
        for map in worlds() {
            let h = hist(map);
            let mut names = HashSet::new();
            for p in &h.characters {
                assert!(names.insert(p.name.to_lowercase()), "{}", p.name);
            }
            for k in &h.kingdoms {
                assert!(names.insert(k.name.to_lowercase()), "{}", k.name);
                assert!(k.house.starts_with("House "));
            }
        }
    }

    #[test]
    fn articles_read_well() {
        assert_eq!(with_article("Kingdom of Varn"), "the Kingdom of Varn");
        assert_eq!(with_article("The Corin Realm"), "the Corin Realm");
        assert_eq!(sentence_case("the Bryn Dominion"), "The Bryn Dominion");
    }

    /// Report for the acceptance size. Run with
    /// `cargo test history_4000 -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn history_4000_by_3000_report() {
        let params = GeneratorParams {
            scope: GenerationScope::History,
            ..GeneratorParams::default()
        };
        let start = std::time::Instant::now();
        let map = generate_map(&params).unwrap();
        let total = start.elapsed();
        let h = hist(&map);
        let mut kinds = std::collections::BTreeMap::new();
        for r in &h.relationships {
            *kinds.entry(r.kind).or_insert(0) += 1;
        }
        let wars = h.events.iter().filter(|e| e.kind == EventKind::War).count();
        let areas = map
            .layers
            .iter()
            .find(|l| l.name == LAYER_KINGDOMS)
            .unwrap()
            .objects
            .len();
        println!("total {total:?}; stats {:?}", map.stats);
        println!(
            "kingdoms {} ({areas} areas), characters {}, events {} ({wars} wars), documents {}, relationships {} {kinds:?}",
            h.kingdoms.len(),
            h.characters.len(),
            h.events.len(),
            h.documents.len(),
            h.relationships.len()
        );
        for k in &h.kingdoms {
            println!(
                "  {} / {} / {} settlements / {} cells / allies {:?} enemies {:?} / ruler {}",
                k.name,
                k.house,
                k.settlements.len(),
                k.cells,
                k.allies,
                k.enemies,
                h.characters[k.ruler].name
            );
        }
        for p in &h.characters {
            println!("  {:?} {} {:?}: {}", p.role, p.name, p.age, p.summary);
        }
        for e in &h.events {
            println!(
                "  {} {:?}-{:?}: {}",
                e.name,
                e.start.year,
                e.end.map(|d| d.year),
                e.summary
            );
        }
        let (_, plain) = h.documents[0]
            .render(
                |r| Ok(format!("{r:?}")),
                |r| match r {
                    Ref::Kingdom(i) => h.kingdoms[i].name.clone(),
                    Ref::Character(i) => h.characters[i].name.clone(),
                    Ref::Event(i) => h.events[i].name.clone(),
                    Ref::Settlement(i) => map.places.as_ref().unwrap().settlements[i].name.clone(),
                },
                |d| format!("{} AF", d.year + 1),
            )
            .unwrap();
        println!("{plain}");
    }
}
