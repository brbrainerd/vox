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
        let map = std::mem::take(&mut self.prev);
        let keys: Vec<String> = map.keys().cloned().collect();
        for (k, prev) in map {
            match prev {
                Some(v) => unsafe {
                    env::set_var(&k, v);
                },
                None => unsafe {
                    env::remove_var(&k);
                },
            }
        }
        // Invalidate snapshot caches so any accessor that read a key we mutated
        // will re-read from the now-restored environment on the next call.
        if !keys.is_empty() {
            let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
            vox_config::snapshot::bump(&key_refs);
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
