//! KaKa core: everything except the UI. Linux only, no network access.
//!
//! Users add their own Windows trainers; KaKa runs them inside the Proton
//! prefix of the game that is already running. Nothing here downloads or
//! auto-launches anything, and processes are always started with argument
//! lists, never through a shell.

pub mod game_info;
pub mod language;
pub mod model;
pub mod proton;
pub mod storage;
pub mod trainer_data;
