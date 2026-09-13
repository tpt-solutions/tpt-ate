//! A tiny deterministic PRNG (xorshift64*) so simulated equipment behavior
//! is fully reproducible from a seed — the same approach `tpt-silicon`'s
//! testbench generator uses. No external RNG dependency needed.

/// Seeded xorshift64* generator.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Seed the generator. A zero seed is remapped to a nonzero state.
    pub fn new(seed: u64) -> Rng {
        Rng { state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed } }
    }

    /// Seed derived from mixed integer parts (e.g. base seed + die
    /// coordinates) so each die's simulation is independent but stable.
    pub fn from_parts(parts: &[u64]) -> Rng {
        let mut h: u64 = 0xCBF2_9CE4_8422_2325; // FNV-1a offset basis
        for part in parts {
            h ^= *part;
            h = h.wrapping_mul(0x0000_0100_0000_01B3); // FNV-1a prime
        }
        Rng::new(h)
    }

    /// Next raw 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform float in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // Take the top 24 bits for a uniform mantissa in [2^-24, 1).
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A value uniformly in `[lo, hi)`.
    pub fn uniform(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_from_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn uniform_in_range_and_deterministic_from_parts() {
        let mut rng = Rng::from_parts(&[7, 100, 200]);
        let first = rng.uniform(-1.0, 1.0);
        assert!((-1.0..1.0).contains(&first));
        let again = Rng::from_parts(&[7, 100, 200]).uniform(-1.0, 1.0);
        assert_eq!(first, again);
    }

    #[test]
    fn zero_seed_is_valid() {
        let mut rng = Rng::new(0);
        assert_ne!(rng.next_u64(), 0);
    }
}
