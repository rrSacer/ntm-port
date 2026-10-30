//! The server's storage of the world's state ([`crate::persist`]): the
//! native host's, tests a map.

pub trait Kv {
    fn get(&mut self, key: &str) -> Option<Vec<u8>>;
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String>;
    fn delete(&mut self, key: &str);
}

/// An in-memory [`Kv`] (tests).
#[derive(Clone, Debug, Default)]
pub struct MemKv {
    pub map: std::collections::BTreeMap<String, Vec<u8>>,
    /// Bytes written so far: what the one-call budget would count.
    pub written: u64,
    /// Keys whose writes fail (a full folder, an I/O error).
    pub refuse: std::collections::BTreeSet<String>,
}

impl Kv for MemKv {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.map.get(key).cloned()
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        if self.refuse.contains(key) {
            return Err(format!("{key}: refused"));
        }
        if !self.map.contains_key(key) && self.map.len() >= 64 {
            return Err("would hold over 64 keys".into());
        }
        self.written += value.len() as u64;
        self.map.insert(key.into(), value.to_vec());
        Ok(())
    }
    fn delete(&mut self, key: &str) {
        self.map.remove(key);
    }
}

