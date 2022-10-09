#![allow(dead_code)]
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    process::id,
    sync::mpsc::Sender,
};

use log::info;
use raptorq::{Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation};
use thiserror::Error;

use crate::{
    gd_udp::gd_udp::GDUdp,
    packetize,
    protocol::protocol::{
        get_batch_id,
        get_packet_id,
        AckMessage,
        Header,
        InnerKey,
        KadMessage,
        Message,
        Packet,
    },
    utils::utils::ByteRep,
};

/// The core struct of the handler module
/// Contains an outgoing message sender
/// an incoming acknowldgement sender
/// a hashmap of pending message packets
/// a kad sender for sending messages to a kademlia instance
/// and a gossip sender for sending messages to a gossip instance
///
/// TODO: make kad_tx and gossip_tx optional
#[derive(Error, Debug)]
pub enum MsgHandlingError {
    #[error("inavlid seed generated")]
    DecoderAddFailed(),
}
pub struct MessageHandler {
    om_tx: Sender<(SocketAddr, Message)>,
    ia_tx: Sender<AckMessage>,
    pending: HashMap<InnerKey, HashMap<usize, Packet>>,
    kad_tx: Sender<(SocketAddr, KadMessage)>,
    gossip_tx: Sender<(SocketAddr, Message)>,
    decoder_hash: HashMap<[u8; 32], (usize, Decoder)>,
}

impl MessageHandler {
    /// Creates a new mesasge handler instance
    ///
    /// # Arguments
    ///
    /// * om_tx - an outgoing message sender that sends a tuple of a
    ///   SocketAddress (destination) and Message to send to the transport layer
    /// * ia_tx - an incoming acknowledgement sender that sends an
    ///   acknowledgement message to the transport (or GDUDP) layer
    /// * pending - a hashmap containing a message key as the key and a hashmap
    ///   of derived packets to store the packets until all are received and
    ///   message can be reassembled
    /// * kad_tx - a sender to send a tuple of the sender and the kad message to
    ///   a kademlia dht
    /// * gossip_tx - a sender to send a tuple of the sender address and the
    ///   message to the gossip instance
    pub fn new(
        om_tx: Sender<(SocketAddr, Message)>,
        ia_tx: Sender<AckMessage>,
        pending: HashMap<InnerKey, HashMap<usize, Packet>>,
        kad_tx: Sender<(SocketAddr, KadMessage)>,
        gossip_tx: Sender<(SocketAddr, Message)>,
    ) -> MessageHandler {
        MessageHandler {
            om_tx,
            ia_tx,
            pending,
            kad_tx,
            gossip_tx,
            decoder_hash: Default::default(),
        }
    }

    /// Receives a message to the UDP socket buffer and processes the packet
    ///
    /// # Arguments
    ///
    /// * sock - the UDP socket to read messages into the buffer from
    /// * buf - the buffer to write incoming bytes to
    /// * local - the local socket address.
    pub fn recv_msg(&mut self, sock: &UdpSocket, buf: &mut [u8], local: SocketAddr) {
        let res = sock.recv_from(buf);
        match res {
            Ok((amt, src)) => {
                if let Some(packet) = self.process_packet(local, buf.to_vec(), amt, src) {
                    dbg!("in recvb msg hcekcin gi f packet is raptorQ");
                    if !packet.is_raptor_q_packet {
                        self.insert_packet(packet, src);
                    } else {
                        //TODO: rm dbg statements
                        dbg!("Packet is RQPTORQ");
                        let mut data = hex::decode(packet.bytes.clone()).unwrap();

                  
                        if let Some(i) = data.iter().rposition(|x| *x != 0) {
                            let new_len = i + 1;
                            data.truncate(new_len);
                        }

                        let size = std::mem::size_of::<usize>();
                        println!("Data {:?}, received {:?}", data.len(), amt - 50);
                        println!("Data :{:?}", data);
                        match self.decoder_hash.get_mut(&packet.id) {
                            Some((num_packets, decoder)) => {
                                println!("Entered");
                                *num_packets += 1;
                                dbg!("here");
                                // Decoding the packet.
                                let result = decoder
                                    .decode(EncodingPacket::deserialize(&data[0..1228].to_vec()));
                                dbg!("here");
                                println!("Received {:?}", result);
                                if !result.is_none() {
                                    // This is the part of the code that is sending the reassembled
                                    // file to the `file_send` channel.
                                    let result_bytes = result.unwrap();
                                    let batch_id_str =
                                        String::from(std::str::from_utf8(&packet.id).unwrap());
                                    let msg = (batch_id_str, result_bytes);
                                    self.decoder_hash.remove(&packet.id);
                                }
                            },
                            None => {
                                dbg!("NO DECODER");
                                // This is creating a new decoder for a new batch.
                                self.decoder_hash.insert(
                                    packet.id,
                                    (
                                        1_usize,
                                        Decoder::new(ObjectTransmissionInformation::new(
                                            108, 1176, 1, 1, 8,
                                        )),
                                    ),
                                );

                                match self.decoder_hash.get_mut(&packet.id) {
                                    Some((num_packets, decoder)) => {
                                        println!("Entered");
                                        *num_packets += 1;
                                        dbg!("here1");
                                        // Decoding the packet.
                                        let result = decoder.decode(EncodingPacket::deserialize(
                                            &data[0..1228].to_vec(),
                                        ));
                                        dbg!("here1");

                                        match &result {
                                            Some(_) => {
                                                dbg!("here2");
                                                println!("Received {:?}", result);
                                                // This is the part of the code that is sending the
                                                // reassembled file to the `file_send` channel.
                                                let result_bytes = result.unwrap();
                                                if (result_bytes.len() != 0) {
                                                    let batch_id_str = get_packet_id(&packet);
                                                    let msg = (batch_id_str, result_bytes);
                                                    self.decoder_hash.remove(&packet.id);
                                                } else {
                                                    println!("empty packet");
                                                }
                                            },
                                            None => {
                                                dbg!("here");
                                                println!("Received {:?}", result);
                                            },
                                        }
                                    },
                                    None => {
                                        dbg!("Error; decoder could not be added");
                                    },
                                }
                            },
                        }
                    }
                }
            },

            Err(_) => {
                println!("Error occurred");
            },
        }
    }

    /// Processes the packet, sends an acknowledgement if requested, and returns
    /// the packet
    ///
    /// # Arguments
    ///
    /// * local - the local nodes socket address
    /// * buf - a vector of u8 bytes to write packet bytes to
    /// * amt - the number of bytes received by the socket
    /// * src - the sender of the message
    pub fn process_packet(
        &self,
        local: SocketAddr,
        buf: Vec<u8>,
        amt: usize,
        src: SocketAddr,
    ) -> Option<Packet> {
        if let Some(packet) = Packet::from_bytes(&buf[..amt]) {
            if packet.ret == GDUdp::RETURN_RECEIPT {
                let ack = AckMessage {
                    packet_id: packet.id,
                    packet_number: packet.n,
                    src: local.to_string().as_bytes().to_vec(),
                };
                let header = Header::Ack;
                let message = Message {
                    head: header,
                    msg: ack.as_bytes().unwrap(),
                };

                if let Err(_) = self.om_tx.clone().send((src, message)) {
                    info!("Error sending ack message to transport thread");
                }
            }
            return Some(packet);
        }
        None
    }

    /// Inserts a packet into the pending table, and checks if all the packets
    /// for the message they're dervied from. If so it reassembles the
    /// message and calls handle_message
    ///
    /// # Arguments
    ///
    /// * packet - the packet received
    /// * src - the sender of the packet
    pub fn insert_packet(&mut self, packet: Packet, src: SocketAddr) {
        if let Some(map) = self.pending.clone().get_mut(&packet.id) {
            map.entry(packet.n).or_insert(packet.clone());
            self.pending.insert(packet.id, map.clone());
            if map.len() == packet.total_n {
                if let Some(message) = self.assemble_packets(packet.clone(), map.clone()) {
                    self.handle_message(message, src);
                }
            }
        } else {
            if packet.total_n == 1 {
                let bytes = hex::decode(&packet.bytes).unwrap();
                if let Some(message) = Message::from_bytes(&bytes) {
                    self.handle_message(message, src);
                }
            } else {
                let mut map = HashMap::new();
                map.insert(packet.n, packet.clone());
                self.pending.insert(packet.id, map);
            }
        }
    }

    /// Assembles the packets and returns a message
    ///
    /// # Arguments
    ///
    /// * packet - the final packet received, used to get the total number of
    ///   packets
    /// * map - the map of all the bytes for the message that needs to be
    ///   assembled
    fn assemble_packets(&self, packet: Packet, map: HashMap<usize, Packet>) -> Option<Message> {
        let mut bytes = vec![];
        (1..=packet.total_n).into_iter().for_each(|n| {
            let converted = hex::decode(&map[&n].bytes.clone()).unwrap();
            bytes.extend(converted)
        });
        Message::from_bytes(&bytes)
    }

    /// Handles and routes a message to the proper component
    ///
    /// # Arguments
    ///
    /// * message - the message to be routed
    /// * src - the sender of the message
    fn handle_message(&self, message: Message, src: SocketAddr) {
        match message.head {
            Header::Request | Header::Response => {
                if let Some(msg) = KadMessage::from_bytes(&message.msg) {
                    if let Err(_) = self.kad_tx.send((src, msg)) {
                        info!("Error sending to kad");
                    }
                }
            },
            Header::Ack => {
                if let Some(ack) = AckMessage::from_bytes(&message.msg) {
                    if let Err(_) = self.ia_tx.send(ack) {
                        info!("Error sending ack message")
                    }
                }
            },
            Header::Gossip => {
                if let Err(_) = self.gossip_tx.send((src, message)) {
                    info!("Error sending to gossip");
                }
            },
            _ => {},
        }
    }
}
