use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;

// A value as it crosses between processes: its encoded bytes, and the file
// holding them once it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedValue {
    pub bytes: Arc<[u8]>,
    pub file: Option<PathBuf>,
}

impl EncodedValue {
    pub fn encoded<T: Serialize>(value: &T) -> serde_json::Result<EncodedValue> {
        Ok(EncodedValue {
            bytes: serde_json::to_vec(value)?.into(),
            file: None,
        })
    }

    pub fn from_file(file: PathBuf) -> io::Result<EncodedValue> {
        Ok(EncodedValue {
            bytes: fs::read(&file)?.into(),
            file: Some(file),
        })
    }

    pub fn decoded<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_slice(&self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::EncodedValue;

    #[test]
    fn a_value_is_held_as_the_bytes_it_crosses_as() {
        let value = EncodedValue::encoded(&vec![1u8, 2, 3]).expect("encodes");
        assert_eq!(&*value.bytes, b"[1,2,3]");
        assert_eq!(value.file, None);
        assert_eq!(value.decoded::<Vec<u8>>().ok(), Some(vec![1, 2, 3]));
    }
}
