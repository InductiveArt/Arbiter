use crate::value::sha256::hex;
use crate::value::sha256::is_sha256_hex;
use crate::value::sha256::Sha256;

// What a task's output is looked up by: the SHA-256 of everything it was
// computed from, so the same code on the same inputs finds it again.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskCacheKey(String);

impl TaskCacheKey {
    // Each part is marked present or absent, and a present one is led by its
    // length, so no two lists of parts share a key.
    pub(crate) fn of(parts: &[Option<&[u8]>]) -> TaskCacheKey {
        let mut sha = Sha256::new();
        for part in parts {
            match part {
                None => sha.update(&[0]),
                Some(bytes) => {
                    sha.update(&[1]);
                    sha.update(&(bytes.len() as u64).to_be_bytes());
                    sha.update(bytes);
                }
            }
        }
        TaskCacheKey(hex(&sha.finish()))
    }

    // The key a text names; nothing for any other text.
    pub fn parse(text: &str) -> Option<TaskCacheKey> {
        is_sha256_hex(text).then(|| TaskCacheKey(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::TaskCacheKey;

    #[test]
    fn parts_that_join_to_the_same_bytes_still_give_different_keys() {
        let split = TaskCacheKey::of(&[Some(b"ab"), Some(b"c")]);
        let moved = TaskCacheKey::of(&[Some(b"a"), Some(b"bc")]);
        let absent = TaskCacheKey::of(&[None, Some(b"abc")]);
        let empty = TaskCacheKey::of(&[Some(b""), Some(b"abc")]);
        assert_ne!(split, moved);
        assert_ne!(absent, empty);
        assert_eq!(split, TaskCacheKey::of(&[Some(b"ab"), Some(b"c")]));
    }
}
