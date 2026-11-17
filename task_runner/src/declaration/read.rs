use std::any::Any;

use serde::Deserialize;
use serde::Serialize;

use crate::record::completion::Completion;
use crate::value::encoded_value::EncodedValue;
use crate::value::value_type::ValueType;

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
    pub value_type: ValueType,
    pub when: When,
}

impl Read {
    pub fn of<T: Any>(when: When) -> Read {
        Read {
            value_type: ValueType::of::<T>(),
            when,
        }
    }
}

// What one read is handed: the value for a read that fires on passing, the
// completion for one fired by failing or finishing, both for a finishing read
// whose producer left a value.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadInput {
    pub(crate) value: Option<EncodedValue>,
    pub(crate) completion: Option<Completion>,
}
