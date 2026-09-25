//! Filling a caller-supplied memfd with a payload.

use std::{fs::File, os::fd::OwnedFd, os::unix::fs::FileExt};

use crate::{Error, Result};

/// Replaces the contents of `sink` with `payload`. Only regular files are
/// accepted, so a peer cannot hand over a pipe and stall the event loop.
pub fn fill(sink: OwnedFd, payload: &[u8]) -> Result<()> {
    let sink = File::from(sink);
    if !sink.metadata()?.is_file() {
        return Err(Error::SinkNotAFile);
    }
    sink.set_len(payload.len() as u64)?;
    sink.write_all_at(payload, 0)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use rustix::fs::{MemfdFlags, memfd_create};

    use super::*;

    #[test]
    fn memfd_receives_the_payload() {
        let memfd = memfd_create("payload", MemfdFlags::CLOEXEC).unwrap();
        let reader = memfd.try_clone().unwrap();
        fill(memfd, b"hello").unwrap();
        let mut contents = String::new();
        File::from(reader).read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "hello");
    }

    #[test]
    fn pipes_are_refused() {
        let (_reader, writer) = std::io::pipe().unwrap();
        assert!(matches!(
            fill(writer.into(), b"x"),
            Err(Error::SinkNotAFile)
        ));
    }
}
