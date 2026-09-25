//! Eager capture of a foreign selection: every payload is read while its
//! source is still alive, one mime at a time.

pub mod harvest;
pub mod transfer;

use calloop::LoopHandle;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::ExtDataControlOfferV1;

use crate::{Daemon, Result, ownership::SelectionKind, store::Payloads};

pub use harvest::{Harvest, Verdict};
pub use transfer::Transfer;

pub enum Progress {
    Pending,
    /// `None` when the selection was secret, empty, or could not be read.
    Finished(Option<Payloads>),
}

/// Dropping a capture abandons its transfer and destroys its offer.
pub struct Capture {
    offer: Option<ExtDataControlOfferV1>,
    harvest: Harvest,
    transfer: Option<Transfer>,
}

impl Capture {
    pub fn new(offer: ExtDataControlOfferV1, mimes: Vec<String>, max_bytes: usize) -> Self {
        Self {
            offer: Some(offer),
            harvest: Harvest::new(mimes, max_bytes),
            transfer: None,
        }
    }

    /// Requests the next payload, or finishes when nothing is left to read.
    pub fn fetch_next(
        &mut self,
        kind: SelectionKind,
        handle: &LoopHandle<'static, Daemon>,
    ) -> Progress {
        let (Some(mime), Some(offer)) = (self.harvest.next_mime(), &self.offer) else {
            return Progress::Finished(self.harvest.take_payloads());
        };
        match Transfer::start(offer, mime, self.harvest.remaining_bytes(), kind, handle) {
            Ok(transfer) => {
                self.transfer = Some(transfer);
                Progress::Pending
            }
            Err(error) => {
                log::warn!("cannot read the {kind:?} selection as {mime}: {error}");
                Progress::Finished(None)
            }
        }
    }

    pub fn advance(
        &mut self,
        outcome: Result<Vec<u8>>,
        kind: SelectionKind,
        handle: &LoopHandle<'static, Daemon>,
    ) -> Progress {
        self.transfer = None;
        match outcome.map(|payload| self.harvest.accept(payload)) {
            Ok(Verdict::Continue) => self.fetch_next(kind, handle),
            Ok(Verdict::Secret) => {
                log::debug!("the {kind:?} selection is marked secret; not recording it");
                Progress::Finished(None)
            }
            Err(error) => {
                log::warn!("dropping the {kind:?} selection: {error}");
                Progress::Finished(None)
            }
        }
    }

    /// The owner is gone: finish the transfer in flight, request nothing more.
    pub fn orphan(&mut self) {
        if let Some(offer) = self.offer.take() {
            offer.destroy();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.orphan();
    }
}
