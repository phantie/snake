use crate::router::Route;

use yew::prelude::*;

pub fn switch(routes: Route) -> Html {
    use crate::components::*;

    #[allow(unused)]
    enum Home {
        ArticleList,
        Snake,
    }

    use snake::comp::NotBegunState;

    match routes {
        Route::Home => html! { <Snake/> },
        Route::Snake => {
            html! {
                <Snake/>
            }
        }
        Route::SnakeCreateJoinLobby => {
            let state = NotBegunState::MPCreateJoinLobby;
            html! {
                <Snake {state}/>
            }
        }
        Route::SnakeLobbies => {
            let state = NotBegunState::MPSetUsername {
                next_state: Box::new(NotBegunState::MPCreateJoinLobby),
            };
            html! {
                <Snake {state}/>
            }
        }
        Route::SnakeCreateLobby => {
            let state = NotBegunState::MPCreateLobby;
            html! {
                <Snake {state}/>
            }
        }
        // TODO requires user_name setting
        Route::SnakeLobby { lobby_name } => {
            let state = NotBegunState::MPLobby {
                state: snake::comp::MPLobbyState::ToJoin { lobby_name },
            };
            html! {
                // TODO refactor
                <Snake state={ state }/>
            }
        }
        Route::NotFound => html! { <Error msg={"Not Found"} code=404 /> },
        Route::Unauthorized => html! { <Error msg={"Unauthorized"} code=401 /> },
    }
}
