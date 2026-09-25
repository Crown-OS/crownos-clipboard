//! The crate's typed error.

use wayland_client::{ConnectError, globals::GlobalError};

use crate::store::EntryId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "the compositor does not offer ext_data_control_manager_v1 \
         (this client is sandboxed, or the compositor has no clipboard-manager support)"
    )]
    DataControlUnsupported,
    #[error("the compositor offers no wl_seat")]
    NoSeat,
    #[error("clipboard entry {0} does not exist")]
    NoSuchEntry(EntryId),
    #[error("clipboard entry {0} has no payload")]
    EmptyEntry(EntryId),
    #[error("the payload sink is not a memfd or regular file")]
    SinkNotAFile,
    #[error("the transfer exceeded the {0}-byte entry cap")]
    TooLarge(usize),
    #[error("the transfer timed out")]
    TimedOut,
    #[error(transparent)]
    Connect(#[from] ConnectError),
    #[error(transparent)]
    Globals(#[from] GlobalError),
    #[error(transparent)]
    EventLoop(#[from] calloop::Error),
    #[error(transparent)]
    Ipc(#[from] crownos_ipc::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
