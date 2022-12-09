// Credits to the scrtlabs raffle contract : https://github.com/scrtlabs/secret-raffle/blob/master/src/rand.rs

use rand_chacha::ChaChaRng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};

pub struct Prng {
    seed: Vec<u8>,
    pos: u128,
}

impl Prng {
    pub fn new(seed: &[u8]) -> Self {
        Self {
            seed: seed.to_vec(),
            pos: 0,
        }
    }

    /// Return a random number (inclusive) between `from` and `to`
    /// This is not the best algorithm but ut's good enough
    pub fn random_between(&mut self, from: u32, to: u32) -> u32 {
        if from > to {
            return 0;
        }
        let x = self.rand_u32();
        let interval_length = to - from + 1;

        // The algorithm as it is now is not perfect, it can become biased for a large number of bought tickets
        // We don't think a large number of tickets will be purchased (compared to u32::max_value)
        // To make the algorithm better, we could do somehting along the lines of :
        // (comes from https://stackoverflow.com/questions/10984974/why-do-people-say-there-is-modulo-bias-when-using-a-random-number-generator)
        /*
            let u32_max = u32::max_value();
            let limit = u32_max - u32_max % interval_length;
            let x = loop {
                let x = self.rand_u32();
                if x < limit{
                    break x;
                }
            };
        */

        from + (x % interval_length)
    }

    fn rand_u32(&mut self) -> u32 {
        let mut hasher = Sha256::new();

        // write input message
        hasher.update(&self.seed);
        let hash = hasher.finalize();

        let mut result = [0u8; 32];
        result.copy_from_slice(hash.as_slice());

        let mut rng: ChaChaRng = ChaChaRng::from_seed(result);

        rng.set_word_pos(self.pos);
        self.pos += 8;

        rng.next_u32()
    }
}
