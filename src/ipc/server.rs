//! Serves the protocol from the daemon's own event loop, so a request reaches
//! the history without a lock or a channel hop.

use std::os::fd::OwnedFd;

use calloop::LoopHandle;
use crownos_ipc::{RemoteError, ServiceBuilder, adapter::calloop::CalloopService};

use crate::{Daemon, EntrySummary, Error, Result, protocol, util::memfd};

pub fn serve(handle: &LoopHandle<'static, Daemon>) -> Result<CalloopService> {
    let server = ServiceBuilder::new(protocol::SERVICE).build()?;
    Ok(CalloopService::insert(
        handle,
        server,
        |daemon, server, peer, message| {
            protocol::dispatch(&mut Requests(daemon), server, peer, message)
        },
    )?)
}

/// Tells subscribers about history changes. Runs between loop iterations, never
/// inside the service's own dispatch.
pub fn publish(service: &CalloopService, daemon: &mut Daemon) {
    if daemon.take_changed()
        && let Err(error) = service.emit(&protocol::Changed {})
    {
        log::warn!("broadcasting a history change failed: {error}");
    }
}

struct Requests<'a>(&'a mut Daemon);

impl Requests<'_> {
    fn payload(&self, id: u64, sink: OwnedFd) -> Result<String> {
        let entry = self.0.history().get(id).ok_or(Error::NoSuchEntry(id))?;
        let (mime, payload) = entry.preferred().ok_or(Error::EmptyEntry(id))?;
        memfd::fill(sink, payload)?;
        Ok(mime.to_owned())
    }
}

fn remote(error: Error) -> RemoteError {
    RemoteError::handler(error.to_string())
}

impl protocol::Handler for Requests<'_> {
    fn list(&mut self) -> Result<Vec<EntrySummary>, RemoteError> {
        Ok(self
            .0
            .history()
            .iter()
            .map(|entry| EntrySummary::from(&**entry))
            .collect())
    }

    fn get(&mut self, id: u64, sink: OwnedFd) -> Result<String, RemoteError> {
        self.payload(id, sink).map_err(remote)
    }

    fn select(&mut self, id: u64) -> Result<(), RemoteError> {
        self.0.select(id).map_err(remote)
    }

    fn delete(&mut self, id: u64) -> Result<(), RemoteError> {
        self.0.delete(id).map_err(remote)
    }

    fn clear(&mut self) -> Result<(), RemoteError> {
        self.0.clear();
        Ok(())
    }
}
