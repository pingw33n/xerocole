use std::collections::HashMap;

use crate::value::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    fields: HashMap<String, Value>,
    tags: HashMap<String, Value>,
}

impl Event {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            tags: HashMap::new(),
        }
    }

    pub fn fields(&self) -> &HashMap<String, Value> {
        &self.fields
    }

    pub fn fields_mut(&mut self) -> &mut HashMap<String, Value> {
        &mut self.fields
    }
}