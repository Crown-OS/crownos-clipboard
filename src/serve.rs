//! Answers a paste from a cached payload without ever blocking the loop: what
//! does not fit in the pipe now is written as the reader drains it.

use std::{fs::File, os::fd::OwnedFd, sync::Arc};

use calloop::{Interest, LoopHandle, Mode, PostAction, generic::Generic};

use crate::{
    Daemon, Result,
    util::pipe::{self, Flow},
};

pub fn serve(handle: &LoopHandle<'static, Daemon>, payload: Arc<[u8]>, sink: OwnedFd) {
    if let Err(error) = start(handle, payload, sink) {
        log::debug!("a paste went unanswered: {error}");
    }
}

fn start(handle: &LoopHandle<'static, Daemon>, payload: Arc<[u8]>, sink: OwnedFd) -> Result<()> {
    let sink = File::from(sink);
    pipe::set_nonblocking(&sink)?;
    let mut written = 0;
    if pipe::pour(&sink, &payload, &mut written)? == Flow::Done {
        return Ok(());
    }
    handle
        .insert_source(
            Generic::new(sink, Interest::WRITE, Mode::Level),
            move |_, sink, _| {
                Ok(match pipe::pour(&**sink, &payload, &mut written) {
                    Ok(Flow::WouldBlock) => PostAction::Continue,
                    Ok(Flow::Done) => PostAction::Remove,
                    Err(error) => {
                        log::debug!("the paste reader went away: {error}");
                        PostAction::Remove
                    }
                })
            },
        )
        .map_err(|error| error.error)?;
    Ok(())
}
