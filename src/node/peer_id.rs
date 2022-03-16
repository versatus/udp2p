use crate::node::peer_key::Key;
use serde::{Serialize, Deserialize};
use crate::utils::utils::ByteRep;
use crate::impl_ByteRep;
use sha256::digest_bytes;

impl_ByteRep!(for PeerId);

/// A tuple struct containing the hash representation of a 256 bit key
#[derive(Clone, Debug, Hash, Serialize, Deserialize, Eq)]
pub struct PeerId(String);

impl PeerId {

    /// Returns the inner hash string
    pub fn get_id(&self) -> String {
        self.0.clone()
    }

    /// Returns an ID generated with a random 256 bit key
    pub fn rand() -> Self {
        let key = Key::rand();
        PeerId::from_key(&key)
    }

    /// Returns an ID generated with a random 256 bit key within a specific range
    pub fn rand_in_range(idx: usize) -> Self {
        let key = Key::rand_in_range(idx);
        PeerId::from_key(&key)
    }

    /// Returns an ID generated from a 256 bit key passed into the function
    /// 
    /// # Argumetns
    /// 
    /// * key - a 256 bit key represented as a u8 array with 32 u8 bytes
    /// 
    pub fn from_key(key: &Key) -> Self {
        let key_hash = digest_bytes(&key.get_key());
        PeerId(key_hash)
    }

    /// Returns an ID generated from a passed hash string
    /// 
    /// # Arguments
    /// 
    /// * value - a hashstring representation of a 256 bit key
    pub fn from_str(value: String) -> Self {
        serde_json::from_str(&value).unwrap()
    }
}


impl PartialEq for PeerId {
    fn eq(&self, other: &PeerId) -> bool {
        self.get_id().eq(&other.get_id())
    }
}