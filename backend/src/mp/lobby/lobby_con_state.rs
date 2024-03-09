pub struct LobbyConState {
    pub ch: Ch,
    pub un: UserName,
}

impl LobbyConState {
    pub fn new(ch: Ch, un: UserName) -> Self {
        Self { ch, un }
    }
}

use crate::mp::{Ch, UserName};
