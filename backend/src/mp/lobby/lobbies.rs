use super::lobby::{Lobby, LobbyCtrlMsg};
use crate::mp::{Ch, Con, LobbyName, UserName};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

type LobbyMessagePasserAbortHandle = tokio::task::AbortHandle;

type ThreadSafeLobby = Arc<RwLock<Lobby>>;

#[derive(Clone, Default)]
pub struct Lobbies(
    Arc<RwLock<HashMap<LobbyName, ThreadSafeLobby>>>,
    Arc<RwLock<HashMap<Con, LobbyName>>>,
    Arc<RwLock<HashMap<LobbyName, LobbyMessagePasserAbortHandle>>>,
);

impl Lobbies {
    pub async fn lobby_names(&self) -> Vec<LobbyName> {
        self.0.read().await.keys().cloned().into_iter().collect()
    }

    #[allow(dead_code)]
    pub async fn lobby_state(&self, con: Con) -> Option<interfacing::snake::LobbyState> {
        match self.joined_lobby(con).await {
            None => None, // player not in any lobby
            Some(lobby) => Some(lobby.read().await.state(con)),
        }
    }

    pub async fn joined_lobby(&self, con: Con) -> Option<ThreadSafeLobby> {
        match self.1.read().await.get(&con) {
            None => None, // player not in any lobby
            Some(ln) => Some(self.0.read().await[ln].clone()),
        }
    }

    pub async fn joined_any(&self, con: Con) -> bool {
        self.joined_lobby(con).await.is_some()
    }

    // Remove lobby if exists
    pub async fn remove_lobby(&self, lobby_name: LobbyName) {
        // while you hold this lock, noone else touches players
        let mut con_to_lobby = self.1.write().await;

        {
            let _lock = self.0.read().await;

            let lobby = match _lock.get(&lobby_name) {
                None => return,
                Some(lobby) => lobby,
            };

            let mut _lobby_lock = lobby.write().await;

            _lobby_lock.stop();

            let players = &_lobby_lock.players;

            for (con, _) in players {
                con_to_lobby.remove(con);
            }
        }

        self.0.write().await.remove(&lobby_name);
        self.2
            .write()
            .await
            .remove(&lobby_name)
            .expect("to be in sync")
            .abort();
    }

    pub async fn disjoin_con(&self, con: Con) {
        // while you hold this lock, noone else touches players
        let mut con_to_lobby = self.1.write().await;

        match con_to_lobby.get(&con) {
            None => {}
            Some(_lobby_name) => {
                let _lock = self.0.read().await;
                let lobby = _lock.get(_lobby_name).expect("to be in sync");
                con_to_lobby.remove(&con);
                lobby.write().await.disjoin_con(&con);
                lobby.read().await.broadcast_state();
            }
        }
    }

    /// Try join con to specified lobby
    /// Con associates with
    ///     - Ch (WsServerMessage channel)
    ///     - UserName (Cannot be changed while in lobby)
    /// On success return lobby state, as an informative Ack
    pub async fn join_con(
        &self,
        lobby_name: LobbyName,
        con: Con,
        ch: Ch,
        un: UserName,
    ) -> Result<interfacing::snake::LobbyState, JoinLobbyError> {
        // while you hold this lock, noone else touches players
        let mut con_to_lobby = self.1.write().await;

        match con_to_lobby.get(&con) {
            None => {
                let _lock = self.0.read().await;
                let lobby = _lock.get(&lobby_name);

                match lobby {
                    None => Err(JoinLobbyError::NotFound),
                    Some(lobby) => {
                        con_to_lobby.insert(con.clone(), lobby_name);
                        let mut lock = lobby.write().await;
                        match lock.join_con(con, ch, un) {
                            Ok(()) => {
                                lock.broadcast_state_except(con);
                                Ok(lock.state(con))
                            }
                            Err(_m) => Err(JoinLobbyError::AlreadyStarted),
                        }
                    }
                }
            }
            Some(_lobby_name) => {
                // idempotency
                if lobby_name == *_lobby_name {
                    // don't need to check lobby, since it must be in sync

                    Ok(self
                        .get(_lobby_name)
                        .await
                        .unwrap() // TODO verify no in between changes
                        .read()
                        .await
                        .state(con))
                } else {
                    Err(JoinLobbyError::AlreadyJoined(lobby_name.clone()))
                }
            }
        }
    }

    /// Get lobby by name
    pub async fn get(&self, name: &LobbyName) -> Option<ThreadSafeLobby> {
        self.0.read().await.get(name).cloned()
    }

    /// Create lobby only if it's not already created
    pub async fn insert_if_missing(&self, lobby: Lobby) -> Result<(), String> {
        use std::collections::hash_map::Entry;
        let mut w_lock = self.0.write().await;

        match w_lock.entry(lobby.name.clone()) {
            Entry::Occupied(_) => Err("Lobby with this name already exists".into()),
            Entry::Vacant(_) => {
                let lobby_name = lobby.name.clone();

                let (s, mut r) = tokio::sync::mpsc::unbounded_channel::<LobbyCtrlMsg>();
                let lobby = Arc::new(RwLock::new(lobby.set_ch(s)));

                {
                    w_lock.insert(lobby_name.clone(), lobby.clone());
                }

                {
                    let lobbies = self.clone();
                    let lobby_msg_passer_handle = tokio::spawn(async move {
                        while let Some(msg) = r.recv().await {
                            match msg {
                                LobbyCtrlMsg::LobbyMsg(msg) => {
                                    lobby.write().await.handle_message(msg)
                                }
                                LobbyCtrlMsg::LobbiesMsg(msg) => match msg {
                                    LobbiesMsg::RemoveLobby(ln) => {
                                        lobbies.remove_lobby(ln).await;
                                    }
                                },
                            }
                        }
                    })
                    .abort_handle();
                    self.2
                        .write()
                        .await
                        .insert(lobby_name, lobby_msg_passer_handle);
                }

                Ok(())
            }
        }
    }
}

pub enum JoinLobbyError {
    // to the other
    AlreadyJoined(LobbyName),
    NotFound,
    AlreadyStarted,
}

// internal use messages sent from Lobby
pub enum LobbiesMsg {
    RemoveLobby(LobbyName),
}
