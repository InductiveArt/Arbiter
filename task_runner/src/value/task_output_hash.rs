use crate::value::sha256::is_sha256_hex;
use crate::value::sha256::sha256_hex;

// What names a task's output: the SHA-256 of its bytes, so the same output
// always goes by the same name and a kept file is checked against it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskOutputHash(String);

impl TaskOutputHash {
    pub fn of(bytes: &[u8]) -> TaskOutputHash {
        TaskOutputHash(sha256_hex(bytes))
    }

    // The hash a text names; nothing for any other text.
    pub fn parse(text: &str) -> Option<TaskOutputHash> {
        is_sha256_hex(text).then(|| TaskOutputHash(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
