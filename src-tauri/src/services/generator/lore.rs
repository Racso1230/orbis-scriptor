//! Lore stubs (Phase 14c): a short document for every kingdom, its ruler
//! and its capital, written from templates. Related entities appear as
//! mentions, so backlinks, search and the graph work straight away.
//!
//! The documents only hold [`Ref`]s; the writer swaps them for entity ids
//! (and formats the years with the display calendar) when it saves them.

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

use super::history::{Chronicle, sentence_case, with_article};
use super::plan::{EventKind, GeneratedPlaces, HistDate, LoreDoc, LoreInline, Ref};

/// `LoreInline::Text` from anything string-like.
fn t(text: impl Into<String>) -> LoreInline {
    LoreInline::Text(text.into())
}

fn m(r: Ref) -> LoreInline {
    LoreInline::Mention(r)
}

/// "[A]", "[A] and [B]", "[A], [B] and [C]".
fn list(refs: &[Ref]) -> Vec<LoreInline> {
    let mut out = Vec::new();
    for (i, r) in refs.iter().enumerate() {
        if i > 0 {
            out.push(t(if i + 1 == refs.len() { " and " } else { ", " }));
        }
        out.push(m(*r));
    }
    out
}

/// Date of the first event of `kind` that involves `who`.
fn event_date(chronicle: &Chronicle, kind: EventKind, who: Ref) -> Option<HistDate> {
    chronicle
        .events
        .iter()
        .find(|e| e.kind == kind && e.participants.contains(&who))
        .map(|e| e.start)
}

/// Indices of the wars `who` took part in, oldest first.
fn wars_of(chronicle: &Chronicle, who: Ref) -> Vec<usize> {
    chronicle
        .events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == EventKind::War && e.participants.contains(&who))
        .map(|(i, _)| i)
        .collect()
}

/// One kingdom, one ruler and one capital document per kingdom.
pub fn build_documents(
    chronicle: &Chronicle,
    places: &GeneratedPlaces,
    rng: &mut ChaCha8Rng,
) -> Vec<LoreDoc> {
    let mut docs = Vec::new();
    for (k, (kingdom, d)) in chronicle
        .kingdoms
        .iter()
        .zip(&chronicle.dynasties)
        .enumerate()
    {
        let faction = Ref::Kingdom(k);
        let capital = Ref::Settlement(kingdom.capital);
        let cap = &places.settlements[kingdom.capital];
        let realm = with_article(&kingdom.name);
        let house = &kingdom.house;
        let region = places
            .regions
            .iter()
            .find(|r| r.mass_index == cap.mass)
            .map(|r| r.phrase.clone())
            .unwrap_or_default();
        let city = if cap.port { "a port city" } else { "a city" };
        let founded = kingdom.founded;
        let capital_founded =
            event_date(chronicle, EventKind::CapitalFounding, capital).unwrap_or(HistDate {
                year: d.capital_founded,
                part: 0.0,
            });
        let crowned = event_date(chronicle, EventKind::Coronation, Ref::Character(d.ruler))
            .unwrap_or(HistDate {
                year: d.crowned,
                part: 0.0,
            });
        let heir_alive = chronicle.characters[d.heir].alive;
        let wars = wars_of(chronicle, faction);
        let allies: Vec<Ref> = kingdom.allies.iter().map(|&a| Ref::Kingdom(a)).collect();
        let enemies: Vec<Ref> = kingdom.enemies.iter().map(|&e| Ref::Kingdom(e)).collect();
        let children: Vec<Ref> = d.children.iter().map(|&c| Ref::Character(c)).collect();

        // ---- The kingdom ----
        let mut p1 = vec![
            t(format!("{} was founded in ", sentence_case(&realm))),
            LoreInline::Year(founded),
            t(" by "),
            m(Ref::Character(d.founder)),
            t(format!(", first of {house}. Its capital, ")),
            m(capital),
            t(format!(
                ", {city} {region}, is older still: it was settled in "
            )),
            LoreInline::Year(capital_founded),
            t("."),
        ];
        if rng.random_bool(0.5) {
            p1.push(t(" Bards still sing of the founding."));
        }
        let mut p2 = vec![
            t("It is ruled today by "),
            m(Ref::Character(d.ruler)),
            t(", crowned in "),
            LoreInline::Year(crowned),
            t(if heir_alive {
                " when "
            } else {
                " after the death of "
            }),
            m(Ref::Character(d.heir)),
        ];
        p2.push(t(if heir_alive {
            " gave up the throne."
        } else {
            "."
        }));
        if !allies.is_empty() {
            p2.push(t(" It is allied with "));
            p2.extend(list(&allies));
            p2.push(t("."));
        }
        if !enemies.is_empty() {
            p2.push(t(" Its old rivals are "));
            p2.extend(list(&enemies));
            p2.push(t("."));
        }
        let mut paragraphs = vec![p1, p2];
        let others: Vec<Ref> = kingdom
            .settlements
            .iter()
            .filter(|&&s| s != kingdom.capital)
            .take(5)
            .map(|&s| Ref::Settlement(s))
            .collect();
        let mut p3 = Vec::new();
        if !wars.is_empty() {
            let war_refs: Vec<Ref> = wars.iter().map(|&w| Ref::Event(w)).collect();
            p3.push(t(if war_refs.len() == 1 {
                "Its history was shaped by "
            } else {
                "Its history was shaped by its wars: "
            }));
            p3.extend(list(&war_refs));
            p3.push(t(". "));
        }
        if !others.is_empty() {
            p3.push(t("Its other settlements include "));
            p3.extend(list(&others));
            p3.push(t("."));
        }
        if !p3.is_empty() {
            paragraphs.push(p3);
        }
        docs.push(LoreDoc {
            target: faction,
            heading: kingdom.name.clone(),
            paragraphs,
        });

        // ---- The ruler ----
        let ruler = &chronicle.characters[d.ruler];
        let mut r1 = vec![
            m(Ref::Character(d.ruler)),
            t(" rules "),
            m(faction),
            t(format!(
                " as the third of {house} to hold its throne, crowned in "
            )),
            LoreInline::Year(crowned),
            t(". A child of "),
            m(Ref::Character(d.heir)),
            t(" and grandchild of the founder, "),
            m(Ref::Character(d.founder)),
            t("."),
        ];
        if rng.random_bool(0.5) {
            r1.push(t(format!(
                " At {} years of age, the ruler is said to be {}.",
                ruler.age.unwrap_or_default(),
                if ruler.age.unwrap_or_default() < 35 {
                    "bold but untested"
                } else {
                    "shrewd and patient"
                }
            )));
        }
        let mut r2 = vec![
            t("The ruler is married to "),
            m(Ref::Character(d.spouse)),
            t(if children.len() == 1 {
                "; their child is "
            } else {
                "; their children are "
            }),
        ];
        r2.extend(list(&children));
        r2.push(t(". The court sits at "));
        r2.push(m(capital));
        r2.push(t("."));
        let mut paragraphs = vec![r1, r2];
        if !enemies.is_empty() {
            let mut r3 = vec![t("The crown keeps a wary eye on ")];
            r3.extend(list(&enemies));
            r3.push(t("."));
            paragraphs.push(r3);
        } else if !allies.is_empty() {
            let mut r3 = vec![t("The crown counts on its friendship with ")];
            r3.extend(list(&allies));
            r3.push(t("."));
            paragraphs.push(r3);
        }
        docs.push(LoreDoc {
            target: Ref::Character(d.ruler),
            heading: ruler.name.clone(),
            paragraphs,
        });

        // ---- The capital ----
        let c1 = vec![
            m(capital),
            t(" is the capital of "),
            m(faction),
            t(format!(", {city} {region}. It was founded in ")),
            LoreInline::Year(capital_founded),
            t(", long before the kingdom itself."),
        ];
        let c2 = vec![
            t(format!("It is the seat of {house} and of its ruler, ")),
            m(Ref::Character(d.ruler)),
            t(", who was crowned here in "),
            LoreInline::Year(crowned),
            t("."),
        ];
        let mut paragraphs = vec![c1, c2];
        if !wars.is_empty() {
            let war_refs: Vec<Ref> = wars.iter().map(|&w| Ref::Event(w)).collect();
            let mut c3 = vec![t("Its people still remember ")];
            c3.extend(list(&war_refs));
            c3.push(t("."));
            paragraphs.push(c3);
        }
        docs.push(LoreDoc {
            target: capital,
            heading: cap.name.clone(),
            paragraphs,
        });
    }
    docs
}
