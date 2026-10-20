//! Seeded random numbers.
//!
//! Every stage of the generator gets its OWN random stream, derived from the
//! world seed and a stage id. That way adding a random call to, say, the
//! stamp scatter cannot change the coastline: each stage's numbers depend
//! only on the seed and that stage.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// The independent random streams used by the generator.
#[derive(Debug, Clone, Copy)]
pub enum Stage {
    /// Seeds for the noise functions.
    Noise = 1,
    /// Positions of continents and islands.
    Shape = 2,
    /// River meanders.
    Rivers = 3,
    /// Stamp positions, icons, sizes and rotations.
    Scatter = 4,
}

/// A reproducible generator for one stage. ChaCha8 is a fast, high-quality
/// generator whose output is identical on every platform and Rust version,
/// which is exactly what "same seed, same world" needs.
pub fn stage_rng(seed: u32, stage: Stage) -> ChaCha8Rng {
    // `u64::from` widens without loss; the stage id goes in the high bits so
    // the streams of different stages never coincide.
    let mixed = u64::from(seed) | ((stage as u64) << 32);
    ChaCha8Rng::seed_from_u64(mixed)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    // `RngExt` adds convenience methods such as `random_range` to any
    // generator. A trait's methods are only callable when the trait is in
    // scope, hence this `use`.
    use rand::RngExt;

    #[test]
    fn same_seed_same_numbers_and_stages_differ() {
        let a: Vec<u32> = (0..5)
            .map(|_| stage_rng(7, Stage::Noise).random_range(0..1000))
            .collect();
        let mut r1 = stage_rng(7, Stage::Noise);
        let mut r2 = stage_rng(7, Stage::Noise);
        let mut r3 = stage_rng(7, Stage::Scatter);
        let x: Vec<u32> = (0..8).map(|_| r1.random_range(0..u32::MAX)).collect();
        let y: Vec<u32> = (0..8).map(|_| r2.random_range(0..u32::MAX)).collect();
        let z: Vec<u32> = (0..8).map(|_| r3.random_range(0..u32::MAX)).collect();
        assert_eq!(x, y);
        assert_ne!(x, z);
        assert!(a.iter().all(|v| *v == a[0]));
    }
}
