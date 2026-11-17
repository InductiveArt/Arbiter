use std::any::Any;
use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::value::encoded_value::EncodedValue;
use crate::value::value_type::ValueType;

// Whether an output, read from its value, is one the cache names.
type AcceptsFn = Arc<dyn Fn(&EncodedValue) -> Result<bool, String> + Send + Sync>;

// Whether an output once computed for the same inputs is taken from the store
// instead of computed again, and which outputs the cache names: none, every
// output a passed attempt leaves, or only those a test on the output accepts,
// so an output it refuses is computed again on the next run.
#[derive(Clone)]
pub enum Caching {
    Never,
    Every,
    Accepted(CacheTest),
}

// A test on a task's output deciding whether the cache names it: the type it
// reads, which the plan holds to the type the task leaves, and the test.
#[derive(Clone)]
pub struct CacheTest {
    pub output: ValueType,
    accepts: AcceptsFn,
}

impl CacheTest {
    pub fn of<T: Any + DeserializeOwned>(
        accepts: impl Fn(&T) -> bool + Send + Sync + 'static,
    ) -> CacheTest {
        CacheTest {
            output: ValueType::of::<T>(),
            accepts: Arc::new(move |output: &EncodedValue| {
                output
                    .decoded::<T>()
                    .map(|output| accepts(&output))
                    .map_err(|error| error.to_string())
            }),
        }
    }
}

impl Caching {
    // Whether an attempt of the task is looked up in the cache before it runs.
    pub fn replays(&self) -> bool {
        !matches!(self, Caching::Never)
    }

    // Whether the cache names the output, or why the output does not read as
    // the type the test reads.
    pub(crate) fn names(&self, output: &EncodedValue) -> Result<bool, String> {
        match self {
            Caching::Never => Ok(false),
            Caching::Every => Ok(true),
            Caching::Accepted(test) => (test.accepts)(output),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CacheTest;
    use super::Caching;
    use crate::value::encoded_value::EncodedValue;
    use crate::value::value_type::ValueType;

    fn encoded(number: u8) -> EncodedValue {
        EncodedValue::encoded(&number).expect("encodes")
    }

    #[test]
    fn a_task_computed_every_run_is_never_looked_up_and_names_no_output() {
        assert!(!Caching::Never.replays());
        assert_eq!(Caching::Never.names(&encoded(1)), Ok(false));
    }

    #[test]
    fn a_task_caching_every_output_names_each_one_it_is_handed() {
        assert!(Caching::Every.replays());
        assert_eq!(Caching::Every.names(&encoded(1)), Ok(true));
    }

    #[test]
    fn a_cache_test_names_the_outputs_it_accepts_and_no_other() {
        let even = Caching::Accepted(CacheTest::of(|number: &u8| number.is_multiple_of(2)));
        assert!(even.replays(), "an output it accepted is replayed");
        assert_eq!(even.names(&encoded(2)), Ok(true));
        assert_eq!(even.names(&encoded(3)), Ok(false));
    }

    #[test]
    fn a_cache_test_names_the_type_it_reads_and_refuses_a_value_of_another() {
        let test = CacheTest::of(|number: &u8| *number == 0);
        assert_eq!(test.output, ValueType::of::<u8>());
        let refused = Caching::Accepted(test)
            .names(&EncodedValue::encoded(&"zero").expect("encodes"))
            .expect_err("a string is not a u8");
        assert!(refused.starts_with("invalid type"), "{refused}");
    }
}
