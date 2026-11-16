use std::any::type_name;
use std::any::Any;
use std::any::TypeId;

// The type of a value, carrying its name so a gap is reported by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueType {
    pub id: TypeId,
    pub name: &'static str,
}

impl ValueType {
    pub fn of<T: Any>() -> ValueType {
        ValueType {
            id: TypeId::of::<T>(),
            name: type_name::<T>(),
        }
    }
}
