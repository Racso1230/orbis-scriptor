//! Biome classification from relief, moisture and temperature.

use super::fields::Relief;
use crate::models::generator::{Biome, Climate};

/// Picks the biome of a land node.
///
/// - `elev`: height above sea `0..1`; `relief`: hills/mountain percentile
/// - `m`: moisture `0..1`; `t`: temperature (below 0 = freezing)
/// - `lava`: volcanic noise (only non-zero for the Volcanic climate)
pub fn classify(climate: Climate, relief: Relief, elev: f64, m: f64, t: f64, lava: f64) -> Biome {
    if t < -0.25 {
        return Biome::IceSheet;
    }
    match relief {
        Relief::Mountain => {
            return if t < -0.05 {
                Biome::SnowyPeaks
            } else {
                Biome::Mountain
            };
        }
        Relief::Hills => {
            if climate == Climate::Volcanic && lava > 0.35 {
                return Biome::Lava;
            }
            if t < 0.0 {
                return Biome::Tundra;
            }
            if t >= 0.1 {
                return Biome::Hills;
            }
        }
        Relief::Low => {}
    }
    if climate == Climate::Volcanic && t > 0.0 {
        if lava > 0.3 {
            return Biome::Lava;
        }
        if m < 0.5 {
            return Biome::AshPlain;
        }
    }
    if t < 0.0 {
        return Biome::Tundra;
    }
    if t < 0.15 {
        return if m > 0.3 { Biome::Taiga } else { Biome::Tundra };
    }
    if m > 0.8 && elev < 0.1 && t > 0.3 {
        return Biome::Swamp;
    }
    if t < 0.62 {
        if m < 0.22 {
            Biome::Desert
        } else if m < 0.45 {
            Biome::Grassland
        } else {
            Biome::TemperateForest
        }
    } else if m < 0.3 {
        Biome::Desert
    } else if m < 0.52 {
        Biome::Savanna
    } else {
        Biome::Jungle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_temperature_and_moisture() {
        let c = Climate::Temperate;
        assert_eq!(
            classify(c, Relief::Low, 0.2, 0.5, -0.5, 0.0),
            Biome::IceSheet
        );
        assert_eq!(
            classify(c, Relief::Mountain, 0.9, 0.5, -0.1, 0.0),
            Biome::SnowyPeaks
        );
        assert_eq!(
            classify(c, Relief::Mountain, 0.7, 0.5, 0.3, 0.0),
            Biome::Mountain
        );
        assert_eq!(classify(c, Relief::Low, 0.2, 0.6, 0.1, 0.0), Biome::Taiga);
        assert_eq!(
            classify(c, Relief::Low, 0.2, 0.6, 0.4, 0.0),
            Biome::TemperateForest
        );
        assert_eq!(classify(c, Relief::Low, 0.2, 0.1, 0.8, 0.0), Biome::Desert);
        assert_eq!(classify(c, Relief::Low, 0.2, 0.7, 0.9, 0.0), Biome::Jungle);
        assert_eq!(classify(c, Relief::Low, 0.05, 0.9, 0.5, 0.0), Biome::Swamp);
        let v = Climate::Volcanic;
        assert_eq!(
            classify(v, Relief::Low, 0.2, 0.3, 0.5, 0.0),
            Biome::AshPlain
        );
        assert_eq!(classify(v, Relief::Low, 0.2, 0.7, 0.5, 0.5), Biome::Lava);
    }
}
