use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::eval::evaluator::EvalResult;
use crate::object::{Object, RuntimeError};
use crate::object::builtins::lookup;

pub mod evaluator;
mod test;

type Env = Rc<RefCell<Environment>>;
type Val = Rc<RefCell<Object>>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Environment {
    store: HashMap<String, Val>,
    outer: Option<Env>,
}

impl Environment {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn extend(outer: Env) -> Self {
        Environment {
            store: HashMap::new(),
            outer: Some(outer),
        }
    }
    pub fn get(&self, key: &str) -> Option<Val> {
        match self.store.get(key) {
            Some(v) => Some(v.clone()),
            None => self
                .outer
                .as_ref()
                .and_then(|outer| outer.borrow().get(key)),
        }
    }

    pub fn set(&mut self, key: &str, val: Object) -> EvalResult<()> {
        if lookup(key).is_some() {
            Err(RuntimeError::VariableHasBeenDeclared(key.to_string()))
        } else {
            self.store
                .insert(key.to_string(), Rc::new(RefCell::new(val)));
            Ok(())
        }
    }
    pub fn contains(&self, key: &str) -> bool {
        if self.store.contains_key(key) {
            true
        } else {
            match &self.outer {
                Some(out) => out.borrow().contains(key),
                None => false,
            }
        }
    }

    pub fn keys(&self) -> Vec<String> {
        let keys = self.store.keys();
        keys.cloned().collect()
    }
}
