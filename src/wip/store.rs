use std::any::type_name;
use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

use crate::wip::job::id::JobId;
use crate::wip::trace::outcome::Outcome;

// One type asked of the store, carrying its name so a gap is reported by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Need {
    pub id: TypeId,
    pub name: &'static str,
}

impl Need {
    pub fn of<T: Any>() -> Need {
        Need {
            id: TypeId::of::<T>(),
            name: type_name::<T>(),
        }
    }
}

// When a read fires, against how the producer of its type finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum When {
    Passed,
    Failed,
    Finished,
}

// One read a task declares: the type, and when the read fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Read {
    pub need: Need,
    pub when: When,
}

impl Read {
    pub fn of<T: Any>(when: When) -> Read {
        Read {
            need: Need::of::<T>(),
            when,
        }
    }
}

// A value as it crosses between processes: its encoded bytes, and the file
// holding them once it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub bytes: Arc<[u8]>,
    pub file: Option<PathBuf>,
}

impl Product {
    pub fn encoded<T: Serialize>(value: &T) -> serde_json::Result<Product> {
        Ok(Product {
            bytes: serde_json::to_vec(value)?.into(),
            file: None,
        })
    }

    pub fn from_file(file: PathBuf) -> io::Result<Product> {
        Ok(Product {
            bytes: fs::read(&file)?.into(),
            file: Some(file),
        })
    }

    pub fn decoded<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_slice(&self.bytes)
    }
}

// How the producer of one type finished: its last job and its outcome. What a
// read fired by failing or finishing is handed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    pub job: JobId,
    pub outcome: Outcome,
}

// What one read is handed: the value for a read that fires on passing, the
// completion for one fired by failing or finishing, both for a finishing read
// whose producer left a value.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub value: Option<Product>,
    pub completion: Option<Completion>,
}

// Values held by type, one per type, and how the producer of each finished.
// A value is encoded bytes, never a live object, so it crosses to the process
// that reads it unchanged.
#[derive(Default)]
pub struct Store {
    items: HashMap<TypeId, Product>,
    completions: HashMap<TypeId, Completion>,
}

impl Store {
    pub fn new() -> Self {
        Store::default()
    }

    pub fn put<T: Serialize + Any>(&mut self, value: &T) -> serde_json::Result<()> {
        self.items
            .insert(TypeId::of::<T>(), Product::encoded(value)?);
        Ok(())
    }

    // Writes every value not yet on disk into the folder, so a process reading
    // it is handed a file.
    pub fn place(&mut self, dir: &Path) -> io::Result<()> {
        let mut unfiled: Vec<&mut Product> = self
            .items
            .values_mut()
            .filter(|product| product.file.is_none())
            .collect();
        if unfiled.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(dir)?;
        for (n, product) in unfiled.iter_mut().enumerate() {
            let file = dir.join(format!("{n}.json"));
            fs::write(&file, &product.bytes)?;
            product.file = Some(file);
        }
        Ok(())
    }

    // Records how a type's producer finished; a value it left is held too.
    pub fn finish(&mut self, id: TypeId, completion: Completion, value: Option<Product>) {
        if let Some(value) = value {
            self.items.insert(id, value);
        }
        self.completions.insert(id, completion);
    }

    pub fn has(&self, id: TypeId) -> bool {
        self.items.contains_key(&id)
    }

    // The value held for a type, decoded; nothing when none is held or the
    // bytes do not decode as the type.
    pub fn get<T: DeserializeOwned + Any>(&self) -> Option<T> {
        self.items
            .get(&TypeId::of::<T>())
            .and_then(|product| product.decoded().ok())
    }

    // What a reader's outcome already is, from how its producers finished, or
    // nothing when every read fires and it has to run. A skipped producer
    // skips the reader before a branch not taken is counted, and both carry
    // the path of the task that decided them. A seeded type, having no
    // producer, always fires. Asked once every producer has finished.
    pub fn settled(&self, reads: &[Read]) -> Option<Outcome> {
        let mut unneeded: Option<String> = None;
        for read in reads {
            let Some(completion) = self.completions.get(&read.need.id) else {
                continue;
            };
            if read.when == When::Finished {
                continue;
            }
            match &completion.outcome {
                Outcome::Skipped(because) => return Some(Outcome::Skipped(because.clone())),
                Outcome::Unneeded(because) => {
                    unneeded.get_or_insert_with(|| because.clone());
                }
                outcome if read.when == When::Passed && outcome.failed() => {
                    return Some(Outcome::Skipped(completion.job.path.clone()));
                }
                Outcome::Ok if read.when == When::Failed => {
                    unneeded.get_or_insert_with(|| completion.job.path.clone());
                }
                _ => {}
            }
        }
        unneeded.map(Outcome::Unneeded)
    }

    // The parts the reads name, in the reads' order. Asked only once the
    // reader is to run, so every part a read needs is present.
    pub fn pull(&self, reads: &[Read]) -> Vec<Part> {
        reads
            .iter()
            .map(|read| {
                let value = self.items.get(&read.need.id).cloned();
                let completion = self.completions.get(&read.need.id).cloned();
                match read.when {
                    When::Passed => Part {
                        value,
                        completion: None,
                    },
                    When::Failed => Part {
                        value: None,
                        completion,
                    },
                    When::Finished => Part { value, completion },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::env;
    use std::fs;

    use super::Completion;
    use super::Product;
    use super::Read;
    use super::Store;
    use super::When;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::trace::outcome::Outcome;

    struct A;
    struct B;
    struct C;
    struct Seed;

    fn finished<T: 'static>(store: &mut Store, path: &str, outcome: Outcome) {
        store.finish(
            TypeId::of::<T>(),
            Completion {
                job: JobId::new(RunId::next(), path, 1),
                outcome,
            },
            None,
        );
    }

    #[test]
    fn a_value_is_read_back_by_its_type() {
        let mut store = Store::new();
        store.put(&3u8).expect("encodes");
        store.put(&"x".to_string()).expect("encodes");
        assert_eq!(store.get::<u8>(), Some(3));
        assert_eq!(store.get::<String>(), Some("x".to_string()));
    }

    #[test]
    fn a_type_never_put_reads_as_nothing() {
        let store = Store::new();
        assert_eq!(store.get::<u8>(), None);
    }

    #[test]
    fn a_value_is_held_as_the_bytes_it_crosses_as() {
        let mut store = Store::new();
        store.put(&vec![1u8, 2, 3]).expect("encodes");
        let pulled = store.pull(&[Read::of::<Vec<u8>>(When::Passed)]);
        let product = pulled[0]
            .value
            .as_ref()
            .expect("a value read is handed a value");
        assert_eq!(&*product.bytes, b"[1,2,3]");
        assert_eq!(product.file, None);
        assert_eq!(product.decoded::<Vec<u8>>().ok(), Some(vec![1, 2, 3]));
    }

    #[test]
    fn placing_a_store_writes_each_value_once_into_the_folder() {
        let dir = env::temp_dir().join(format!("arbiter-store-{}", RunId::next()));
        let mut store = Store::new();
        store.put(&7u8).expect("encodes");
        store.place(&dir).expect("writes");
        let file = store.pull(&[Read::of::<u8>(When::Passed)])[0]
            .value
            .clone()
            .and_then(|product| product.file)
            .expect("placed");
        assert_eq!(fs::read(&file).expect("readable"), b"7");
        store.place(&dir).expect("writes nothing more");
        let again = store.pull(&[Read::of::<u8>(When::Passed)])[0]
            .value
            .clone()
            .and_then(|product| product.file);
        assert_eq!(again, Some(file));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pulled_values_come_in_the_reads_order() {
        let mut store = Store::new();
        store.put(&3u8).expect("encodes");
        store.put(&"x".to_string()).expect("encodes");
        let pulled = store.pull(&[
            Read::of::<String>(When::Passed),
            Read::of::<u8>(When::Passed),
        ]);
        let first: Option<String> = pulled[0].value.as_ref().and_then(|p| p.decoded().ok());
        let second: Option<u8> = pulled[1].value.as_ref().and_then(|p| p.decoded().ok());
        assert_eq!((first.as_deref(), second), (Some("x"), Some(3)));
    }

    #[test]
    fn a_read_fired_by_failing_or_finishing_pulls_the_completion_not_only_a_value() {
        let mut store = Store::new();
        store.finish(
            TypeId::of::<A>(),
            Completion {
                job: JobId::new(RunId::next(), "p/a", 1),
                outcome: Outcome::Ok,
            },
            Some(Product::encoded(&()).expect("encodes")),
        );
        let pulled = store.pull(&[
            Read::of::<A>(When::Passed),
            Read::of::<A>(When::Failed),
            Read::of::<A>(When::Finished),
        ]);
        assert!(pulled[0].value.is_some() && pulled[0].completion.is_none());
        assert!(pulled[1].value.is_none() && pulled[1].completion.is_some());
        let finishing = &pulled[2];
        assert!(finishing.value.is_some());
        let completion = finishing
            .completion
            .as_ref()
            .expect("a finishing read is handed the completion");
        assert_eq!(completion.job.path, "p/a");
        assert_eq!(completion.outcome, Outcome::Ok);
        assert!(store.has(TypeId::of::<A>()), "the value left is held too");
    }

    #[test]
    fn a_reader_whose_every_read_fires_has_to_run() {
        let mut store = Store::new();
        store.put(&()).expect("encodes");
        finished::<A>(&mut store, "p/a", Outcome::Ok);
        finished::<B>(&mut store, "p/b", Outcome::Failed("broke".to_string()));
        let settled = store.settled(&[
            Read::of::<Seed>(When::Passed),
            Read::of::<A>(When::Passed),
            Read::of::<B>(When::Failed),
            Read::of::<B>(When::Finished),
        ]);
        assert_eq!(settled, None);
    }

    #[test]
    fn a_failure_read_of_a_task_that_passed_is_unneeded_and_names_that_task() {
        let mut store = Store::new();
        finished::<A>(&mut store, "p/a", Outcome::Ok);
        let settled = store.settled(&[Read::of::<A>(When::Failed)]);
        assert_eq!(settled, Some(Outcome::Unneeded("p/a".to_string())));
    }

    #[test]
    fn a_value_read_of_a_branch_not_taken_is_unneeded_for_the_same_reason() {
        let mut store = Store::new();
        finished::<A>(&mut store, "p/a", Outcome::Unneeded("p/check".to_string()));
        let settled = store.settled(&[Read::of::<A>(When::Passed)]);
        assert_eq!(settled, Some(Outcome::Unneeded("p/check".to_string())));
    }

    #[test]
    fn a_skipped_producer_skips_its_reader_even_beside_a_branch_not_taken() {
        let mut store = Store::new();
        finished::<A>(&mut store, "p/a", Outcome::Ok);
        finished::<B>(&mut store, "p/b", Outcome::Skipped("p/x".to_string()));
        let settled = store.settled(&[Read::of::<A>(When::Failed), Read::of::<B>(When::Passed)]);
        assert_eq!(settled, Some(Outcome::Skipped("p/x".to_string())));
    }

    #[test]
    fn a_finishing_read_fires_whatever_the_producer_s_outcome() {
        let mut store = Store::new();
        finished::<A>(&mut store, "p/a", Outcome::Failed("broke".to_string()));
        finished::<B>(&mut store, "p/b", Outcome::Skipped("p/a".to_string()));
        finished::<C>(&mut store, "p/c", Outcome::Unneeded("p/a".to_string()));
        let settled = store.settled(&[
            Read::of::<A>(When::Finished),
            Read::of::<B>(When::Finished),
            Read::of::<C>(When::Finished),
        ]);
        assert_eq!(settled, None);
    }

    #[test]
    fn a_timed_out_or_cancelled_producer_skips_its_value_readers_and_fires_its_failure_readers() {
        let mut store = Store::new();
        finished::<A>(&mut store, "p/a", Outcome::TimedOut);
        finished::<B>(&mut store, "p/b", Outcome::Cancelled);
        assert_eq!(
            store.settled(&[Read::of::<A>(When::Passed)]),
            Some(Outcome::Skipped("p/a".to_string()))
        );
        assert_eq!(
            store.settled(&[Read::of::<B>(When::Passed)]),
            Some(Outcome::Skipped("p/b".to_string()))
        );
        assert_eq!(
            store.settled(&[Read::of::<A>(When::Failed), Read::of::<B>(When::Failed)]),
            None
        );
    }
}
