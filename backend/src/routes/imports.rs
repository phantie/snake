// #![allow(unused)]
// pub use crate::static_routes::extend::*;

#[allow(unused)]
pub use crate::{
    conf::get_env,
    error::{ApiError, ApiResult},
};
#[allow(unused)]
pub use axum::{
    extract::{Extension, Form, Json, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
pub use interfacing;
pub use static_routes::*;
