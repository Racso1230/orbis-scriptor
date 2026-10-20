//! Climate presets: temperature/moisture parameters and colour palettes.
//!
//! Temperatures are unitless: `0.0` is freezing, about `1.0` is tropical
//! heat. Moisture runs `0.0` (bone dry) to `1.0` (soaked).

use crate::models::generator::{Biome, Climate};

/// An RGB colour.
pub type Rgb = [u8; 3];

/// The numbers that turn a climate preset into temperature and moisture.
#[derive(Debug, Clone, Copy)]
pub struct ClimateProfile {
    /// Temperature at the equator (map middle) at sea level.
    pub base_temperature: f64,
    /// How much colder the top and bottom edges are (× latitude²).
    pub latitude_drop: f64,
    /// Lapse rate: how much colder the highest peak is than sea level.
    pub lapse_rate: f64,
    /// Added to the moisture noise.
    pub base_moisture: f64,
}

/// Water below this temperature is drawn (and classified) as ice.
pub const SEA_ICE_BELOW: f64 = -0.08;

pub fn profile(climate: Climate) -> ClimateProfile {
    // `match` must handle every variant; the compiler checks that for us,
    // so adding a climate later cannot silently fall through.
    match climate {
        Climate::Temperate => ClimateProfile {
            base_temperature: 0.55,
            latitude_drop: 0.6,
            lapse_rate: 0.75,
            base_moisture: 0.0,
        },
        Climate::Frozen => ClimateProfile {
            base_temperature: 0.12,
            latitude_drop: 0.55,
            lapse_rate: 0.6,
            base_moisture: -0.05,
        },
        Climate::Arid => ClimateProfile {
            base_temperature: 0.78,
            latitude_drop: 0.5,
            lapse_rate: 0.7,
            base_moisture: -0.5,
        },
        Climate::Tropical => ClimateProfile {
            base_temperature: 0.95,
            latitude_drop: 0.45,
            lapse_rate: 0.8,
            base_moisture: 0.25,
        },
        Climate::Volcanic => ClimateProfile {
            base_temperature: 0.55,
            latitude_drop: 0.6,
            lapse_rate: 0.6,
            base_moisture: -0.12,
        },
    }
}

/// Colours used by the background renderer and the vector styles.
#[derive(Debug, Clone)]
pub struct Palette {
    pub deep_water: Rgb,
    pub shallow_water: Rgb,
    pub lake: Rgb,
    pub ice: Rgb,
    pub beach: Rgb,
    pub river: Rgb,
    pub coast_line: Rgb,
}

pub fn palette(climate: Climate) -> Palette {
    match climate {
        Climate::Temperate => Palette {
            deep_water: [28, 62, 104],
            shallow_water: [74, 128, 168],
            lake: [70, 122, 160],
            ice: [232, 240, 246],
            beach: [222, 206, 160],
            river: [62, 112, 158],
            coast_line: [52, 60, 54],
        },
        Climate::Frozen => Palette {
            deep_water: [36, 62, 92],
            shallow_water: [96, 136, 166],
            lake: [120, 156, 182],
            ice: [236, 243, 250],
            beach: [196, 204, 208],
            river: [104, 148, 184],
            coast_line: [70, 86, 102],
        },
        Climate::Arid => Palette {
            deep_water: [26, 72, 102],
            shallow_water: [64, 146, 160],
            lake: [70, 150, 160],
            ice: [232, 240, 246],
            beach: [236, 214, 160],
            river: [58, 128, 150],
            coast_line: [110, 82, 50],
        },
        Climate::Tropical => Palette {
            deep_water: [14, 64, 104],
            shallow_water: [40, 158, 170],
            lake: [44, 130, 140],
            ice: [232, 240, 246],
            beach: [240, 226, 176],
            river: [52, 120, 120],
            coast_line: [30, 66, 42],
        },
        Climate::Volcanic => Palette {
            deep_water: [22, 30, 42],
            shallow_water: [54, 70, 84],
            lake: [60, 72, 84],
            ice: [214, 220, 226],
            beach: [70, 66, 64],
            river: [80, 104, 122],
            coast_line: [20, 18, 18],
        },
    }
}

/// The base colour of a land biome before elevation shading. The climate
/// tints the shared biomes (a Frozen grassland is paler than a Temperate one).
pub fn biome_color(climate: Climate, biome: Biome) -> Rgb {
    let base: Rgb = match biome {
        Biome::Ocean | Biome::Lake | Biome::SeaIce => [80, 120, 160],
        Biome::IceSheet => [238, 244, 250],
        Biome::Tundra => [176, 180, 160],
        Biome::Taiga => [74, 104, 84],
        Biome::SnowyPeaks => [244, 246, 250],
        Biome::Mountain => [140, 124, 106],
        Biome::Hills => [150, 150, 102],
        Biome::TemperateForest => [74, 118, 62],
        Biome::Grassland => [138, 168, 92],
        Biome::Desert => [226, 196, 136],
        Biome::Savanna => [192, 178, 104],
        Biome::Jungle => [36, 104, 52],
        Biome::Swamp => [84, 106, 76],
        Biome::AshPlain => [96, 90, 86],
        Biome::Lava => [170, 52, 26],
    };
    let tint: Rgb = match climate {
        Climate::Temperate => return base,
        Climate::Frozen => [210, 222, 230],
        Climate::Arid => [214, 170, 110],
        Climate::Tropical => [40, 120, 60],
        Climate::Volcanic => [60, 54, 52],
    };
    // Snow, ice and lava keep their own colour; everything else is pulled
    // a little towards the climate's tint.
    match biome {
        Biome::IceSheet | Biome::SnowyPeaks | Biome::Lava => base,
        _ => mix(base, tint, 0.22),
    }
}

/// Linear blend of two colours: `t = 0` gives `a`, `t = 1` gives `b`.
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let ch = |i: usize| (f64::from(a[i]) * (1.0 - t) + f64::from(b[i]) * t).round() as u8;
    [ch(0), ch(1), ch(2)]
}

/// `#rrggbb` for vector styles.
pub fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_mix() {
        assert_eq!(hex([255, 0, 16]), "#ff0010");
        assert_eq!(mix([0, 0, 0], [200, 100, 50], 0.5), [100, 50, 25]);
    }
}
