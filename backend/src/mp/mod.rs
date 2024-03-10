use interfacing::snake::{LobbyName, MsgId, UserName, WsMsg};

pub use interfacing::snake_domain as domain;

// could be any, granting uniqueness to ws connection among all
pub type Con = u16;

type ServerMsg = WsMsg<interfacing::snake::WsServerMsg>;
type Ch = tokio::sync::mpsc::UnboundedSender<ServerMsg>;

pub mod con_state;
pub mod lobby;
