use crate::value::sha256::sha256_hex;

// What the code computing outputs is: the SHA-256 of the task executable's
// bytes, so a rebuilt executable computes everything again.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identity(String);

impl Identity {
    pub fn of(bytes: &[u8]) -> Identity {
        Identity(sha256_hex(bytes))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
