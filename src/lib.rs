#![allow(dead_code)]

#[cfg(feature = "audio")]
pub mod audio;
pub mod configuration;
#[cfg(feature = "mqtt")]
pub mod mqtt;

pub mod chat_manager;
pub mod cli_history;
pub mod utils;
