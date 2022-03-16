use std::net::SocketAddr;
use crate::traits::routable::Routable;
use crate::node::peer_id::PeerId;
use crate::node::peer_key::Key;
use serde::{Serialize, Deserialize};
use crate::utils::utils::Distance;
use std::cmp::Ordering;
use crate::utils::utils::ByteRep;
use crate::impl_ByteRep;

impl_ByteRep!(for PeerInfo);

#[derive(Clone, Debug, Hash, Serialize, Deserialize, Eq)]
pub struct PeerInfoDistancePair(pub PeerInfo, pub Key);

impl PartialEq for PeerInfo {
    fn eq(&self, other: &PeerInfo) -> bool {
        self.key.get_key().eq(&other.key.get_key())
    }
}

impl PartialOrd for PeerInfo {
    fn partial_cmp(&self, other: &PeerInfo) -> Option<Ordering> {
        Some(other.key.get_key().cmp(&self.key.get_key()))
    }
}

impl PartialEq for PeerInfoDistancePair {
    fn eq(&self, other: &PeerInfoDistancePair) -> bool {
        self.0.eq(&other.0)
    }
}

impl PartialOrd for PeerInfoDistancePair {
    fn partial_cmp(&self, other: &PeerInfoDistancePair) -> Option<Ordering> {
        Some(other.1.cmp(&self.1))
    }
}

impl Ord for PeerInfoDistancePair {
    fn cmp(&self, other: &PeerInfoDistancePair) -> Ordering {
        other.1.cmp(&self.1)
    }
}

/// The core identifying struct for a node in the network
#[derive(Clone, Debug, Hash, Serialize, Deserialize, Eq)]
pub struct PeerInfo {
    pub id: PeerId,
    pub key: Key,
    pub address: SocketAddr,
}

impl PeerInfo {

    /// Generate a new PeerInfo instance given a PeerId, Key and Socket Address
    /// 
    /// # Arguments
    /// 
    /// * id - a PeerId instance
    /// * key - the key used to generate the PeerId
    /// * address - the receiving socket address for the local node
    /// 
    pub fn new(id: PeerId, key: Key, address: SocketAddr) -> Self {
        
        PeerInfo {
            id,
            key,
            address,
        }
    }

    /// gets the local nodes's key
    pub fn get_key(&self) -> Key {
        self.key.clone()
    }
}

impl Distance for PeerInfo {
    type Output = Key;

    fn xor(&self, other: Key) -> Key {
        self.key.xor(other)
    }

    fn leading_zeros(&self) -> usize {
        self.key.leading_zeros()
    }

}

impl Routable for PeerInfo {
    type Id = PeerId;
    type Key = Key;

    fn get_id(&self) -> Self::Id {
        self.id.clone()
    }

    fn get_key(&self) -> Self::Key {
        self.key
    }

    fn get_address(&self) -> SocketAddr {
        self.address
    }
}