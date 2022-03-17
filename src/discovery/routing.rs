use crate::discovery::{MAX_BUCKETS, MAX_BUCKET_LEN, REFRESH_INTEVAL};
use crate::node::peer_info::PeerInfo;
use crate::node::peer_key::Key;
use crate::utils::utils::Distance;
use std::hash::Hash;
use std::{cmp, mem};
use crate::utils::utils::{timestamp_now};
use std::collections::{BTreeMap, HashMap};
use ritelinked::LinkedHashMap;
use crate::node::peer_id::PeerId;
use std::net::SocketAddr;
use crate::protocol::protocol::InnerKey;

/// The derivative data type used to maintain clusters of peers
/// in the routing table with the same xor prefix to the local peer.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct KBucket {
    nodes: LinkedHashMap<PeerId, PeerInfo>,
    last_updated: u128,
}

/// The core data structure which maintains a HahsMap of xor prefix -> kbucket
/// used for peer lookups and discovery, also contains local info to measure
/// distance of incoming peers and requested lookups.
#[derive(Debug, Clone)]
pub struct RoutingTable {
    pub tree: HashMap<String, KBucket>,
    pub local_info: PeerInfo,
}

impl KBucket {
    /// Creates a new KBucket with a "last update" of now, and a capacity of MAX_BUCKET_LEN.
    pub fn new() -> Self {
        KBucket {
            nodes: LinkedHashMap::with_capacity(MAX_BUCKET_LEN),
            last_updated: timestamp_now(),
        }
    }

    /// Inserts and/or updates a peer into the bucket, because the
    /// bucket uses a LinkedHashMap, inserting an already existing
    /// key/value pair moves the pair to the back making it the most recently
    /// used. This is beneficial for cleanup purposes when we want to remove
    /// or shuffle the LRU. It also updates the last updated field to now.
    /// 
    /// # Arguments
    /// 
    /// * peer - the peer to be inserted into the kbucket
    pub fn upsert(&mut self, peer: &PeerInfo) {
        self.last_updated = timestamp_now();
        self.nodes.insert(peer.id.clone(), peer.clone());    
    }


    /// Checks if the bucket contains the peer and returns true or false
    /// 
    /// # Arguments
    /// 
    /// * peer - the peer to check
    pub fn contains(&self, peer: &PeerInfo) -> bool {
        self.nodes.contains_key(&peer.id)
    }

    /// Chreates and returns a new bucket.
    /// 
    /// TODO:
    /// 
    /// Need to implement actual splitting functionality, currently this just
    /// creates and returns a new bucket.
    /// 
    pub fn split(&mut self, index: usize, prefix: String) -> KBucket {
        let mut new_bucket = KBucket::new();        
        return new_bucket
    }

    /// Returns a vector of all the Peers in the bucket without their key
    pub fn get_nodes(&self) -> Vec<PeerInfo> {
        self.nodes.clone().iter().map(|(k, v)| v.clone()).collect()
    }

    /// Removes the least recently used peer from the bucket
    /// and returns it if one exists, otherwise returns None
    pub fn remove_lru(&mut self) -> Option<PeerInfo> {
        if self.size() == 0 {
            None
        } else {
            Some(self.nodes.pop_front().unwrap().1)
        }
    }

    /// Removes the peer requested to be removed from the bucket, if it exists.
    /// 
    /// # Arguments
    /// 
    /// * peer - the peer that we are requesting be removed.
    pub fn remove_peer(&mut self, peer: &PeerInfo) -> Option<PeerInfo> {
        self.nodes.remove(&peer.id)
    }

    /// Checks if the bucket is equal to or greater than MAX_BUCKET_LEN
    /// and returns true or false.
    pub fn is_full(&self) -> bool {
        self.nodes.len() >= MAX_BUCKET_LEN
    }

    /// Checks of the bucket has become stale and hasn't been updated in
    /// a given interval. Returns true or false
    pub fn is_stale(&self) -> bool {
        let diff = timestamp_now() - self.last_updated;
        diff > REFRESH_INTEVAL
    }

    /// Returns the number of entries in the bucket.
    pub fn size(&self) -> usize {
        self.nodes.len()
    }
}

impl RoutingTable {

    /// Creates a new instance of a RoutingTable
    /// 
    /// # Arguments
    /// 
    /// * local_info - the local nodes PeerInfo to be inserted into the routing table
    pub fn new(local_info: PeerInfo) -> Self {
        let mut tree = HashMap::new();
        let mut kbucket = KBucket::new();
        kbucket.upsert(&local_info);
        tree.insert(local_info.get_key().xor(local_info.get_key()).get_prefix(0), kbucket);
        RoutingTable { tree, local_info }
    }

    /// Recursively inserts or updates a peer into the routing table in the proper
    /// kbucket.
    /// 
    /// * peer_info - the peer to update
    /// * traverse - the xor prefix length to start at.
    /// 
    pub fn update_peer(&mut self, peer_info: &PeerInfo, traverse: usize) -> bool {
        let distance = self.local_info.get_key().xor(peer_info.get_key());
        let prefix = distance.get_prefix(traverse);
        if let Some(bucket) = self.tree.get_mut(&prefix) {
            if !bucket.is_full() {
                bucket.upsert(peer_info);
                return true
            } else {
                self.update_peer(peer_info, traverse + 1)
            }
        } else {
            let mut new_bucket = KBucket::new();
            new_bucket.upsert(peer_info);
            self.tree.insert(prefix, new_bucket.clone());
            return true
        }
    }

    /// Returns a vector of the n the closest peers to the requested peer as measured by XOR
    /// 
    /// # Arguments
    /// 
    /// * peer_info - the peer we are trying to find it's closest peers
    /// * count - the number of closest peers to find.\
    /// 
    pub fn get_closest_peers(&self, peer_info: PeerInfo, count: usize) -> Vec<PeerInfo> {
        // If the requested number of peers exceeds or is equal to the total number of peers, 
        // in our local routing table simply return all the peers from the local routing table
        if self.total_peers() <= count {
            return self.get_all_peers()
        }

        // Otherwise, get the distance as measured by the XOR of the local key to the peer's key
        let distance = self.local_info.get_key().xor(peer_info.get_key());

        // Clone the tree and retain only the kbuckets that have the same prefix
        // as the distance.
        let mut cloned_tree = self.tree.clone();
        cloned_tree.retain(|k, bucket| {
            let prefix = distance.get_prefix(k.len()-1);
            prefix == *k 
        });

        // count the total number of peers contained in the cloned tree
        // after filtering out the buckets with different prefixes.
        let total = cloned_tree.iter().fold(0, |acc, (k, v)| acc + v.size());

        // Check of the cloned tree is empty or the total peers is less than the
        // requested number of peers.
        if cloned_tree.is_empty() || total < count {
            // if it is then just get all the peers we know of
            let mut closest = self.get_all_peers();
            // sortt them by the XOR distance to the peer
            closest.sort_by_key(|peer| peer.get_key().xor(peer_info.get_key()));
            // Truncate the vector of peers to only include the number requested
            closest.truncate(count);
            // And return this vector.
            return closest
        }

        // if the cloned tree is not empty and the total number of peers is equal to
        // or exceeds the number requested, collect the prefixes, and the nodes
        // and return them as a vector of (prefix, Vec<PeerInfo>) tuples.
        let mut closest: Vec<_> = cloned_tree.iter().map(|(k, v)| {
            (k, v.get_nodes())
        }).collect();

        // Sort them by the lenght of the prefix and reverse it so that the longest
        // prefixes are first.
        closest.sort_unstable_by_key(|(k, v)| k.len());
        closest.reverse();

        // intialize an iterator over the vector and return
        // the first vector of peers.
        let mut iter = closest.iter();
        let mut ret = {
            if let Some((k, nodes)) = iter.next() {
                nodes.clone()
            } else {
                vec![]
            }
        };

        // Until the length of the return vector is equal to the
        // number of nodes, continue to iterate over the vector of tuples
        // and extend the return vector with the nodes in each tuple
        while ret.len() < count {
            if let Some((k, nodes)) = iter.next() {
                ret.extend(nodes.clone())
            } else {
                break
            }
        }

        // sort the return vector by XOR to the requesting peer and truncate to
        // only include the number requested. return the return vector. 
        ret.sort_by_key(|peer| peer.get_key().xor(peer_info.get_key()));
        ret.truncate(count);
        ret
    }

    /// Removes the least recently used peer from a given kbucket
    /// 
    /// # Arguments
    /// 
    /// * peer_key - the key of the peer used to find the kbucket that the LRU peer will be removed from
    pub fn remove_lru(&mut self, peer_key: &InnerKey) -> Option<PeerInfo> {
        // TODO:
        // This will no longer work because of the change to how the
        // tree and kbuckets are organized, restructure to work with the
        // prefix key system.
        let index = cmp::min(
            self.local_info.get_key().xor(Key::new(*peer_key)).leading_zeros(),
            self.tree.len() - 1,
        );

        let prefix = self.local_info.get_key().xor(Key::new(*peer_key)).get_prefix(index);

        if let Some(bucket) = self.tree.get_mut(&prefix) {
            return bucket.remove_lru();
        }

        None
    }

    /// Removes a specified peer from the routing table
    /// 
    /// # Arguments
    /// 
    /// * peer_info - the peer to remove from the routing table.
    pub fn remove_peer(&mut self, peer_info: &PeerInfo) {
        // TODO:
        // This will no longer work because of the change to how the
        // tree and kbuckets are organized, restructure to work with the
        // prefix key system.
        let index = cmp::min(
            self.local_info.get_key().xor(peer_info.get_key()).leading_zeros(),
            self.tree.len() - 1,
        );

        let prefix = self.local_info.get_key().xor(peer_info.get_key()).get_prefix(index);
        
        if let Some(bucket) = self.tree.get_mut(&prefix) {
            println!("{}", bucket.size());
            bucket.remove_peer(peer_info);
            println!("{}", bucket.size());
        }
    }


    /// Get stale buckets and return a vector of the keys of those buckets
    pub fn get_stale_indices(&self) -> Vec<String> {
        let mut ret = Vec::new();
        self.tree.iter().for_each(|(prefix, bucket)| {
            if bucket.is_stale() {
                ret.push(prefix.clone())
            }
        });

        ret
    }

    /// Check if a peer is a new peer or already exists in the routing table.
    /// return true or false
    /// 
    /// # Arguments
    /// 
    /// * peer - the peer to check
    /// 
    pub fn is_new(&self, peer: &PeerInfo) -> bool {
        !self.tree.iter().any(|(_, bucket)| bucket.contains(peer))
    }

    /// Get the number of kbuckets in the tree
    pub fn size(&self) -> usize {
        self.tree.len()
    }

    /// Get the total number of peers in the tree (sum of the size of all the kbuckets)
    pub fn total_peers(&self) -> usize {
        self.tree.iter().fold(0, |acc, (k, v)| acc + v.size())
    }

    /// Get all the peers in the routing table and return them as a vector of PeerInfo
    pub fn get_all_peers(&self) -> Vec<PeerInfo> {
        self.tree.iter().map(|(k, v)| {
            v.get_nodes()
        }).collect::<Vec<_>>().into_iter().flatten().collect()
    }
}
