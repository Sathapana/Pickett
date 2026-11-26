//! Readers and writers for the files used by Herbert M. Pickett's SPFIT/SPCAT
//! (CALPGM) programs.
//!
//! | File | Type | Written by |
//! |---|---|---|
//! | `.par`, `.var`, `.bak` | [`ParFile`] | you / SPFIT |
//! | `.lin` | [`LinFile`] | you / `calbak` |
//! | `.int` | [`IntFile`] | you |
//! | `.cat` | [`CatFile`] | SPCAT / `calmrg` |
//! | `.egy` | [`EgyFile`] | SPCAT |
//!
//! Every type has a `parse(&str)` constructor and a `to_text()` writer. The
//! parsers follow the C code (`getpar`, `getlin`, `pcard`, `getbcd`,
//! `readqn` …), so a file SPFIT/SPCAT accepts is read the same way here. The
//! writers use the same `printf` formats as the C programs, so a file written
//! by SPFIT/SPCAT and read back is written out again byte for byte.
//!
//! ```
//! use pickett_io::CatFile;
//!
//! let text = "   18397.7830  0.0000 -5.3539 3    0.0000  3  41505 202 1 0         0 0         \n";
//! let cat = CatFile::parse(text).unwrap();
//! let line = &cat.lines[0];
//! assert_eq!(line.freq, 18397.7830);
//! assert_eq!(line.upper, vec![1, 0]);
//! assert_eq!(line.lower, vec![0, 0]);
//! assert_eq!(cat.to_text(), text);
//! ```

pub mod cfmt;
pub mod pcard;
pub mod qn;

mod cat;
mod egy;
mod error;
mod int;
mod lin;
mod par;
mod param_id;

pub use cat::{CatFile, CatLine};
pub use egy::{EgyEntry, EgyFile, EgyLevel};
pub use error::ParseError;
pub use int::{Dipole, IntControl, IntFile};
pub use lin::{LinFile, LinLine};
pub use par::{Control, Flavor, ParFile, Parameter, SpinvOption};
pub use param_id::ParamId;
