// 128-bit FNV-1a: stable across runs and platforms, fast, and wide enough for
// a local cache whose adversary is chance, never an attacker.
pub struct Digest(u128);

const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
const PRIME: u128 = 0x0000000001000000000000000000013b;

impl Digest {
    pub fn new() -> Self {
        Digest(OFFSET)
    }

    pub fn eat(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u128::from(*byte);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    pub fn hex(&self) -> String {
        format!("{:032x}", self.0)
    }
}

impl Default for Digest {
    fn default() -> Self {
        Self::new()
    }
}
