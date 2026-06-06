use super::value::VoxValue;
use std::collections::HashMap;
use std::rc::Rc;

/// A lexical scope: a stack of frames. Each frame is `Rc`-shared so that
/// cloning a `Scope` — which the interpreter does on every closure capture and
/// (via `Fn.env`) every closure application — is O(number-of-frames) refcount
/// bumps instead of a deep clone of every variable binding. Writes use
/// [`Rc::make_mut`] (copy-on-write): a frame is cloned only when it is shared
/// with a still-live closure capture, preserving value semantics.
#[derive(Debug, Clone)]
pub struct Scope {
    frames: Vec<Rc<HashMap<String, VoxValue>>>,
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Scope {
    pub fn new() -> Self {
        Self {
            frames: vec![Rc::new(HashMap::new())],
        }
    }

    pub fn push_frame(&mut self) {
        self.frames.push(Rc::new(HashMap::new()));
    }

    pub fn pop_frame(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }
    }

    pub fn get(&self, name: &str) -> Option<&VoxValue> {
        for frame in self.frames.iter().rev() {
            if let Some(val) = frame.get(name) {
                return Some(val);
            }
        }
        None
    }

    /// Mutable access to a binding, searching inner→outer frames. Used by the
    /// copy-on-write mutation paths (`list.push`, `arr[i] = x`, `obj.k = x`) so
    /// they can `Rc::make_mut` the stored payload in place — O(1) amortized when
    /// the binding is the sole owner, clone-once when it is aliased.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut VoxValue> {
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(name) {
                return Rc::make_mut(frame).get_mut(name);
            }
        }
        None
    }

    pub fn set(&mut self, name: String, value: VoxValue) {
        if let Some(frame) = self.frames.last_mut() {
            Rc::make_mut(frame).insert(name, value);
        }
    }

    pub fn set_mut(&mut self, name: &str, value: VoxValue) -> bool {
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(name) {
                Rc::make_mut(frame).insert(name.to_string(), value);
                return true;
            }
        }
        false
    }
}
