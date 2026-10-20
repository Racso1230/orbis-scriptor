//! Models for the procedural world generator (Phase 14).
//!
//! `GeneratorParams` is what the wizard sends; `GenerationStats` and `Biome`
//! come back in the "finished" event. The full in-memory plan
//! (`GeneratedMap`) is plain Rust and lives in `services::generator::plan`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::{AppError, AppResult};
use crate::services::maps::{MAX_MAP_SIZE, MIN_MAP_SIZE};

/// The overall arrangement of land and sea.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LandShape {
    /// One large land mass with a ragged coast and a few offshore islands.
    #[default]
    Continent,
    /// Two or three large land masses separated by sea.
    TwinContinents,
    /// Many small and medium islands.
    Archipelago,
    /// One huge land mass with inland seas and lakes.
    Pangaea,
    /// Land along one or two edges, open sea on the rest.
    Coastline,
}

/// A climate preset: base temperature, base moisture and colour palette.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Climate {
    #[default]
    Temperate,
    Frozen,
    Arid,
    Tropical,
    Volcanic,
}

/// How much of a world to generate. `History` arrives in Phase 14c.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum GenerationScope {
    #[default]
    MapOnly,
    Places,
    History,
}

/// Everything the generator needs. The same params always give the same map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorParams {
    /// Any 32-bit number. (specta refuses 64-bit integers because JavaScript
    /// numbers cannot hold them exactly, so the seed is a `u32`.)
    pub seed: u32,
    /// Map size in pixels, `256..=16384` per side.
    pub width: u32,
    pub height: u32,
    pub land_shape: LandShape,
    pub climate: Climate,
    /// Share of the map that is land, `10..=90` (%).
    pub land_percent: u8,
    /// Warmer (+) or colder (−) than the preset, `-1.0..=1.0`.
    #[specta(type = specta_typescript::Number)]
    pub temperature: f64,
    /// Wetter (+) or drier (−) than the preset, `-1.0..=1.0`.
    #[specta(type = specta_typescript::Number)]
    pub rainfall: f64,
    /// Number of kingdoms, `1..=12` (used from Phase 14c).
    pub kingdoms: u8,
    pub scope: GenerationScope,
}

impl Default for GeneratorParams {
    fn default() -> Self {
        GeneratorParams {
            seed: 42,
            width: 4000,
            height: 3000,
            land_shape: LandShape::Continent,
            climate: Climate::Temperate,
            land_percent: 40,
            temperature: 0.0,
            rainfall: 0.0,
            kingdoms: 5,
            scope: GenerationScope::MapOnly,
        }
    }
}

impl GeneratorParams {
    /// Checks every field is in range. Returns the first problem found as an
    /// `AppError::Validation` with a message suitable for the wizard.
    pub fn validate(&self) -> AppResult<()> {
        let sizes = MIN_MAP_SIZE..=MAX_MAP_SIZE;
        if !sizes.contains(&self.width) || !sizes.contains(&self.height) {
            return Err(AppError::Validation(format!(
                "Map size must be between {MIN_MAP_SIZE} and {MAX_MAP_SIZE} pixels per side"
            )));
        }
        if !(10..=90).contains(&self.land_percent) {
            return Err(AppError::Validation(
                "Land must be between 10 and 90 percent".into(),
            ));
        }
        if !(1..=12).contains(&self.kingdoms) {
            return Err(AppError::Validation(
                "Number of kingdoms must be between 1 and 12".into(),
            ));
        }
        // `contains` is false for NaN, so this also rejects "not a number".
        let slider = -1.0..=1.0;
        if !slider.contains(&self.temperature) {
            return Err(AppError::Validation(
                "Temperature must be between -1 and 1".into(),
            ));
        }
        if !slider.contains(&self.rainfall) {
            return Err(AppError::Validation(
                "Rainfall must be between -1 and 1".into(),
            ));
        }
        Ok(())
    }
}

/// What covers one cell of the terrain grid. Water biomes come first.
///
/// `PartialOrd`/`Ord` let a `Biome` be a `BTreeMap` key (a sorted map needs
/// to compare its keys); the order is the declaration order below.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type,
)]
#[serde(rename_all = "camelCase")]
pub enum Biome {
    /// Sea connected to the map edge.
    Ocean,
    /// Water enclosed by land (not connected to the map edge).
    Lake,
    /// Ocean or lake water below freezing.
    SeaIce,
    IceSheet,
    Tundra,
    /// Cold coniferous forest.
    Taiga,
    SnowyPeaks,
    Mountain,
    Hills,
    TemperateForest,
    Grassland,
    Desert,
    Savanna,
    Jungle,
    Swamp,
    AshPlain,
    Lava,
}

impl Biome {
    /// True for ocean, lake and sea ice.
    pub fn is_water(self) -> bool {
        matches!(self, Biome::Ocean | Biome::Lake | Biome::SeaIce)
    }

    /// Cold land biomes.
    pub fn is_cold(self) -> bool {
        matches!(
            self,
            Biome::IceSheet | Biome::Tundra | Biome::Taiga | Biome::SnowyPeaks
        )
    }

    /// Dry land biomes.
    pub fn is_dry(self) -> bool {
        matches!(self, Biome::Desert | Biome::Savanna | Biome::AshPlain)
    }
}

/// A summary of what was generated, reported to the frontend when done.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GenerationStats {
    /// Measured land share in percent (0..100).
    #[specta(type = specta_typescript::Number)]
    pub land_percent_actual: f64,
    /// Islands and continents big enough to be traced as vector areas.
    pub land_masses: u32,
    pub rivers: u32,
    pub stamps: u32,
    /// Number of grid cells per biome (water biomes included).
    pub biome_counts: BTreeMap<Biome, u32>,
    /// Settlements placed (0 for the map-only scope).
    pub settlements: u32,
    /// Road objects drawn (0 for the map-only scope).
    pub roads: u32,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_params_are_valid() {
        GeneratorParams::default().validate().unwrap();
    }

    #[test]
    fn rejects_out_of_range_values() {
        let ok = GeneratorParams::default();
        let bad = [
            GeneratorParams {
                width: 255,
                ..ok.clone()
            },
            GeneratorParams {
                height: 16385,
                ..ok.clone()
            },
            GeneratorParams {
                land_percent: 9,
                ..ok.clone()
            },
            GeneratorParams {
                land_percent: 91,
                ..ok.clone()
            },
            GeneratorParams {
                kingdoms: 0,
                ..ok.clone()
            },
            GeneratorParams {
                kingdoms: 13,
                ..ok.clone()
            },
            GeneratorParams {
                temperature: 1.5,
                ..ok.clone()
            },
            GeneratorParams {
                rainfall: -1.01,
                ..ok.clone()
            },
            GeneratorParams {
                rainfall: f64::NAN,
                ..ok.clone()
            },
        ];
        for params in bad {
            let err = params.validate().unwrap_err();
            assert!(matches!(err, AppError::Validation(_)), "{params:?}");
        }
    }

    #[test]
    fn enums_serialise_camel_case() {
        let json = serde_json::to_string(&LandShape::TwinContinents).unwrap();
        assert_eq!(json, "\"twinContinents\"");
        let json = serde_json::to_string(&Biome::SnowyPeaks).unwrap();
        assert_eq!(json, "\"snowyPeaks\"");
        let json = serde_json::to_string(&GenerationScope::MapOnly).unwrap();
        assert_eq!(json, "\"mapOnly\"");
    }
}
