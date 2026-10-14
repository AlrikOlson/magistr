//! Available select values and the currently accepted value for one session option.
use alloc::{string::String, vec::Vec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionConfig {
    pub id: String,
    pub name: String,
    pub current_value: String,
    pub values: Vec<(String, String)>,
}
