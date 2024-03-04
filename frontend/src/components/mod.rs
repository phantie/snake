#![allow(non_upper_case_globals)]

// TODO switch.rs uses it, refactor
pub mod imports;

mod default_styling;
mod error;
pub mod snake;
mod title;

pub use default_styling::DefaultStyling;
pub use error::Error;
pub use snake::comp::Snake;
pub use title::PageTitle;

mod state;
pub mod theme;
