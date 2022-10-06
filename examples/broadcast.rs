use rand::{thread_rng, Rng};
use std::collections::{HashMap, HashSet};
use std::env::args;
use std::fs::File;
use std::io::Read;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use std::{fs, thread};
use tokio::io::split;
use udp2p::discovery::kad::Kademlia;
use udp2p::discovery::routing::RoutingTable;
use udp2p::gossip::gossip::{GossipConfig, GossipService};
use udp2p::gossip::protocol::GossipMessage;
use udp2p::node::peer_id::PeerId;
use udp2p::node::peer_info::PeerInfo;
use udp2p::node::peer_key::Key;
use udp2p::protocol::protocol::{
    packetize, split_into_packets, AckMessage, Header, Message, MessageKey,
};
use udp2p::transport::handler::MessageHandler;
use udp2p::transport::transport::Transport;
use udp2p::utils::utils::ByteRep;

#[allow(unused_variables, unused_mut)]
#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bind a UDP Socket to a Socket Address with a random port between
    // 9292 and 19292 on the localhost address.

    let port: usize = thread_rng().gen_range(9292..19292);
    let pub_ip = public_ip::addr_v4().await;
    let addr: SocketAddr = format!("0.0.0.0:{:?}", port)
        .parse()
        .expect("Unable to parse address");
    let sock: UdpSocket = UdpSocket::bind(addr).expect("Unable to bind to address");

    // Initiate channels for communication between different threads
    let (to_transport_tx, to_transport_rx): (
        Sender<(SocketAddr, Message)>,
        Receiver<(SocketAddr, Message)>,
    ) = channel();
    let (to_gossip_tx, to_gossip_rx) = channel();
    let (to_kad_tx, to_kad_rx) = channel();
    let (incoming_ack_tx, incoming_ack_rx): (Sender<AckMessage>, Receiver<AckMessage>) = channel();
    let (to_app_tx, _to_app_rx) = channel::<GossipMessage>();

    // Initialize local peer information
    let key: Key = Key::rand();
    let id: PeerId = PeerId::from_key(&key);
    let info: PeerInfo = PeerInfo::new(id, key, pub_ip.clone().unwrap(), port as u32);

    // initialize a kademlia, transport and message handler instance
    let routing_table = RoutingTable::new(info.clone());
    let ping_pong = Instant::now();
    let interval = Duration::from_secs(20);

    println!("Neighbours {:?}", routing_table.get_all_peers().clone());
    let mut kad = Kademlia::new(
        routing_table.clone(),
        to_transport_tx.clone(),
        to_kad_rx,
        HashSet::new(),
        interval,
        ping_pong.clone(),
    );
    if args().len() >= 2 {
        let data = args().nth(1).unwrap().clone();
        let v: Vec<&str> = data.split(":").collect();
        dbg!("here");
        let key1: Key = Key::rand();
        let id1: PeerId = PeerId::from_key(&key);
        let info1: PeerInfo = PeerInfo::new(
            id1,
            key1,
            pub_ip.clone().unwrap(),
            v.get(1).unwrap().parse::<u32>().unwrap(),
        );
        dbg!("here2");
        kad.add_peer(info1.as_bytes().unwrap());
    }

    let mut transport = Transport::new(info.address.clone(), incoming_ack_rx, to_transport_rx);

    let mut message_handler = MessageHandler::new(
        to_transport_tx.clone(),
        incoming_ack_tx.clone(),
        HashMap::new(),
        to_kad_tx.clone(),
        to_gossip_tx.clone(),
    );
    let protocol_id = String::from("vrrb-0.1.0-test-net");
    let gossip_config = GossipConfig::new(
        protocol_id,
        8,
        3,
        8,
        3,
        12,
        3,
        0.4,
        Duration::from_millis(250),
        80,
    );
    let heartbeat = Instant::now();
    let ping_pong = Instant::now();
    let mut gossip = GossipService::new(
        addr.clone(),
        info.address.clone(),
        to_gossip_rx,
        to_transport_tx.clone(),
        to_app_tx.clone(),
        kad,
        gossip_config,
        heartbeat,
        ping_pong,
    );

    // Inform the local node of their address (since the port is randomized)
    println!("My Address: {:?}", addr);
    println!("My ID: {:?}", info.id);
    // Clone the socket for the transport and message handling thread(s)
    let local = gossip.get_pub_ip().clone();
    let thread_sock = sock.try_clone().expect("Unable to clone socket");
    thread::spawn(move || {
        let inner_sock = thread_sock.try_clone().expect("Unable to clone socket");
        thread::spawn(move || loop {
            transport.incoming_ack();
            transport.outgoing_msg(&inner_sock);
            transport.check_time_elapsed(&inner_sock);
        });

        loop {
            let local = local.clone();
            let mut buf = [0u8; 65536];
            message_handler.recv_msg(&thread_sock, &mut buf, local);
        }
    });

    if let Some(to_dial) = args().nth(1) {
        let bootstrap: SocketAddr = to_dial.parse().expect("Unable to parse address");
        gossip.kad.bootstrap(&bootstrap);
        if let Some(bytes) = info.as_bytes() {
            gossip.kad.add_peer(bytes)
        }
    }
    for arg in args().skip(2) {
        let key: Key = Key::rand();
        let id: PeerId = PeerId::from_key(&key);
        let info: PeerInfo = PeerInfo::new(id, key, pub_ip.clone().unwrap(), port as u32);
        if let Some(bytes) = info.as_bytes() {
            gossip.kad.add_peer(bytes)
        }
    }
    println!(
        "Neighbours {:?}",
        gossip.kad.routing_table.get_all_peers().len()
    );
    dbg!("here3");
    println!("Neighbours {:?}", gossip.kad.routing_table.get_all_peers());

    dbg!("here4");
    let thread_to_gossip = to_gossip_tx.clone();
    dbg!("here5");

    //line 196 executes before be begin spawning new threads
    thread::spawn(move || {
        if args().len() >= 2 {
            let mut line = String::new();
            dbg!("here6");
            let raw_contents = read_file(PathBuf::from("src/record/record.rs"));
            println!("HELLO {:?}",raw_contents);
            let input = std::io::stdin().read_line(&mut line);
            println!("INPUT {:?}", input);
            dbg!("here7");
            let msg_id = MessageKey::rand();
            let msg = GossipMessage {
                id: msg_id.inner(),
                data: raw_contents,
                sender: addr.clone(),
            };
            let message = Message {
                head: Header::RaptorQGossip,
                msg: msg.as_bytes().unwrap(),
            };
            dbg!("here8");
            if let Err(_) = thread_to_gossip.clone().send((addr.clone(), message)) {
                println!("Error sending message to gossip")
            }
            dbg!("here9");
        }
    });

    dbg!("here10");
    gossip.start(to_app_tx.clone());
    dbg!("here11");

    Ok(())
}

pub fn read_file(file_path: PathBuf) -> Vec<u8> {
    let metadata = fs::metadata(&file_path).expect("unable to read metadata");
    let mut buffer = vec![0; metadata.len() as usize];
    File::open(file_path)
        .unwrap()
        .read_exact(&mut buffer)
        .expect("buffer overflow");
    buffer
}

//GossipMessage For PeerID

//PeerID -- XOR Distance To all the peers within bootstrap node

//Top K neigbours (k-nn algorithm)

//Top K PeerIds

// ID1--->(OthersIds add to routing table)

// Check if decoder hash has taht packet ID
//If not create decoderhash for that packet ID

//
