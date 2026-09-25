//! Non-blocking reads and writes on transfer pipes.

use std::{
    io::{self, ErrorKind, Read, Write},
    os::fd::AsFd,
};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    WouldBlock,
    Done,
}

pub fn set_nonblocking(fd: impl AsFd) -> io::Result<()> {
    Ok(rustix::io::ioctl_fionbio(fd, true)?)
}

/// Appends everything readable to `buffer`; `Done` once the writer closed.
pub fn drain(reader: impl Read, buffer: &mut Vec<u8>, limit: usize) -> Result<Flow> {
    let allowance = limit.saturating_sub(buffer.len()) as u64 + 1;
    let outcome = reader.take(allowance).read_to_end(buffer);
    if buffer.len() > limit {
        return Err(Error::TooLarge(limit));
    }
    match outcome {
        Ok(_) => Ok(Flow::Done),
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(Flow::WouldBlock),
        Err(error) => Err(error.into()),
    }
}

/// Writes `payload[*written..]` until done or the pipe is full.
pub fn pour(mut writer: impl Write, payload: &[u8], written: &mut usize) -> io::Result<Flow> {
    while let Some(rest) = payload.get(*written..).filter(|rest| !rest.is_empty()) {
        match writer.write(rest) {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(count) => *written += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(Flow::WouldBlock),
            Err(error) => return Err(error),
        }
    }
    Ok(Flow::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_reads_to_end_within_the_limit() {
        let mut buffer = Vec::new();
        assert_eq!(drain(&b"hello"[..], &mut buffer, 5).unwrap(), Flow::Done);
        assert_eq!(buffer, b"hello");
    }

    #[test]
    fn drain_rejects_payloads_over_the_limit() {
        let mut buffer = Vec::new();
        assert!(matches!(
            drain(&b"hello"[..], &mut buffer, 4),
            Err(Error::TooLarge(4))
        ));
    }

    #[test]
    fn drain_and_pour_pause_on_an_empty_or_full_pipe() {
        let (reader, writer) = std::io::pipe().unwrap();
        set_nonblocking(&reader).unwrap();
        set_nonblocking(&writer).unwrap();
        let mut buffer = Vec::new();
        assert_eq!(drain(&reader, &mut buffer, 16).unwrap(), Flow::WouldBlock);

        let payload = vec![7; 1 << 20];
        let mut written = 0;
        assert_eq!(
            pour(&writer, &payload, &mut written).unwrap(),
            Flow::WouldBlock
        );
        assert!(written > 0 && written < payload.len());
    }

    #[test]
    fn pour_fails_once_the_reader_is_gone() {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let mut written = 0;
        let error = pour(&writer, b"x", &mut written).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::BrokenPipe);
    }
}
