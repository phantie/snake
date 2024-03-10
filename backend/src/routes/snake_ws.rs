use crate::routes::imports::*;
use crate::server::UserConnectInfo;
use axum::extract::{
    connect_info::ConnectInfo,
    ws::{Message, WebSocket, WebSocketUpgrade},
};
use futures_util::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use interfacing::snake::WsMsg;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::mp::{
    con_state::ConState,
    lobby::{
        lobbies::{JoinLobbyError, Lobbies},
        lobby::Lobby,
        usernames::PlayerUserNames,
    },
    Con,
};

// for debugging
const AUTO_GEN_USER_NAME: bool = false;

pub async fn ws(
    maybe_ws: Result<WebSocketUpgrade, axum::extract::ws::rejection::WebSocketUpgradeRejection>,
    ConnectInfo(con_info): ConnectInfo<UserConnectInfo>,
    headers: hyper::HeaderMap,
    Extension(lobbies): Extension<Lobbies>,
    Extension(uns): Extension<PlayerUserNames>,
) -> Response {
    let ws = match maybe_ws {
        Ok(ws) => ws,
        Err(e) => {
            tracing::trace!("{headers:?}");
            tracing::error!("{e}");
            return e.into_response();
        }
    };

    let sock_addr = con_info.socket_addr(&headers);
    if Env::current().local() {
        tracing::info!("Client connected to Snake Ws: {:?}", sock_addr);
    } else {
        tracing::info!("Client connected to Snake Ws");
    }

    // connection identifier,
    // expected to be unique across current state
    let con = sock_addr.port();

    ws.on_upgrade(move |socket| handle_socket(socket, con, lobbies, uns))
}

type ClientMsg = WsMsg<interfacing::snake::WsClientMsg>;
type ServerMsg = WsMsg<interfacing::snake::WsServerMsg>;

async fn handle_socket(socket: WebSocket, con: Con, lobbies: Lobbies, uns: PlayerUserNames) {
    let con_state = {
        let mut con_state = ConState::default();

        con_state.un = if AUTO_GEN_USER_NAME && Env::current().local() {
            let un = format!("Player {con}");
            // do not handle possible collision, since it's debug only feature
            uns.try_insert(un.clone(), con).await.unwrap();
            Some(un)
        } else {
            None
        };

        Arc::new(Mutex::new(con_state))
    };

    let (server_msg_sender, server_msg_receiver) = mpsc::unbounded_channel::<ServerMsg>();

    let (sender, receiver) = socket.split();
    let rh = tokio::spawn(read(
        receiver,
        con_state.clone(),
        server_msg_sender.clone(),
        lobbies.clone(),
        con.clone(),
        uns.clone(),
    ));
    let wh = tokio::spawn(write(sender, server_msg_receiver));

    // as soon as a closed channel error returns from any of these procedures,
    // cancel the other
    () = tokio::select! {
        _ = rh => (),
        _ = wh => (),
    };

    // TODO investigate when port becomes free
    // undefined behavior is possible, if port can become free before this sections running
    //
    // clean up
    lobbies.disjoin_con(con).await;
    uns.clean_con(con).await;
}

async fn read(
    mut receiver: SplitStream<WebSocket>,
    con_state: Arc<Mutex<ConState>>,
    server_msg_sender: mpsc::UnboundedSender<ServerMsg>,
    lobbies: Lobbies,
    con: Con,
    uns: PlayerUserNames,
) {
    loop {
        match receiver.next().await {
            Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMsg>(&text) {
                Ok(msg) => {
                    tracing::info!("Received message: {text:?}");
                    tokio::task::spawn(handle_received_message(
                        msg,
                        con_state.clone(),
                        server_msg_sender.clone(),
                        lobbies.clone(),
                        con.clone(),
                        uns.clone(),
                    ));
                }
                Err(_) => {
                    tracing::info!("Received unexpected message: {text:?}");
                }
            },
            Some(Ok(msg)) => {
                tracing::info!("Received unhandled message: {msg:?}");
            }
            Some(Err(_)) => {
                tracing::info!("Client disconnected");
                return;
            }
            None => {
                tracing::info!("Broadcast channel closed");
                return;
            }
        }
    }
}

async fn handle_received_message(
    msg: ClientMsg,
    con_state: Arc<Mutex<ConState>>,
    server_msg_sender: mpsc::UnboundedSender<ServerMsg>,
    lobbies: Lobbies,
    con: Con,
    uns: PlayerUserNames,
) {
    use interfacing::snake::{PinnedMessage, WsClientMsg::*, WsServerMsg};

    let con = con;

    match msg {
        WsMsg(Some(id), CreateLobby(value)) => {
            let lobby = Lobby::new(value);
            let result = lobbies.insert_if_missing(lobby).await;

            let send = match result {
                Ok(()) => WsServerMsg::Ack,
                Err(msg) => WsServerMsg::Err(msg),
            };

            server_msg_sender.send(id.pinned_msg(send)).unwrap();
        }

        WsMsg(Some(id), SetUserName(value)) => {
            let send = if lobbies.joined_any(con).await {
                // forbid name changing when joined lobby
                interfacing::snake::WsServerMsg::ForbiddenWhenJoined
            } else {
                match uns.try_insert(value.clone(), con).await {
                    Ok(()) => {
                        con_state.lock().await.un.replace(value);
                        WsServerMsg::Ack
                    }
                    Err(()) => interfacing::snake::WsServerMsg::UserNameOccupied,
                }
            };
            server_msg_sender.send(id.pinned_msg(send)).unwrap();
        }

        WsMsg(Some(id), UserName) => {
            let un = con_state.lock().await.un.clone();
            let send = interfacing::snake::WsServerMsg::UserName(un);
            server_msg_sender.send(id.pinned_msg(send)).unwrap();
        }

        WsMsg(Some(id), JoinLobby(lobby_name)) => {
            use interfacing::snake::JoinLobbyDecline;

            let send = match &con_state.lock().await.un {
                None => interfacing::snake::WsServerMsg::JoinLobbyDecline(
                    interfacing::snake::JoinLobbyDecline::UserNameNotSet,
                ),
                Some(un) => {
                    match lobbies
                        .join_con(lobby_name, con, server_msg_sender.clone(), un.clone())
                        .await
                    {
                        Ok(s) => WsServerMsg::LobbyState(s),

                        Err(e) => {
                            use JoinLobbyError::*;
                            // TODO impl From or use the same datastructure
                            let e = match e {
                                NotFound => JoinLobbyDecline::NotFound,
                                AlreadyJoined(lobby_name) => {
                                    JoinLobbyDecline::AlreadyJoined(lobby_name)
                                }
                                AlreadyStarted => JoinLobbyDecline::AlreadyStarted,
                            };
                            WsServerMsg::JoinLobbyDecline(e)
                        }
                    }
                }
            };

            server_msg_sender.send(id.pinned_msg(send)).unwrap();
        }

        WsMsg(Some(id), LobbyList) => {
            let lobby_list = lobbies
                .lobby_names()
                .await
                .into_iter()
                .map(|name| interfacing::snake::list::Lobby { name })
                .collect::<Vec<_>>();

            let send = WsServerMsg::LobbyList(lobby_list);
            server_msg_sender.send(id.pinned_msg(send)).unwrap();
        }

        WsMsg(Some(id), VoteStart(value)) => {
            let lobby = lobbies.joined_lobby(con).await;

            match lobby {
                None => {
                    let send = WsServerMsg::Err("lobby does not exist".into());
                    server_msg_sender.send(id.pinned_msg(send)).unwrap();
                }
                Some(lobby) => {
                    let mut lock = lobby.write().await;
                    let result = lock.vote_start(con, value);

                    match result {
                        Ok(()) => {
                            lock.pinned_broadcast_state(id, con);
                        }
                        Err(m) => {
                            // this branch handling is required because
                            // it's possible that between lobbies.joined_lobby and lobby.vote_start
                            // player leaves the lobby
                            let send = WsServerMsg::Err(m);
                            server_msg_sender.send(id.pinned_msg(send)).unwrap();
                        }
                    };
                }
            };
        }

        WsMsg(Some(id), LeaveLobby) => {
            lobbies.disjoin_con(con).await;
            server_msg_sender
                .send(id.pinned_msg(WsServerMsg::Ack))
                .unwrap();
        }

        WsMsg(Some(_id), SetDirection(_)) => {
            unreachable!("id not expected");
        }

        WsMsg(None, SetDirection(direction)) => {
            let lobby = lobbies.joined_lobby(con).await;

            if let Some(lobby) = lobby {
                lobby
                    .write()
                    .await
                    .set_con_direction(con, direction)
                    .unwrap_or(());
            }

            // do not send response
        }

        WsMsg(
            None,
            CreateLobby(_) | JoinLobby(_) | UserName | LobbyList | SetUserName(_) | VoteStart(_)
            | LeaveLobby,
        ) => {
            if Env::current().prod() {
                tracing::info!("ack expected")
            } else {
                unreachable!("ack expected")
            }
        }
    }
}

async fn write(
    mut sender: SplitSink<WebSocket, Message>,
    mut server_msg_receiver: mpsc::UnboundedReceiver<ServerMsg>,
) {
    while let Some(msg) = server_msg_receiver.recv().await {
        let msg = Message::Text(serde_json::to_string(&msg).unwrap());

        match sender.send(msg.clone()).await {
            Ok(()) => {
                tracing::info!("Sent message: {msg:?}")
            }
            Err(_) => {
                tracing::info!("Client disconnected");
                return;
            }
        }
    }
}
