//! Selection events of the seat's data-control device.

use wayland_client::{Connection, Dispatch, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, EVT_DATA_OFFER_OPCODE, ExtDataControlDeviceV1},
    ext_data_control_offer_v1::ExtDataControlOfferV1,
};

use super::offer::OfferMimes;
use crate::{Daemon, ownership::SelectionKind};

impl Dispatch<ExtDataControlDeviceV1, ()> for Daemon {
    fn event(
        daemon: &mut Self,
        device: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::Selection { id } => {
                daemon.on_selection(SelectionKind::Clipboard, id);
            }
            ext_data_control_device_v1::Event::PrimarySelection { id } => {
                daemon.on_selection(SelectionKind::Primary, id);
            }
            ext_data_control_device_v1::Event::Finished => {
                device.destroy();
                daemon.on_device_finished();
            }
            _ => {}
        }
    }

    event_created_child!(Daemon, ExtDataControlDeviceV1, [
        EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, OfferMimes::default()),
    ]);
}
