use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::value::encoded_value::EncodedValue;

// Values held by type, one per type. A value is encoded bytes, never a live
// object, so it crosses to the process that reads it unchanged.
#[derive(Default)]
pub struct RunStore {
    values: HashMap<TypeId, EncodedValue>,
}

impl RunStore {
    pub fn new() -> Self {
        RunStore::default()
    }

    pub fn put<T: Serialize + Any>(&mut self, value: &T) -> serde_json::Result<()> {
        self.values
            .insert(TypeId::of::<T>(), EncodedValue::encoded(value)?);
        Ok(())
    }

    pub(crate) fn insert(&mut self, id: TypeId, value: EncodedValue) {
        self.values.insert(id, value);
    }

    // Writes every value not yet on disk into the folder, so a process reading
    // it is handed a file.
    pub(crate) fn place(&mut self, dir: &Path) -> io::Result<()> {
        let mut unfiled: Vec<&mut EncodedValue> = self
            .values
            .values_mut()
            .filter(|value| value.file.is_none())
            .collect();
        if unfiled.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(dir)?;
        for (n, value) in unfiled.iter_mut().enumerate() {
            let file = dir.join(format!("{n}.json"));
            fs::write(&file, &value.bytes)?;
            value.file = Some(file);
        }
        Ok(())
    }

    pub fn has(&self, id: TypeId) -> bool {
        self.values.contains_key(&id)
    }

    pub(crate) fn value(&self, id: TypeId) -> Option<&EncodedValue> {
        self.values.get(&id)
    }

    // The value held for a type, decoded; nothing when none is held or the
    // bytes do not decode as the type.
    pub fn get<T: DeserializeOwned + Any>(&self) -> Option<T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.decoded().ok())
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::env;
    use std::fs;
    use std::process;

    use super::RunStore;

    #[test]
    fn a_value_is_read_back_by_its_type() {
        let mut store = RunStore::new();
        store.put(&3u8).expect("encodes");
        store.put(&"x".to_string()).expect("encodes");
        assert_eq!(store.get::<u8>(), Some(3));
        assert_eq!(store.get::<String>(), Some("x".to_string()));
    }

    #[test]
    fn a_type_never_put_reads_as_nothing() {
        let store = RunStore::new();
        assert_eq!(store.get::<u8>(), None);
    }

    #[test]
    fn placing_a_store_writes_each_value_once_into_the_folder() {
        let dir = env::temp_dir().join(format!("task-runner-store-{}", process::id()));
        let mut store = RunStore::new();
        store.put(&7u8).expect("encodes");
        store.place(&dir).expect("writes");
        let file = store
            .value(TypeId::of::<u8>())
            .and_then(|value| value.file.clone())
            .expect("placed");
        assert_eq!(fs::read(&file).expect("readable"), b"7");
        store.place(&dir).expect("writes nothing more");
        let again = store
            .value(TypeId::of::<u8>())
            .and_then(|value| value.file.clone());
        assert_eq!(again, Some(file));
        let _ = fs::remove_dir_all(&dir);
    }
}
