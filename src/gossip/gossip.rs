#![allow(dead_code)]
use crate::gossip::protocol::GossipMessage;
use crate::discovery::kad::Kademlia;
use crate::protocol::protocol::{Message, MessageKey};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use crate::utils::utils::ByteRep;
use std::time::Instant;
use rand::Rng;
use crate::traits::routable::Routable;
use log::info;

/// A configuration struct for the user to pass different
/// parameters into the gossip struct.
pub struct GossipConfig {
    // Protocol ID
    id: String,
    // Number of heartbeats to keep in cache
    history_len: usize,
    // Number of past heartbeats to gossip about
    history_gossip: usize,
    // Target number of peers
    target: usize,
    // Minimum number of peers
    low: usize,
    // Maximum number of peers
    high: usize,
    // Minimum number of peers to disseminate gossip to
    min_gossip: usize,
    // % of peers to send gossip to
    factor: f64,
    // Time between heartbeats
    interval: Duration,
    check: usize,
}


/// The core gossip struct. This is the gosisp engine that handles incoming and outgoing
/// messages, peer sampling, and other message
pub struct GossipService {
    address: SocketAddr,
    public_ip: SocketAddr,
    to_gossip_rx: Receiver<(SocketAddr, Message)>,
    pub to_transport_tx: Sender<(SocketAddr, Message)>,
    pub to_app_tx: Sender<GossipMessage>,
    pub kad: Kademlia,
    cache: HashMap<MessageKey, (Message, Instant)>,
    config: GossipConfig,
    heartbeat: Instant,
    ping_pong: Instant,
    // TODO: add pending pings
    // to clean when received
    // and check expired pings to
    // remove unresponsive nodes
    // from the routing table
}

impl GossipConfig {

    /// Create a new GossipConfig Instance
    /// 
    /// # Arguments
    /// 
    /// * id - a string representing the protocol id
    /// * history_len - the number of heartbeats to keep messages in the cache
    /// * history_gossip - the number of heartbeats to gossip about messages
    /// * target - the target number of peers to gossip to
    /// * low - the minimum number of peers to maintain in routing table
    /// * high - the maximum number of peers to maintain a 'connection' to (See Gossip Documentation)
    /// * min_gossip - the minimum number of peers to disseminate gossip to at each heartbeat
    /// * factor - the percentage of 'connected' peers to gossip to
    /// * interval - the length of a heartbeat
    /// * check - the number of heartbeats between sending ping messages
    /// 
    pub fn new(
        id: String,
        history_len: usize,
        history_gossip: usize,
        target: usize,
        low: usize,
        high: usize,
        min_gossip: usize,
        factor: f64,
        interval: Duration,
        check: usize,
    ) -> GossipConfig {
        GossipConfig {
            id,
            history_len,
            history_gossip,
            target,
            low,
            high,
            min_gossip,
            factor,
            interval,
            check
        }
    }

    /// Return the minimum number of peers to gossip to
    pub fn min(&self) -> usize {
        self.min_gossip
    }

    /// Return the maximum number of peers to maintain a 'connection' with.
    pub fn max(&self) -> usize {
        self.high
    }
}

impl GossipService {

    /// Creates a new instance of GossipService
    /// 
    /// # Arguments
    /// 
    /// * address - the local node's socket address
    /// * to_gossip_rx - the receiver for the gossip service to accept incoming messages from the transport layer to
    /// * to_transport_tx - the sender used for sending messages to the transport layer
    /// * to_app_tx - a sender to send to apps that this create is integrated into
    /// * kad - a kademlia dht used as a peer discovery routing table 
    /// * config - a GossipConfig instance that contains configuration information for the local gossip instance
    /// * heartbeat - the time of the last heartbeat
    /// * ping_pong - the time of the last ping message sent
    /// 
    pub fn new(
        address: SocketAddr,
        public_ip: SocketAddr,
        to_gossip_rx: Receiver<(SocketAddr, Message)>,
        to_transport_tx: Sender<(SocketAddr, Message)>,
        to_app_tx: Sender<GossipMessage>,
        kad: Kademlia,
        config: GossipConfig,
        heartbeat: Instant,
        ping_pong: Instant,
    ) -> GossipService {
        GossipService {
            address,
            public_ip,
            to_gossip_rx,
            to_transport_tx,
            to_app_tx,
            cache: HashMap::new(),
            kad,
            config,
            heartbeat,
            ping_pong,
        }
    }

    /// The main gossip service loop
    pub fn start(&mut self, tx: Sender<GossipMessage>) {
        let inner_tx = tx.clone();
        loop {
            let tx = inner_tx.clone();
            self.kad.recv();
            self.recv(tx.clone());
            self.gossip();
        }
    }

    /// Checks whether enough time has passed to be considered a heartbeat
    /// returns true or false
    pub fn heartbeat(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.heartbeat) > self.config.interval {
            self.heartbeat = now;
            return true
        }
        false 
    }

    /// Dissemenates messages that are still "alive" and in the message cache.
    pub fn gossip(&mut self) {
        let now = Instant::now();
        let cache_clone = self.cache.clone();
        if self.heartbeat() {
            cache_clone.iter().for_each(|(key, (message, expires))| {
                if now.duration_since(*expires) < self.config.interval * self.config.history_gossip as u32 {
                    if let Some(gossip_message) = GossipMessage::from_bytes(&message.msg) {
                        let src = gossip_message.sender;
                        self.publish(&src, message.clone())     
                    }
                }
                if now.duration_since(*expires) > self.config.interval * self.config.history_len as u32 {
                    self.cache.remove(&key);
                }
            });

            // TODO: Add Ping Pong messages every x heartbeats.
            if now.duration_since(self.ping_pong) > self.config.interval * self.config.check as u32 {
                // Send Ping message to peers
            }
        }
    }

    /// Forwards a message and an intended destination address to the transport layer
    /// 
    /// # Arguments
    /// 
    /// * src - the destination socket address
    /// * message - a Message to be packetized and forwarded to the destination
    /// 
    pub fn publish(&mut self, src: &SocketAddr, message: Message) {
        let local = self.kad.routing_table.local_info.clone();
        let gossip_to = {
            let mut sample = HashSet::new();
            let peers = self.kad.routing_table.get_closest_peers(local, 30).clone();
            if peers.len() > 7 {

                let infection_factor = self.config.factor;
                let n_peers = peers.len() as f64 * infection_factor;
                for _ in 0..n_peers as usize {
                    let rn: usize = rand::thread_rng().gen_range(0..peers.len());
                    let address = peers[rn].get_address();
                    if &address != src && address != self.address {
                        sample.insert(address);
                    }
                }
            
                sample

            } else {
                peers.iter().for_each(|peer| {
                    let address = peer.get_address();
                    if address != self.address {
                        sample.insert(address);
                    }
                });

                sample
            }
        };
        gossip_to.iter().for_each(|peer| {
            if let Err(_) = self.to_transport_tx.send((peer.clone(), message.clone())) {
                println!("Error forwarding to transport")
            }
        });
    }


    /// handles an incoming message by placing it in the cache and forwarding to peers
    /// if it is not already in the cache. If it is already in the cache it is ignored
    /// If the protocol is 'chat' and the source is not the local node it prints the message
    /// 
    /// # Arguments
    /// 
    /// * src - the sender of the incoming message
    /// * msg - the incoming message to be handled
    /// 
    fn handle_message(&mut self, src: &SocketAddr, msg: &Message) -> Option<GossipMessage> {
        if let Some(message) = GossipMessage::from_bytes(&msg.msg){
            if !self.cache.contains_key(&MessageKey::from_inner(message.id)) {
                if *src != self.address {
                    if let Err(e) = self.to_app_tx.send(message.clone()) {
                        info!("Error sending message to application layer: {:?}", e)
                    }
                    let key = MessageKey::from_inner(message.id);
                    self.publish(src, msg.clone());
                    self.cache.entry(key).or_insert((msg.clone(), Instant::now()));    
                    return Some(message)
                } else {
                    let key = MessageKey::from_inner(message.id);
                    self.cache.entry(key).or_insert((msg.clone(), Instant::now()));
                    return None

                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// receives messages coming into the "to_gossip_rx"
    pub fn recv(&mut self, tx: Sender<GossipMessage>) {
        let res = self.to_gossip_rx.try_recv();
        match res {
            Ok((src, msg)) => {
                if let Some(string) = self.handle_message(&src, &msg) {
                    if let Err(e) = tx.clone().send(string) {
                        println!("Error sending message");
                    }
                }
            }
            Err(_) => { }
        }
    }

    pub fn get_pub_ip(&self) -> SocketAddr {
        self.public_ip.clone()
    }
}
