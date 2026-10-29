//! Capability `storage` as the engines see it: the mods pass their
//! `horizon_mod_sdk::storage`, tests a map.

pub trait Kv {
    fn get(&mut self, key: &str) -> Option<Vec<u8>>;
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String>;
    fn delete(&mut self, key: &str);
}

/// The keys of the shared `ntmlogic` folder (64 at most, 4 MiB: docs/fundacao.md).
pub mod keys {
    /// Client to server mailbox.
    pub const C2S: &str = "c2s";
    /// Server to client mailbox.
    pub const S2C: &str = "s2c";
    /// The worlds this installation has saved, and a boot counter.
    pub const WORLDS: &str = "worlds";
    /// The last world save written, whole: survives a torn write of `w.<wid>`.
    pub const JOURNAL: &str = "journal";
    /// One world's machines.
    pub fn world(wid: &str) -> String {
        format!("w.{wid}")
    }
}

/// An in-memory [`Kv`] (tests, and the client and server engines' tests
/// share one, as single player's two mods share a folder).
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

/// Lets two engines share one [`Kv`] in a test.
impl<T: Kv> Kv for std::rc::Rc<std::cell::RefCell<T>> {
    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.borrow_mut().get(key)
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), String> {
        self.borrow_mut().set(key, value)
    }
    fn delete(&mut self, key: &str) {
        self.borrow_mut().delete(key)
    }
}
