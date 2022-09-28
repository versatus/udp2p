use crate::discovery::protocol::{Req, Resp, RPC};
use crate::discovery::routing::RoutingTable;
use crate::discovery::DEFAULT_N_PEERS;
use crate::node::peer_info::PeerInfo;
use crate::node::peer_key::Key;
use crate::protocol::protocol::{
    Header, KadMessage, Message, MessageKey, Nodes, Peer, RequestBytes, ResponseBytes, Value,
};
use crate::traits::routable::Routable;
use crate::utils::utils::timestamp_now;
use crate::utils::utils::ByteRep;
use crate::utils::utils::Distance;
use log::info;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// The kademlia is the basic struct used for Peer Discovery in this crate
/// Kademlia has a RoutingTable, a to_transport sender and from transport receiver
/// that is used to communicate incoming and outgoing data from the
/// transport layer. It also maintains a set of keys from pending messages
/// to ensure that any responses are in relation to a message the local node
/// sent. Lastly, an interval used to maintain the amount of time between
/// ping-pong events, and a ping pong timer that is used to check whether
/// or not the amount of time since the last ping pong event has exceeded
/// the interval or not. If so then it is time to check on the health of
/// the peers and clean up the routing table to get rid of any unresponsive peers.
#[derive(Debug)]
pub struct Kademlia {
    pub routing_table: RoutingTable,
    pub to_transport: Sender<(SocketAddr, Message)>,
    pub from_transport: Receiver<(SocketAddr, KadMessage)>,
    pub pending: HashSet<MessageKey>,
    interval: Duration,
    ping_pong: Instant,
}

impl Kademlia {
    /// Creates a new Kademlia instance
    ///
    /// # Arguments
    /// * routing_table - the routing table for this instance
    /// * to_transport - an mpsc sender to the transport layer
    /// * from_transport - an mpsc receiver from the transport layer
    /// * pending - a hashset of message keys of pending outgoing messages
    /// * interval - a fixed duration used to check if it is time to send ping pong events
    /// * ping_pong - an instant that is checked against the interval to determine if its time to send ping pong events
    pub fn new(
        routing_table: RoutingTable,
        to_transport: Sender<(SocketAddr, Message)>,
        from_transport: Receiver<(SocketAddr, KadMessage)>,
        pending: HashSet<MessageKey>,
        interval: Duration,
        ping_pong: Instant,
    ) -> Kademlia {
        Kademlia {
            routing_table,
            to_transport,
            from_transport,
            pending,
            interval,
            ping_pong,
        }
    }

    /// A method to receive data from the transport layer and determine if
    /// it is time to send ping-pong events.
    pub fn recv(&mut self) {
        let res = self.from_transport.try_recv();
        match res {
            Ok((_src, msg)) => {
                self.handle_message(&msg);
            }
            Err(_) => {}
        }

        // TODO: check if its time to send pings out
        let now = Instant::now();
        if now.duration_since(self.ping_pong) > self.interval {
            // Send ping messages to peers.
        }
    }

    /// Adds a peer to the routing table if they don't exist
    /// Update's a peer if they do exist
    ///
    /// # Arguments
    ///
    /// * peer - a Byte Representation of a PeerInfo struct.
    ///
    pub fn add_peer(&mut self, peer: Peer) {
        println!("{:?}", PeerInfo::from_bytes(&peer));
        let peer = PeerInfo::from_bytes(&peer).unwrap();
        self.routing_table.update_peer(&peer, 0);
    }

    /// Requests nodes from the bootstrap node provided at the start
    /// and then subsequently adds and requests more nodes from each
    /// node returned by the bootstrap node, etc. etc.
    ///
    /// # Arguments
    ///
    /// * bootstrap - The socket address of the bootstrap node
    ///
    pub fn bootstrap(&mut self, bootstrap: &SocketAddr) {
        // Structure Message
        info!("Bootstrapping peer: {:?}", bootstrap);
        let local_info = self.routing_table.local_info.clone();
        let (id, message) = self.prepare_find_node_message(local_info, None);
        if let Err(e) = self.to_transport.send((bootstrap.clone(), message)) {
            info!("Error sending to transport: {:?}", e);
        }
        self.add_peer(self.routing_table.local_info.clone().as_bytes().unwrap());
    }

    /// Structures and returns a nodes response message, i.e. a response to a find nodes request
    ///
    /// # Arguments
    ///
    /// * req - the request we are responding to
    /// * nodes - the nodes found during the node lookup
    ///
    pub fn prepare_nodes_response_message(&self, req: Req, nodes: Vec<PeerInfo>) -> Message {
        let nodes_vec: Nodes = nodes.iter().map(|peer| peer.as_bytes().unwrap()).collect();
        let local_info = self.routing_table.local_info.clone();
        let rpc: RPC = RPC::Nodes(nodes_vec);
        let resp = Resp {
            request: req.as_bytes().unwrap(),
            receiver: local_info.as_bytes().unwrap(),
            payload: rpc.as_bytes().unwrap(),
        };

        let msg = Message {
            head: Header::Response,
            msg: KadMessage::Response(resp.as_bytes().unwrap())
                .as_bytes()
                .unwrap(),
        };

        msg
    }

    /// Prepares a find node request message to be sent to a peer
    ///
    /// # Arguments
    ///
    /// * peer - the node that we are requesting a lookup on
    /// * req - an optional Req used if the message is forwarded from a node other than the original requestor
    ///
    pub fn prepare_find_node_message(
        &self,
        peer: PeerInfo,
        req: Option<Req>,
    ) -> (MessageKey, Message) {
        if let Some(opt_req) = req {
            let message = Message {
                head: Header::Request,
                msg: KadMessage::Request(opt_req.as_bytes().unwrap())
                    .as_bytes()
                    .unwrap(),
            };

            return (MessageKey::from_inner(opt_req.id), message);
        }

        let local_info = self.routing_table.local_info.clone();
        let rpc: RPC = RPC::FindNode(peer.as_bytes().unwrap());
        let req: Req = Req {
            id: MessageKey::rand().inner(),
            sender: local_info.as_bytes().unwrap(),
            payload: rpc.as_bytes().unwrap(),
        };

        let msg = Message {
            head: Header::Request,
            msg: KadMessage::Request(req.as_bytes().unwrap())
                .as_bytes()
                .unwrap(),
        };
        (MessageKey::from_inner(req.id), msg)
    }

    /// Prepares a new peer message to be sent to known peers when a new peer is being bootstrapped.
    /// Eventually, use of this crate will be enabled even for users that do not have port forwarding
    /// and as such, known peers need to know when a new peer joins so they can initiate the hole punching
    /// procedure, and hopefully complete the procedure when the other node finds out (at rougly the same time)
    /// that they exist
    ///
    /// # Arguments
    ///
    /// * peer - the new peer discovered.
    pub fn prepare_new_peer_message(&self, peer: PeerInfo) -> (MessageKey, Message) {
        let local_info = self.routing_table.local_info.clone();
        let rpc: RPC = RPC::NewPeer(peer.as_bytes().unwrap());
        let req: Req = Req {
            id: MessageKey::rand().inner(),
            sender: local_info.as_bytes().unwrap(),
            payload: rpc.as_bytes().unwrap(),
        };

        let msg = Message {
            head: Header::Request,
            msg: KadMessage::Request(req.as_bytes().unwrap())
                .as_bytes()
                .unwrap(),
        };
        (MessageKey::from_inner(req.id), msg)
    }

    /// Prepares a ping message to be sent out to peers to check their health
    pub fn prepare_ping_message(&self) -> (MessageKey, Message) {
        let local_info = self.routing_table.local_info.clone();
        let rpc: RPC = RPC::Ping;
        let req: Req = Req {
            id: MessageKey::rand().inner(),
            sender: local_info.as_bytes().unwrap(),
            payload: rpc.as_bytes().unwrap(),
        };

        let msg = Message {
            head: Header::Request,
            msg: KadMessage::Request(req.as_bytes().unwrap())
                .as_bytes()
                .unwrap(),
        };
        (MessageKey::from_inner(req.id), msg)
    }

    /// Structures the message used to repond to a ping request
    ///
    /// # Arguments
    ///
    /// * peer - the peer that sent the ping request
    /// * req - the original ping request
    ///
    pub fn prepare_pong_response(&self, peer: &PeerInfo, req: Req) -> Message {
        let rpc = RPC::Pong(self.routing_table.local_info.as_bytes().unwrap());
        let resp = Resp {
            request: req.as_bytes().unwrap(),
            receiver: peer.as_bytes().unwrap(),
            payload: rpc.as_bytes().unwrap(),
        };

        let msg = Message {
            head: Header::Response,
            msg: KadMessage::Response(resp.as_bytes().unwrap())
                .as_bytes()
                .unwrap(),
        };

        msg
    }

    pub fn prepare_store_message(&self, peer: &PeerInfo, value: Value) {}
    pub fn prepare_saved_response(&self, peer: &PeerInfo, req: Req) {}
    pub fn prepare_find_value_message(&self, value: Value) {}
    pub fn prepare_value_response(&self, peer: &PeerInfo, value: Value, req: Req) {}

    /// The base request handler. This function does alot of the "heavy lifting"
    /// for the kademlia structure by routing different RPCs to the correct function
    ///
    /// # Arguments
    ///
    /// * req - a byte representation of an incoming Req struct
    ///
    fn handle_request(&mut self, req: &RequestBytes) {
        let req_msg = Req::from_bytes(&req);
        if let Some(request) = req_msg {
            let (id, sender, rpc) = request.to_components();
            self.add_peer(sender.clone().unwrap().as_bytes().unwrap());
            match rpc.unwrap() {
                RPC::FindNode(node) => {
                    let peer = PeerInfo::from_bytes(&node);
                    if let Some(peer) = peer {
                        self.lookup_node(peer, request.clone());
                    }
                }
                RPC::NewPeer(peer) => {
                    self.add_peer(peer);
                }
                RPC::FindValue(value) => {
                    // TODO:
                    //
                    // Check if you are a provider
                    // if so respond with the value
                    // if not, check if you know a provider
                    // if so, request the value from the provider
                    // if not then find the closest peers to the key
                    // and respond with a nodes message.
                    // Change response handler to be able to handle the
                    // case of a nodes response to a FindValue request.
                }
                RPC::Store(key, value) => {
                    // TODO:
                    //
                    // Add to record store, respond with
                    // Saved response with the key for this record
                    // so that the receiving node can add you as a
                    // provider of the value.
                }
                RPC::Ping => {
                    // Send pong response
                    let resp_msg = self.prepare_pong_response(&sender.clone().unwrap(), request);
                    if let Err(e) = self
                        .to_transport
                        .send((sender.unwrap().address.clone(), resp_msg.clone()))
                    {
                        info!("Error sending to transport: {:?}", e);
                    }
                }
                _ => {
                    self.handle_response(req);
                }
            }
        }
    }

    /// The mirror image of the handle request function, this function does the "heavy lifting"
    /// for incoming response RPCs, by routing them to the correct function
    ///
    /// # Arguments
    ///
    /// * resp - a byte representation of a Resp struct
    ///
    fn handle_response(&mut self, resp: &ResponseBytes) {
        let resp_msg = Resp::from_bytes(&resp);
        if let Some(rm) = resp_msg {
            let (req, receiver, rpc) = rm.to_components();
            if let Some(request) = req {
                let (id, sender, req_rpc) = request.to_components();
                let mut complete = false;
                match rpc.unwrap() {
                    RPC::Nodes(nodes) => {
                        // TOOD:
                        //
                        // match req.
                        // IF req is a FindNode
                        // then proceed with the below functionality
                        nodes.iter().for_each(|peer| {
                            let peer_info = PeerInfo::from_bytes(&peer);
                            if let Some(info) = peer_info {
                                let new = self.routing_table.is_new(&info);
                                self.add_peer(peer.clone());
                                if new {
                                    self.bootstrap(&info.get_address());
                                }
                            }
                        })
                        // TODO:
                        //
                        // IF the req is a FindValue then
                        // attempt to find the value from the new
                        // nodes received. This means that the
                        // peer you requested the value from
                        // didn't have it and didn't know any
                        // providers of it, so they sent you
                        // the closest nodes they know of to
                        // the value.
                        // If
                    }
                    RPC::Value(value) => {}
                    RPC::Saved(key) => {}
                    RPC::Pong(peer) => {
                        // TODO:
                        //
                        // Check pending requests to ensure
                        // the Ping request isn't expired
                        // If it hadn't expired then
                        // The peer is still alive
                        // keep them in the routing table and update
                        // the routing table to move them to the back
                        // as the LRU
                        self.add_peer(peer)
                    }
                    _ => {
                        self.handle_request(resp);
                    }
                }
            }
        }
    }

    /// Decodes and determines which type of message the
    /// incoming message is. If it's a Request variant, then it calls handle_request()
    /// if it's a Response variant then it calls handle_response.
    ///
    /// # Arguments
    ///
    /// * message - a KadMessage enum that contains a response or request and the relevant RPC.
    pub fn handle_message(&mut self, message: &KadMessage) {
        match message {
            KadMessage::Request(req) => {
                self.handle_request(req);
            }
            KadMessage::Response(resp) => {
                self.handle_response(&resp);
            }
            KadMessage::Kill => {}
        }
    }

    pub fn ping_node(&mut self, node: PeerInfo) {}
    pub fn pong_response(&mut self, node: PeerInfo, req: Req) {}

    /// The core function of the kademlia DHT. This function takes in a peer (and the request that contained said peer)
    /// and traverses the Routing table retuning DEFAULT_N_PEERS closest peers. It then responds to the requestor
    /// with the closest peers to the requested peer, and if the peer is a new peer subsequently sends all the closest
    /// peers a new peer message to inform them that we have discovered a new peer.
    ///
    /// # Arguments
    ///
    /// * node - the node to lookup and/or find it's closest peers
    /// * req - the original request.
    pub fn lookup_node(&mut self, node: PeerInfo, req: Req) {
        let mut closest_peers = self
            .routing_table
            .get_closest_peers(node.clone(), DEFAULT_N_PEERS);
        let (_, sender, rpc) = req.to_components();
        // TODO: check if the peer is a new peer, if so then send a new peer message
        // if not then don't worry about it.
        if let Some(bytes) = node.as_bytes() {
            self.add_peer(bytes);
        }
        let (id, msg) = self.prepare_new_peer_message(node.clone());
        let resp_msg = self.prepare_nodes_response_message(req.clone(), closest_peers.clone());
        if let Err(e) = self
            .to_transport
            .send((node.address.clone(), resp_msg.clone()))
        {
            info!("Error sending to transport: {:?}", e);
        }
        closest_peers.iter().for_each(|peer| {
            if let Err(e) = self.to_transport.send((peer.get_address(), msg.clone())) {
                info!("Error sending to transport: {:?}", e);
            }
        });
    }

    pub fn lookup_value(&mut self, value: Value) {}
    pub fn store_value(&mut self, value: Value) {}
}
