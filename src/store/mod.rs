//! The clipboard history: captured entries, newest first.

pub mod entry;
pub mod history;

pub use entry::{Entry, EntryId, Payloads};
pub use history::History;
