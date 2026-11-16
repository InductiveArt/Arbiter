use std::any::Any;
use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::value::encoded_value::EncodedValue;
use crate::value::value_type::ValueType;

// The key an item goes by, read from the item's value.
type ItemKeyFn = Arc<dyn Fn(&EncodedValue) -> Result<String, String> + Send + Sync>;

// A task run once for each item of a list, its outputs gathered into a list in
// the same order: the task, the item it reads, the list that item is taken
// from, what one run leaves, the list those are gathered into, and the key
// each item goes by in the record, so an item keeps its name however the
// list around it changes.
#[derive(Clone)]
pub struct FanOut {
    pub task: String,
    pub item: ValueType,
    pub list: ValueType,
    pub output: ValueType,
    pub gathered: ValueType,
    key: ItemKeyFn,
}

impl FanOut {
    pub fn of<T: Any + DeserializeOwned, U: Any>(
        task: &str,
        key: impl Fn(&T) -> String + Send + Sync + 'static,
    ) -> FanOut {
        FanOut {
            task: task.to_string(),
            item: ValueType::of::<T>(),
            list: ValueType::of::<Vec<T>>(),
            output: ValueType::of::<U>(),
            gathered: ValueType::of::<Vec<U>>(),
            key: Arc::new(move |item: &EncodedValue| {
                item.decoded::<T>()
                    .map(|item| key(&item))
                    .map_err(|error| error.to_string())
            }),
        }
    }

    // The key an item goes by, or why the item does not read as one.
    pub(crate) fn key_of(&self, item: &EncodedValue) -> Result<String, String> {
        (self.key)(item)
    }
}
