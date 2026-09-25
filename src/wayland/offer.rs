//! Offers announced by the compositor, and the mime types each advertises.

use std::{
    mem,
    sync::{Mutex, PoisonError},
};

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::{
    self, ExtDataControlOfferV1,
};

use crate::Daemon;

#[derive(Debug, Default)]
pub struct OfferMimes(Mutex<Vec<String>>);

impl OfferMimes {
    pub fn take(&self) -> Vec<String> {
        mem::take(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn push(&self, mime: String) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(mime);
    }
}

impl Dispatch<ExtDataControlOfferV1, OfferMimes> for Daemon {
    fn event(
        _: &mut Self,
        _: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        mimes: &OfferMimes,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            mimes.push(mime_type);
        }
    }
}
