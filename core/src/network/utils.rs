use libp2p::{gossipsub::IdentTopic, Multiaddr, PeerId};

pub enum SwarmMessageType {
    Publish(IdentTopic, String),
    AddAddress(PeerId, Multiaddr),
    Bootstrap,
}
