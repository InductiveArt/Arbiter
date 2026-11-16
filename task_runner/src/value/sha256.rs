use std::fmt::Write;

// SHA-256 as FIPS 180-4 defines it, fed in pieces of any size.
pub struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    filled: usize,
    length: u64,
}

// The first 32 bits of the fractional parts of the square roots of the first
// 8 primes, and of the cube roots of the first 64.
const START: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];
const ROUNDS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    pub fn new() -> Sha256 {
        Sha256 {
            state: START,
            block: [0; 64],
            filled: 0,
            length: 0,
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        self.absorb(bytes);
    }

    // The message padded with one bit, then zeros, then its length in bits,
    // to a whole number of blocks.
    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.length.wrapping_mul(8);
        let zeros = if self.filled < 56 {
            55 - self.filled
        } else {
            119 - self.filled
        };
        let mut padding = [0u8; 64];
        padding[0] = 0x80;
        self.absorb(&padding[..1 + zeros]);
        self.absorb(&bits.to_be_bytes());
        let mut digest = [0u8; 32];
        for (bytes, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *bytes = word.to_be_bytes();
        }
        digest
    }

    fn absorb(&mut self, mut bytes: &[u8]) {
        if self.filled > 0 {
            let taken = (64 - self.filled).min(bytes.len());
            self.block[self.filled..self.filled + taken].copy_from_slice(&bytes[..taken]);
            self.filled += taken;
            bytes = &bytes[taken..];
            if self.filled < 64 {
                return;
            }
            let block = self.block;
            compress(&mut self.state, &block);
            self.filled = 0;
        }
        let (blocks, rest) = bytes.as_chunks::<64>();
        for block in blocks {
            compress(&mut self.state, block);
        }
        self.block[..rest.len()].copy_from_slice(rest);
        self.filled = rest.len();
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Sha256::new()
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0u32; 64];
    for (word, bytes) in schedule.iter_mut().zip(block.as_chunks::<4>().0) {
        *word = u32::from_be_bytes(*bytes);
    }
    for at in 16..64 {
        let early = schedule[at - 15];
        let late = schedule[at - 2];
        let s0 = early.rotate_right(7) ^ early.rotate_right(18) ^ (early >> 3);
        let s1 = late.rotate_right(17) ^ late.rotate_right(19) ^ (late >> 10);
        schedule[at] = schedule[at - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[at - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for (round, word) in ROUNDS.iter().zip(schedule) {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choice = (e & f) ^ (!e & g);
        let first = h
            .wrapping_add(s1)
            .wrapping_add(choice)
            .wrapping_add(*round)
            .wrapping_add(word);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let second = s0.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    for (word, worked) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(worked);
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut sha = Sha256::new();
    sha.update(bytes);
    hex(&sha.finish())
}

pub fn hex(digest: &[u8; 32]) -> String {
    let mut text = String::with_capacity(64);
    for byte in digest {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

// Whether a text is a digest as `hex` writes one.
pub fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::is_sha256_hex;
    use super::sha256_hex;
    use super::Sha256;

    #[test]
    fn the_digests_match_the_examples_of_the_standard() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            sha256_hex(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn bytes_fed_in_pieces_of_any_size_hash_as_when_fed_whole() {
        let bytes: Vec<u8> = (0..1000u32).map(|n| (n * 7 % 251) as u8).collect();
        let whole = sha256_hex(&bytes);
        for size in [1, 3, 55, 56, 63, 64, 65, 127, 128, 999] {
            let mut sha = Sha256::new();
            for piece in bytes.chunks(size) {
                sha.update(piece);
            }
            assert_eq!(
                super::hex(&sha.finish()),
                whole,
                "fed {size} bytes at a time"
            );
        }
    }

    #[test]
    fn a_digest_in_hex_is_told_from_any_other_text() {
        assert!(is_sha256_hex(&sha256_hex(b"abc")));
        assert!(!is_sha256_hex("abc"));
        assert!(!is_sha256_hex(&sha256_hex(b"abc").to_uppercase()));
        assert!(!is_sha256_hex(&format!("{}g", &sha256_hex(b"abc")[1..])));
    }
}
