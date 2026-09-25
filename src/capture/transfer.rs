//! One `receive` of one mime type: a pipe drained by the event loop, bounded
//! in size and time. Dropping a transfer abandons it.

use std::{fs::File, mem, os::fd::AsFd, time::Duration};

use calloop::{
    Interest, LoopHandle, Mode, PostAction, RegistrationToken,
    generic::Generic,
    timer::{TimeoutAction, Timer},
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::ExtDataControlOfferV1;

use crate::{
    Daemon, Error, Result,
    ownership::SelectionKind,
    util::pipe::{self, Flow},
};

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(1);

pub struct Transfer {
    handle: LoopHandle<'static, Daemon>,
    reader: RegistrationToken,
    deadline: RegistrationToken,
}

impl Transfer {
    pub fn start(
        offer: &ExtDataControlOfferV1,
        mime: &str,
        max_bytes: usize,
        kind: SelectionKind,
        handle: &LoopHandle<'static, Daemon>,
    ) -> Result<Self> {
        let (reader, writer) = std::io::pipe()?;
        offer.receive(mime.to_owned(), writer.as_fd());
        drop(writer);
        let reader = File::from(std::os::fd::OwnedFd::from(reader));
        pipe::set_nonblocking(&reader)?;

        let mut payload = Vec::new();
        let reader = handle
            .insert_source(
                Generic::new(reader, Interest::READ, Mode::Level),
                move |_, pipe, daemon| {
                    let outcome = match pipe::drain(&**pipe, &mut payload, max_bytes) {
                        Ok(Flow::WouldBlock) => return Ok(PostAction::Continue),
                        Ok(Flow::Done) => Ok(mem::take(&mut payload)),
                        Err(error) => Err(error),
                    };
                    daemon.on_transfer_done(kind, outcome);
                    Ok(PostAction::Remove)
                },
            )
            .map_err(|error| error.error)?;

        let deadline = handle.insert_source(
            Timer::from_duration(TRANSFER_TIMEOUT),
            move |_, _, daemon| {
                daemon.on_transfer_done(kind, Err(Error::TimedOut));
                TimeoutAction::Drop
            },
        );
        match deadline {
            Ok(deadline) => Ok(Self {
                handle: handle.clone(),
                reader,
                deadline,
            }),
            Err(error) => {
                handle.remove(reader);
                Err(error.error.into())
            }
        }
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        self.handle.remove(self.reader);
        self.handle.remove(self.deadline);
    }
}
