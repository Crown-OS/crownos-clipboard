//! Our own sources: each serves one entry until the compositor cancels it.

use std::sync::Arc;

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::{
    self, ExtDataControlSourceV1,
};

use crate::{Daemon, serve, store::Entry};

impl Dispatch<ExtDataControlSourceV1, Arc<Entry>> for Daemon {
    fn event(
        daemon: &mut Self,
        source: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        entry: &Arc<Entry>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                match entry.payload(&mime_type) {
                    Some(payload) => serve::serve(daemon.loop_handle(), Arc::clone(payload), fd),
                    None => log::debug!("entry {} was never offered as {mime_type}", entry.id),
                }
            }
            ext_data_control_source_v1::Event::Cancelled => daemon.on_source_cancelled(source),
            _ => {}
        }
    }
}
