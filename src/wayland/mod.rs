//! The compositor connection and the globals the daemon binds.

pub mod device;
pub mod offer;
pub mod source;

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry::WlRegistry, wl_seat::WlSeat},
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;

use crate::{Daemon, Error, Result};

pub struct Globals {
    pub manager: ExtDataControlManagerV1,
    pub seat: WlSeat,
}

pub fn connect() -> Result<(Connection, EventQueue<Daemon>, Globals)> {
    let connection = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<Daemon>(&connection)?;
    let queue_handle = queue.handle();
    let manager = globals
        .bind(&queue_handle, 1..=1, ())
        .map_err(|_| Error::DataControlUnsupported)?;
    let seat = globals
        .bind(&queue_handle, 1..=1, ())
        .map_err(|_| Error::NoSeat)?;
    Ok((connection, queue, Globals { manager, seat }))
}

impl Dispatch<WlRegistry, GlobalListContents> for Daemon {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(Daemon: ignore WlSeat);
delegate_noop!(Daemon: ExtDataControlManagerV1);
