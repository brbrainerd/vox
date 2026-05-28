//! Scoped environment variable mutations that restore previous values on [`Drop`].
//!
//! Use this instead of ad hoc `set_var`/`remove_var` pairs that forget teardown.
#![allow(unsafe_code)] // Rust 2024: `std::env::{set_var,remove_var}` are `unsafe`.

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;

pub struct EnvScratch {
    prev: HashMap<String, Option<OsString>>,
}

impl EnvScratch {
    pub fn empty() -> Self {
        Self {
            prev: HashMap::new(),
        }
    }

    fn note_key(&mut self, key: impl AsRef<str>) {
        let key = key.as_ref().to_string();
        self.prev
            .entry(key.clone())
            .or_insert_with(|| env::var_os(&key));
    }

    pub fn set(mut self, key: impl AsRef<str>, val: impl AsRef<str>) -> Self {
        let key = key.as_ref();
        self.note_key(key);
        // SAFETY: `set_var` is `unsafe` on Rust 2024 when other threads may read the environment.
        // Tests using [`EnvScratch`] must run single-threaded or otherwise synchronize env access.
        unsafe {
            env::set_var(key, val.as_ref());
        }
        self
    }

    pub fn remove(mut self, key: impl AsRef<str>) -> Self {
        let key = key.as_ref();
        self.note_key(key);
        unsafe {
            env::remove_var(key);
        }
        self
    }
}

impl Drop for EnvScratch {
    fn drop(&mut self) {
        for (k, prev) in std::mem::take(&mut self.prev) {
            match prev {
                Some(v) => unsafe {
                    env::set_var(&k, v);
                },
                None => unsafe {
                    env::remove_var(&k);
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_after_drop() {
        let key = "VOX_TEST_HARNESS_ENV_SCRATCH_DUMMY";
        unsafe {
            env::remove_var(key);
        }
        {
            let _g = EnvScratch::empty().set(key, "hello");
            assert_eq!(env::var(key).unwrap(), "hello");
        }
        assert!(env::var_os(key).is_none());

        unsafe {
            env::set_var(key, "existing");
        }
        {
            let _g = EnvScratch::empty().set(key, "temp");
            assert_eq!(env::var(key).unwrap(), "temp");
        }
        assert_eq!(env::var(key).unwrap(), "existing");
        unsafe {
            env::remove_var(key);
        }
    }
}
