//! Deterministic pseudo-random number generation.
//!
//! The generator lives inside the simulation state, so every draw advances
//! world state and is covered by the checksum. Nothing here reads the clock,
//! thread-local state, or the operating system, because two machines
//! replaying the same commands must draw the same numbers.

/// A xoshiro256** generator.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// Builds a generator from a seed.
    ///
    /// The seed is expanded through splitmix64 rather than being written into
    /// the state directly: an all-zero xoshiro state is a fixed point that
    /// emits zeros forever, and low-entropy seeds take many rounds to
    /// scramble.
    pub fn from_seed(seed: u64) -> Rng {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        Rng {
            state: [next(), next(), next(), next()],
        }
    }

    /// Draws the next value and advances the state.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);

        result
    }

    /// Draws a value in `0..bound`.
    ///
    /// Panics if `bound` is zero, since no value satisfies an empty range.
    ///
    /// Uses Lemire's multiply-shift with rejection rather than a modulo, so
    /// the result is unbiased. The rejection loop is deterministic: it
    /// consumes a predictable number of draws for a given state, which a
    /// modulo-with-retry scheme would not guarantee across bounds.
    pub fn below(&mut self, bound: u32) -> u32 {
        assert!(bound > 0, "below(0) has no valid result");
        let bound = bound as u64;
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let draw = self.next_u64() >> 32;
            let product = draw * bound;
            if (product & 0xFFFF_FFFF) >= threshold {
                return (product >> 32) as u32;
            }
        }
    }
}
