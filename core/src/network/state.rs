use libp2p::{gossipsub::IdentTopic, Multiaddr, PeerId};

use crate::pb::query::StartRequest;

impl StartRequest {
    pub fn with_white_player(self, white_player: String) -> Self {
        Self {
            white_player,
            ..self
        }
    }

    pub fn with_black_player(self, black_player: String) -> Self {
        Self {
            black_player,
            ..self
        }
    }
}

pub enum SwarmMessageType {
    Publish(IdentTopic, String),
    AddAddress(PeerId, Multiaddr),
    Bootstrap,
}
