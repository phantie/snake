use super::lobbies::LobbiesMsg;
use super::lobby_con_state::LobbyConState;
use super::state::{LobbyState, PrepLobbyState, RunningLobbyState};
use crate::mp::{domain, Ch, Con, LobbyName, MsgId, ServerMsg, UserName, WsMsg};
use std::collections::HashMap;

pub struct Lobby {
    pub name: LobbyName,
    pub players: HashMap<Con, LobbyConState>,
    pub state: LobbyState,

    ch: Option<tokio::sync::mpsc::UnboundedSender<LobbyCtrlMsg>>,
    // TODO maybe ship with RunningLobbyState
    _loop_handle: Option<tokio::task::AbortHandle>,
}

impl Lobby {
    pub fn new(name: LobbyName) -> Self {
        Self {
            name,
            players: Default::default(),
            state: LobbyState::Prep(PrepLobbyState::default()),

            ch: None,
            _loop_handle: None,
        }
    }

    pub fn begin(&mut self) -> Result<(), String> {
        match &mut self.state {
            LobbyState::Prep(s) => {
                self.state = LobbyState::Running(s.to_running());

                let ch = self.ch.clone().expect("set up channel");
                self._loop_handle.replace(
                    tokio::spawn(async move {
                        // TODO should be swaped, or added larger pause before loop
                        loop {
                            ch.send(LobbyCtrlMsg::LobbyMsg(LobbyMsg::Advance)).unwrap();
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    })
                    .abort_handle(),
                );

                Ok(())
            }
            _ => Err("Illegal state".into()),
        }
    }

    pub fn stop(&mut self) {
        match &self.state {
            LobbyState::Running { .. } => {
                self._loop_handle.take().expect("set up channel").abort();
                self.ch.take();

                self.state = LobbyState::Terminated;
            }
            _ => {}
        }
    }

    pub fn vote_start(&mut self, con: Con, value: bool) -> Result<(), String> {
        match &mut self.state {
            LobbyState::Prep(s) => {
                s.vote_start(con, value);
                if s.all_voted_to_start() {
                    self.begin().unwrap();
                }

                Ok(())
            }
            _ => Err("Illegal state".into()),
        }
    }

    pub fn set_con_direction(
        &mut self,
        con: Con,
        direction: domain::Direction,
    ) -> Result<(), String> {
        match &mut self.state {
            LobbyState::Running(s) => {
                s.set_con_direction(con, direction);
                Ok(())
            }
            _ => Err("Illegal state".into()),
        }
    }

    pub fn join_con(&mut self, con: Con, ch: Ch, un: UserName) -> Result<(), String> {
        match &mut self.state {
            LobbyState::Prep(s) => {
                self.players.insert(con, LobbyConState::new(ch, un));
                s.join_con(con);
                Ok(())
            }
            _ => Err("Illegal state".into()),
        }
    }

    pub fn disjoin_con(&mut self, con: &Con) {
        self.players.remove(&con);
        match &mut self.state {
            LobbyState::Prep(s) => {
                s.remove_con(con);
            }

            LobbyState::Running(s) => {
                // when everyone quits from running game, remove lobby
                if self.players.is_empty() {
                    let send = LobbiesMsg::RemoveLobby(self.name.clone());
                    self.ch
                        .as_ref()
                        .unwrap()
                        .send(LobbyCtrlMsg::LobbiesMsg(send))
                        .unwrap();
                }
                s.remove_con(con);
            }

            LobbyState::Terminated => {}
        }
    }
}

// message passing impl
impl Lobby {
    #[must_use = "to use message passing"]
    pub fn set_ch(mut self, ch: tokio::sync::mpsc::UnboundedSender<LobbyCtrlMsg>) -> Self {
        self.ch.replace(ch);
        self
    }

    pub fn handle_message(&mut self, msg: LobbyMsg) {
        match &mut self.state {
            LobbyState::Prep { .. } => {
                tracing::warn!("unhandled message {msg:?}")
            }
            LobbyState::Running(s) => match msg {
                LobbyMsg::Advance => {
                    s.advance();
                    self.broadcast_state();
                }
            },
            LobbyState::Terminated => {
                tracing::warn!("unhandled message {msg:?}")
            }
        }
    }
}

// broadcast impl
impl Lobby {
    pub fn broadcast_state(&self) {
        let send = |con| WsMsg::new(interfacing::snake::WsServerMsg::LobbyState(self.state(con)));

        self.players
            .iter()
            .for_each(|(_con, LobbyConState { ch, .. })| ch.send(send(*_con)).unwrap_or(()));
    }

    // include Id for the participant who's request triggered broadcast
    pub fn pinned_broadcast_state(&self, pin: MsgId, con: Con) {
        let send = |con| WsMsg::new(interfacing::snake::WsServerMsg::LobbyState(self.state(con)));

        self.players
            .iter()
            .filter(|(_con, _)| con == **_con)
            .for_each(|(_con, LobbyConState { ch, .. })| {
                ch.send(send(*_con).id(pin.clone())).unwrap_or(())
            });

        self.players
            .iter()
            .filter(|(_con, _)| con != **_con)
            .for_each(|(_con, LobbyConState { ch, .. })| ch.send(send(*_con)).unwrap_or(()));
    }

    pub fn broadcast_state_except(&self, con: Con) {
        let send = |con| WsMsg::new(interfacing::snake::WsServerMsg::LobbyState(self.state(con)));

        self.players
            .iter()
            .filter(|(_con, _)| con != **_con)
            .for_each(|(_con, LobbyConState { ch, .. })| ch.send(send(*_con)).unwrap_or(()));
    }

    /// Broadcast message to all lobby participants
    #[allow(unused)]
    fn broadcast(&self, msg: ServerMsg) {
        self.players
            .values()
            .for_each(|LobbyConState { ch, .. }| ch.send(msg.clone()).unwrap_or(()));
    }
}

// to ser/de impl
impl Lobby {
    pub fn state(&self, receiver: Con) -> interfacing::snake::LobbyState {
        use interfacing::snake::lobby_state::{LobbyPrep, LobbyPrepParticipant};

        match &self.state {
            // TODO it cannot impl From because State itself participates in calculation
            // one way would be to duplicate user_names to PrepLobbyState
            LobbyState::Prep(PrepLobbyState { start_votes }) => {
                interfacing::snake::LobbyState::Prep(LobbyPrep {
                    participants: self
                        .players
                        .iter()
                        .map(|(con, LobbyConState { un, .. })| LobbyPrepParticipant {
                            user_name: un.clone(),
                            vote_start: *start_votes.get(&con).expect("to be in sync"),
                        })
                        .collect(),
                })
            }

            LobbyState::Running(RunningLobbyState {
                snakes,
                foods,
                boundaries,
                counter,
                cons,
                ..
            }) => {
                use interfacing::snake::lobby_state::LobbyRunning;

                let con: Con = receiver;

                let snake = snakes
                    .into_iter()
                    .find(|(_con, _)| **_con == con)
                    .map(|(_, snake)| snake.clone());

                let other_snakes = snakes
                    .into_iter()
                    .filter(|(_con, _)| **_con != con)
                    .map(|(_, snake)| snake.clone())
                    .collect::<Vec<_>>();

                interfacing::snake::LobbyState::Running(LobbyRunning {
                    counter: *counter,
                    player_counter: cons.len() as _,
                    domain: domain::Domain {
                        snake,
                        foods: foods.clone(),
                        other_snakes,
                        boundaries: *boundaries,
                    },
                })
            }
            LobbyState::Terminated => interfacing::snake::LobbyState::Terminated,
        }
    }
}

#[derive(Debug)]
pub enum LobbyMsg {
    Advance,
}

pub enum LobbyCtrlMsg {
    LobbyMsg(LobbyMsg),
    LobbiesMsg(LobbiesMsg),
}
