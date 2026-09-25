//! crownos-clipboard — the CrownOS clipboard manager.
//!
//! The library target is the wire contract of the `crownos-clipboard`
//! crownos-ipc service; with `default-features = false` a client builds only
//! that. The `daemon` feature adds the ext-data-control client behind it.
//!
//! Payloads never travel in frames: `get` writes into a memfd the caller
//! passes in and replies with the payload's mime type.

use serde::{Deserialize, Serialize};

/// One history entry as a client sees it, newest first in [`protocol::list`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrySummary {
    pub id: u64,
    pub mimes: Vec<String>,
    pub bytes: u64,
    /// The start of the text payload, empty when the entry has no text.
    pub preview: String,
}

crownos_ipc::protocol! {
    pub mod protocol: "crownos-clipboard" {
        /// The history, newest first.
        fn list() -> Vec<EntrySummary>;
        /// Write entry `id`'s preferred payload into `sink` (a memfd or other
        /// regular file, truncated to fit) and reply with its mime type.
        fn get(id: u64) fds(sink) -> String;
        /// Make entry `id` the current clipboard and move it to the front.
        fn select(id: u64);
        /// Drop entry `id` from the history.
        fn delete(id: u64);
        /// Drop the whole history.
        fn clear();
        /// The history changed; call `list` for the new state.
        event Changed {};
    }
}

pub use protocol::{Changed, Client, SERVICE};

#[cfg(feature = "daemon")]
pub mod capture;
#[cfg(feature = "daemon")]
pub mod config;
#[cfg(feature = "daemon")]
pub mod daemon;
#[cfg(feature = "daemon")]
pub mod error;
#[cfg(feature = "daemon")]
pub mod ipc;
#[cfg(feature = "daemon")]
pub mod ownership;
#[cfg(feature = "daemon")]
pub mod serve;
#[cfg(feature = "daemon")]
pub mod store;
#[cfg(feature = "daemon")]
pub mod util;
#[cfg(feature = "daemon")]
pub mod wayland;

#[cfg(feature = "daemon")]
pub use config::Config;
#[cfg(feature = "daemon")]
pub use daemon::Daemon;
#[cfg(feature = "daemon")]
pub use error::{Error, Result};
