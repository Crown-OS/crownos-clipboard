//! The event loop's state: routes selection events through the ownership
//! state machine to capture, history and our own sources.

use std::{mem, sync::Arc};

use calloop::LoopHandle;
use wayland_client::{Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::ExtDataControlSourceV1,
};

use crate::{
    Config, Error, Result,
    capture::{Capture, Progress},
    ownership::{Ownership, Reaction, SelectionKind},
    store::{Entry, EntryId, History, Payloads},
    wayland::{Globals, offer::OfferMimes},
};

#[derive(Default)]
struct Slot {
    ownership: Ownership,
    capture: Option<Capture>,
    source: Option<ExtDataControlSourceV1>,
}

pub struct Daemon {
    config: Config,
    handle: LoopHandle<'static, Daemon>,
    queue: QueueHandle<Daemon>,
    manager: ExtDataControlManagerV1,
    device: ExtDataControlDeviceV1,
    history: History,
    clipboard: Slot,
    primary: Slot,
    changed: bool,
    finished: bool,
}

impl Daemon {
    pub fn new(
        config: Config,
        handle: LoopHandle<'static, Daemon>,
        queue: QueueHandle<Daemon>,
        globals: Globals,
    ) -> Self {
        let device = globals.manager.get_data_device(&globals.seat, &queue, ());
        Self {
            history: History::new(config.history_size),
            config,
            handle,
            queue,
            manager: globals.manager,
            device,
            clipboard: Slot::default(),
            primary: Slot::default(),
            changed: false,
            finished: false,
        }
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn loop_handle(&self) -> &LoopHandle<'static, Daemon> {
        &self.handle
    }

    /// Whether the history changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        mem::take(&mut self.changed)
    }

    /// Whether the seat's data device is gone for good.
    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn select(&mut self, id: EntryId) -> Result<()> {
        let entry = self.history.promote(id).ok_or(Error::NoSuchEntry(id))?;
        self.changed = true;
        self.claim(SelectionKind::Clipboard, entry);
        Ok(())
    }

    pub fn delete(&mut self, id: EntryId) -> Result<()> {
        self.history.remove(id).ok_or(Error::NoSuchEntry(id))?;
        self.clipboard.ownership.forget(|entry| entry.id == id);
        self.changed = true;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.clipboard.ownership.forget(|_| true);
        self.changed = true;
    }

    pub(crate) fn on_selection(
        &mut self,
        kind: SelectionKind,
        offer: Option<ExtDataControlOfferV1>,
    ) {
        if kind == SelectionKind::Primary && !self.config.persist_primary {
            offer.inspect(ExtDataControlOfferV1::destroy);
            return;
        }
        let reaction = self.slot(kind).ownership.on_selection(offer.is_some());
        match (reaction, offer) {
            (Reaction::Capture, Some(offer)) => self.begin_capture(kind, offer),
            (reaction, offer) => {
                offer.inspect(ExtDataControlOfferV1::destroy);
                if let Some(capture) = &mut self.slot(kind).capture {
                    capture.orphan();
                }
                if let Reaction::Persist(entry) = reaction {
                    self.claim(kind, entry);
                }
            }
        }
    }

    pub(crate) fn on_transfer_done(&mut self, kind: SelectionKind, outcome: Result<Vec<u8>>) {
        let (slot, handle) = self.slot_and_handle(kind);
        let Some(capture) = &mut slot.capture else {
            return;
        };
        let progress = capture.advance(outcome, kind, handle);
        self.settle(kind, progress);
    }

    pub(crate) fn on_source_cancelled(&mut self, source: &ExtDataControlSourceV1) {
        source.destroy();
        for slot in [&mut self.clipboard, &mut self.primary] {
            if slot.source.as_ref() == Some(source) {
                slot.source = None;
            }
        }
    }

    pub(crate) fn on_device_finished(&mut self) {
        log::error!("the compositor retired our data-control device");
        self.finished = true;
    }

    fn begin_capture(&mut self, kind: SelectionKind, offer: ExtDataControlOfferV1) {
        let mimes = offer
            .data::<OfferMimes>()
            .map(OfferMimes::take)
            .unwrap_or_default();
        let mut capture = Capture::new(offer, mimes, self.config.max_entry_bytes);
        let progress = capture.fetch_next(kind, &self.handle);
        self.slot(kind).capture = Some(capture);
        self.settle(kind, progress);
    }

    fn settle(&mut self, kind: SelectionKind, progress: Progress) {
        let Progress::Finished(payloads) = progress else {
            return;
        };
        self.slot(kind).capture = None;
        let entry = payloads.map(|payloads| self.keep(kind, payloads));
        if let Reaction::Persist(entry) = self.slot(kind).ownership.on_captured(entry) {
            self.claim(kind, entry);
        }
    }

    fn keep(&mut self, kind: SelectionKind, payloads: Payloads) -> Arc<Entry> {
        match kind {
            SelectionKind::Clipboard => {
                self.changed = true;
                self.history.record(payloads)
            }
            SelectionKind::Primary => self.history.detached(payloads),
        }
    }

    /// Offers `entry` from a fresh source: a source is never set twice.
    fn claim(&mut self, kind: SelectionKind, entry: Arc<Entry>) {
        let source = self
            .manager
            .create_data_source(&self.queue, Arc::clone(&entry));
        for (mime, _) in &entry.mimes {
            source.offer(mime.clone());
        }
        match kind {
            SelectionKind::Clipboard => self.device.set_selection(Some(&source)),
            SelectionKind::Primary => self.device.set_primary_selection(Some(&source)),
        }
        log::debug!("offering entry {} as the {kind:?} selection", entry.id);
        let slot = self.slot(kind);
        slot.ownership.on_claimed();
        if let Some(previous) = slot.source.replace(source) {
            previous.destroy();
        }
    }

    fn slot(&mut self, kind: SelectionKind) -> &mut Slot {
        self.slot_and_handle(kind).0
    }

    fn slot_and_handle(
        &mut self,
        kind: SelectionKind,
    ) -> (&mut Slot, &LoopHandle<'static, Daemon>) {
        let slot = match kind {
            SelectionKind::Clipboard => &mut self.clipboard,
            SelectionKind::Primary => &mut self.primary,
        };
        (slot, &self.handle)
    }
}
