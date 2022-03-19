use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{Sender, Receiver, channel};
use udp2p::protocol::protocol::{AckMessage, Message, MessageKey, Header};
use udp2p::node::peer_id::PeerId;
use udp2p::node::peer_key::Key;
use udp2p::node::peer_info::PeerInfo;
use udp2p::discovery::kad::Kademlia;
use udp2p::discovery::routing::RoutingTable;
use udp2p::transport::transport::Transport;
use udp2p::transport::handler::MessageHandler;
use std::collections::{HashMap, HashSet};
use std::thread;
use std::env::args;
use udp2p::gossip::gossip::{GossipConfig, GossipService};
use udp2p::gossip::protocol::GossipMessage;
use rand::{thread_rng, Rng};
use std::time::{Duration, Instant};
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
    let (chat_tx, chat_rx) = channel::<GossipMessage>();

    // Initialize local peer information
    let key: Key = Key::rand();
    let id: PeerId = PeerId::from_key(&key);
    let info: PeerInfo = PeerInfo::new(id, key, pub_ip.clone().unwrap(), port as u32);

    // initialize a kademlia, transport and message handler instance
    let routing_table = RoutingTable::new(info.clone());
    let ping_pong = Instant::now();
    let interval = Duration::from_secs(20);
    let kad = Kademlia::new(routing_table, to_transport_tx.clone(), to_kad_rx, HashSet::new(), interval, ping_pong.clone());
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
    } else {
        if let Some(bytes) = info.as_bytes() {
            gossip.kad.add_peer(bytes)
        }
    }

    let thread_to_gossip = to_gossip_tx.clone();
    thread::spawn(move || {
        loop {
            let mut line = String::new();
            let input = std::io::stdin().read_line(&mut line);
            if let Ok(_) = input {
                let msg_id = MessageKey::rand();
                let msg = GossipMessage {
                    id: msg_id.inner(),
                    data: line.trim().as_bytes().to_vec(),
                    sender: addr.clone()
                };

                let message = Message {
                    head: Header::Gossip,
                    msg: msg.as_bytes().unwrap()
                };

                if let Err(_) = thread_to_gossip.clone().send((addr.clone(), message)) {
                    println!("Error sending message to gossip")
                }
            }        
        }
    });
    
    thread::spawn(move || {
        loop {
            match chat_rx.recv() {
                Ok(string) => println!("{:?}", string),
                Err(_) => {}
            }
        }
    });

    gossip.start(chat_tx.clone());

    Ok(())
}