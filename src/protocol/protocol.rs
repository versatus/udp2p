use crate::impl_ByteRep;
use crate::utils::utils::ByteRep;
use raptorq::{Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation};
use serde::{Deserialize, Serialize};

impl_ByteRep!(for Packet, AckMessage, Message, MessageKey, Header, KadMessage);

/// There are many instances where a byte
/// representation of a given struct or enu
/// is passed through a function or used for something
/// To make it easier to distinguish what the byte vector
/// is intending to represent, we have some custom types
/// used instead of passsing in a Vec<u8> alone.
// TODO: Convert Vec<u8> to &'static [u8]
// TODO convert Vec<Vec<u8>> to &'statis[&'static [u8]]
//      REASON:
//          Vectors take up double their allocates space
//          wheras arrays do not, this will result in significant memory gains

pub type Peer = Vec<u8>;
pub type RequestBytes = Vec<u8>;
pub type ResponseBytes = Vec<u8>;
pub type Value = Vec<u8>;
pub type Nodes = Vec<Vec<u8>>;
pub type StoreKey = [u8; 32];
pub type InnerKey = [u8; 32];
pub type RPCBytes = Vec<u8>;
pub type MessageData = Vec<u8>;
pub type ReturnReceipt = u8;
pub type AddressBytes = Vec<u8>;
pub type Packets = Vec<Packet>;

/// For RaptorQ
// using random 32 bytes - lets say ipfs Hash
const BATCH_ID_SIZE: usize = 32;
//How many packets to recieve from socket in single system call
pub const NUM_RCVMMSGS: usize = 32;
/// Maximum over-the-wire size of a Transaction
///   1280 is IPv6 minimum MTU
///   40 bytes is the size of the IPv6 header
///   8 bytes is the size of the fragment header
pub const MTU_SIZE: usize = 1280;
const PACKET_SNO: usize = 4;
const FLAGS: usize = 1;
///   40 bytes is the size of the IPv6 header
///   8 bytes is the size of the fragment header
const PAYLOAD_SIZE: usize = MTU_SIZE - PACKET_SNO - FLAGS - 40 - 8;

pub trait Protocol {}

/// A trait implemented on objects that need to be sent across
/// the network, whether kademlia, gossip, or some other protocol
pub trait Packetize<'a>: ByteRep<'a> {
    fn packetize(&self) -> Vec<Packet>;
}

#[macro_export]
macro_rules! packetize {
    ($bytes:expr, $id:expr, $ret:expr, $size:expr) => {
        if $size < 1024 {
            let hex_string: ::hex::encode(&$bytes);
            let packet = ::protocol::Packet {
                id: $id,
                n: 1,
                total_n: 1,
                bytes: hex_string,
                ret: $ret,
            };
            return vec![packet];
        } else {
            let mut n_packets = $size / 2048;
            if $size % 1024 != 0 {
                n_packets += 1;
            }
            let mut start = 0;
            let mut end = 1024;
            let mut packets = vec![];

            for n in 0..n_packets {
                if n == n_packets - 1 {
                    packets.push(bytes[start..].to_vec());
                } else {
                    packets.push(bytes[start..end].to_vec());
                    start = end;
                    end += 1024;
                }
            }

            let packets: Packets = packets
                .iter()
                .enumerate()
                .map(|(idx, packet)| {
                    let hex_string = hex::encode(&packet);
                    ::protocol::Packet {
                        id,
                        n: idx + 1,
                        total_n: packets.len(),
                        bytes: hex_string,
                        ret,
                    }
                })
                .collect();

            packets
        }
    };
}

/// A function that returns a vector of *n* Packet(s) based on the size of
/// the MessageData passed to it.
///
/// # Arguments
///
/// * bytes - a vector of u8 bytes representing the message data to be split up into packets
/// * id - a common id shared by all packets derived from the same message for reassembly by the receiver
/// * ret - a 0 or 1 representing whether a return receipt is required of the sender
///
/// TODO:
///
/// Build a macro for this
///

pub fn packetize(bytes: MessageData, id: InnerKey, ret: ReturnReceipt,is_raptor_q_packet:bool) -> Packets {
    if bytes.len() < 1024 {
        let hex_string = hex::encode(&bytes);
        let packet = Packet {
            id,
            n: 1,
            total_n: 1,
            bytes: hex_string,
            ret,
            is_raptor_q_packet
        };
        return vec![packet];
    }

    let mut n_packets = bytes.len() / 1024;
    if n_packets % 1024 != 0 {
        n_packets += 1;
    }

    let mut start = 0;
    let mut end = 1024;
    let mut packets = vec![];

    for n in 0..n_packets {
        if n == n_packets - 1 {
            packets.push(bytes[start..].to_vec());
        } else {
            packets.push(bytes[start..end].to_vec());
            start = end;
            end += 1024;
        }
    }

    let packets: Packets = packets
        .iter()
        .enumerate()
        .map(|(idx, packet)| {
            let hex_string = hex::encode(&packet);
            Packet {
                id,
                n: idx + 1,
                total_n: packets.len(),
                bytes: hex_string,
                ret,
                is_raptor_q_packet
            }
        })
        .collect();

    packets
}

/// Packet contains a common id derived from the message
/// n is the packet number, total_n is the total number of packets
/// generated by the message, bytes is a hexadecimal string representaiton
/// of the MessageData broken down into packet(s)
/// ret is a 0 or 1 representing a return receipt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Packet {
    pub id: InnerKey,
    pub n: usize,
    pub total_n: usize,
    pub bytes: String,
    pub ret: ReturnReceipt,
    pub is_raptor_q_packet:bool
}

/// An acknowledge message sent back to the sender of a packet
/// in response to a return receipt being required by a given packet
/// Ack messages contain the packet's common, derived id that identifies
/// which message the packet was derived from, the packet number of the
/// packet being acknowledged and src a byte representation of a socket address.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AckMessage {
    pub packet_id: InnerKey,
    pub packet_number: usize,
    pub src: AddressBytes,
}

/// Headers to identify and route messages to the proper component
/// Request, Response, Gossip and Ack allows for easy routing once
/// the packets have been received and aggregated into a message
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Header {
    Request,
    Response,
    Gossip,
    Ack,
    RaptorQGossip,
}
/// A message struct contains a header and message data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub head: Header,
    pub msg: MessageData,
}

/// A tuple struct containing a byte representation of a 256 bit key
#[derive(
    Ord, PartialOrd, PartialEq, Eq, Clone, Hash, Serialize, Deserialize, Default, Copy, Debug,
)]
pub struct MessageKey(InnerKey);

/// TODO: Create a trait called Key to apply to all the different types of keys with the same required
/// functionality
impl MessageKey {
    /// generates a new Message Key from a 32 byte array representing a 256 bit key
    pub fn new(v: InnerKey) -> Self {
        MessageKey(v)
    }

    /// Generate a random key
    pub fn rand() -> Self {
        let mut ret = MessageKey([0; 32]);
        ret.0.iter_mut().for_each(|k| {
            *k = rand::random::<u8>();
        });

        ret
    }

    /// generate a random key within a given range
    pub fn rand_in_range(idx: usize) -> Self {
        let mut ret = MessageKey::rand();
        let bytes = idx / 8;
        let bit = idx % 8;
        (0..bytes).into_iter().for_each(|i| {
            ret.0[i] = 0;
        });
        ret.0[bytes] &= 0xFF >> (bit);
        ret.0[bytes] |= 1 << (8 - bit - 1);

        ret
    }

    /// Return the inner key
    pub fn inner(&self) -> InnerKey {
        self.0
    }

    /// Return the Message Key given an Inner key
    pub fn from_inner(v: InnerKey) -> MessageKey {
        MessageKey(v)
    }
}

/// A message for the Kademlia DHT protocol
/// 3 different variants, Request, Response, and Kill
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KadMessage {
    Request(RequestBytes),
    Response(ResponseBytes),
    Kill,
}

/// `split_into_packets` takes a `full_list` of bytes, a `batch_id` and an `erasure_count` and returns a
/// `Vec<Vec<u8>>` of packets
///
/// Arguments:
///
/// * `full_list`: The list of bytes to be split into packets
/// * `batch_id`: This is a unique identifier for the batch of packets.
/// * `erasure_count`: The number of packets that can be lost and still be able to recover the original data.
pub fn split_into_packets(
    full_list: &[u8],
    batch_id: [u8; BATCH_ID_SIZE],
    erasure_count: u32,
) -> Vec<Vec<u8>> {
    let packet_holder = encode_into_packets(full_list, erasure_count);
    let mut headered_packets: Vec<Vec<u8>> = vec![];
    for (_, ep) in packet_holder.into_iter().enumerate() {
        headered_packets.push(ep)
    }
    println!("Packets len {:?}", headered_packets.len());
    headered_packets
}

/// It takes a list of bytes and an erasure count, and returns a list of packets
///
/// Arguments:
///
/// * `unencoded_packet_list`: This is the list of packets that we want to encode.
/// * `erasure_count`: The number of packets that can be lost and still be able to recover the original
/// data.
///
/// Returns:
///
/// A vector of vectors of bytes.
pub fn encode_into_packets(unencoded_packet_list: &[u8], erasure_count: u32) -> Vec<Vec<u8>> {
    dbg!("inside encode pacjets");
    let encoder = Encoder::with_defaults(unencoded_packet_list, (PAYLOAD_SIZE) as u16);
    let packets: Vec<Vec<u8>> = encoder
        .get_encoded_packets(erasure_count)
        .iter()
        .map(|packet| packet.serialize())
        .collect();

    //Have to be transmitter to peers
    //println!("encoder config {:?}", encoder.get_config());
    println!("Packet size after raptor: {}", packets[0].len());
    packets
}

/// It takes a batch id, a sequence number, and a payload, and returns a packet
///
/// Arguments:
///
/// * `batch_id`: This is the batch id that we're sending.
/// * `payload`: the data to be sent
///
/// Returns:
///
/// A vector of bytes
pub fn create_packet(batch_id: [u8; BATCH_ID_SIZE], payload: Vec<u8>) -> Vec<u8> {
    let mut mtu: Vec<u8> = vec![];

    // empty byte for raptor coding length
    // doing the plus one since raptor is returning minus 1 length.
    mtu.push(0_u8);
    // forward-flag at the beginning
    mtu.push(1_u8);

    for i in 0..BATCH_ID_SIZE {
        mtu.push(batch_id[i]);
    }
    mtu.extend_from_slice(&payload);
    mtu
}


pub fn get_batch_id(packet: &[u8; 1280]) -> [u8; BATCH_ID_SIZE] {
    let mut batch_id: [u8; BATCH_ID_SIZE] = [0; BATCH_ID_SIZE];
    let mut chunk_no: usize = 0;
    for i in 3..(BATCH_ID_SIZE + 3) {
        batch_id[chunk_no] = packet[i];
        chunk_no += 1;
    }
    batch_id
}

pub fn get_packet_id(packet: &Packet) -> InnerKey {
    return packet.id;
}
