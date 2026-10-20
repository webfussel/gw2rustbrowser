//! Reading the Guild Wars 2 `Gw2.dat` archive.

pub mod archive;
pub mod error;
pub mod filetype;
pub mod formats;
pub mod fourcc;
pub mod index;
pub mod inflate;
pub mod names;

pub use archive::{Archive, EntryIds, FIRST_FILE_ENTRY, MftEntry};
pub use error::{Error, Result};
pub use filetype::{FileType, Kind};
pub use index::{Category, Index, IndexEntry, ScanProgress};
