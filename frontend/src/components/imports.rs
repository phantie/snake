#![allow(unused)]
pub use crate::components::theme::prelude::*;
pub use crate::components::DefaultStyling;
pub use crate::components::PageTitle;
pub use crate::router::Route;
pub use static_routes::*;

pub use std::collections::HashMap;
pub use std::rc::Rc;

pub use gloo_console as console;
pub use gloo_net::http::{Request, Response};
pub use serde::{Deserialize, Serialize};
pub use stylist::yew::{styled_component, Global};
pub use stylist::{css, style, Style};
pub use web_sys::{HtmlElement, HtmlInputElement};
pub use yew::prelude::*;
pub use yew_router::prelude::*;

pub trait RequestExtend {
    fn static_get(static_path: impl Get) -> Self;
    fn static_post(static_path: impl Post) -> Self;
}

impl RequestExtend for Request {
    fn static_get(static_path: impl Get) -> Self {
        Request::get(static_path.get().complete())
    }

    fn static_post(static_path: impl Post) -> Self {
        Request::post(static_path.post().complete())
    }
}

pub trait ResponseExtend {
    fn log_status(&self);
}

impl ResponseExtend for Response {
    fn log_status(&self) {
        console::log!(format!("{} status {}", self.url(), self.status()));
    }
}

pub mod request {
    pub type SendResult = std::result::Result<gloo_net::http::Response, gloo_net::Error>;
}

pub fn internal_problems() -> Html {
    html! {
        <DefaultStyling>
            <Global css={ "display: flex; justify-content: center;" }/>
            <h1>{ "Ooops... internal problems" }</h1>
         </DefaultStyling>
    }
}
