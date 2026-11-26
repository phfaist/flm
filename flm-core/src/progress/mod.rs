//! The progress sheet of a build: what is being done and how far it is, the
//! request to stop, and the log lines of the work.
//!
//! A host (the code that starts a build and shows its progress: a program, a
//! preview, a site generator) creates a sheet with [`Sheet::start`] and gets
//! two halves:
//!
//! - the [`Sheet`], the host's half, which reads and controls: it takes a
//!   [`Snapshot`] of the rows whenever the host chooses, and asks the build to
//!   stop ([`Sheet::cancel`]);
//! - the root row as a [`Task`], the work's half, whose [`Scope`] the host lends
//!   to the work, and which the host ends when the build is over.
//!
//! The work writes the sheet and never reads it. The one thing the work reads
//! is whether the build was asked to stop.
//!
//! # Rows
//!
//! The sheet is a tree of rows, one row for each piece of work a person may
//! wait for: a stage, a program run, a conversion, a network fetch. Copying
//! one of many small files is not a row, but an increment of a row's count.
//! Each row has an id ([`RowId`]), a parent (the row under which it was
//! declared, except for the root), a [`Kind`] with an optional title, an
//! optional label (the file, the citation key), a [`RowState`], an optional
//! [`Count`], and the times at which it started and ended when the host's
//! [`Observer`] gave them.
//!
//! Three rules make reading the sheet from time to time lose nothing: a row is
//! never removed; a state only moves forward; the number done only grows, and
//! the total never changes.
//!
//! # The work's handles
//!
//! Four types, so that each rule holds by the shape of the interface:
//!
//! - [`Scope`], lent to the code that works under a running row: it declares
//!   rows under that row, asks whether the build is cancelled, and writes log
//!   lines. It cannot start, count or end anything.
//! - [`Pending`], a declared row: [`start`](Pending::start) gives a `Task`,
//!   [`start_counted`](Pending::start_counted) a `Counted`. Dropped, it ends
//!   as [`RowState::NotRun`].
//! - [`Task`], a running row with nothing to count, and [`Counted`], a running
//!   row with a count, which [`advance`](Counted::advance)s. Each gives its
//!   scope and is ended by its owner with `finish`; dropped without it, by an
//!   early return or a panic, it ends as [`RowState::Failed`].
//!
//! A total is therefore given once, when the row starts, before any
//! increment, and only the owner of a row ends it:
//!
//! ```
//! use flm_core::progress::{Kind, Observer, RowState, Scope, Sheet, Unit};
//!
//! const BUILD: Kind = Kind::new("example.build");
//! const IMAGES: Kind = Kind::titled("example.images", "Converting images");
//!
//! fn convert(scope: &Scope, images: &[&str]) {
//!     let row = scope.child(&IMAGES, None).start_counted(Unit::Items, images.len() as u64);
//!     for _image in images {
//!         if scope.cancelled() {
//!             return; // `row` is dropped, and ends as failed
//!         }
//!         // ... convert the image ...
//!         row.advance(1);
//!     }
//!     row.finish();
//! }
//!
//! let (sheet, root) = Sheet::start(&BUILD, None, Observer::default(), None);
//! convert(&root.scope(), &["a.pdf", "b.pdf"]);
//! root.finish();
//!
//! let snapshot = sheet.snapshot();
//! assert_eq!(snapshot.rows[1].state, RowState::Finished);
//! assert_eq!(snapshot.rows[1].count.map(|count| count.done), Some(2));
//! ```
//!
//! **Children of a counted row.** A row declared under a row that has a count
//! stands for one of the counted units, shown in detail: under "3 of 12
//! trees", a row "`intro.flm`: 7 of 20 images". The owner of the counted row
//! advances it when that unit is done. A row declared under a row with no
//! count is one part of that row's work.
//!
//! **Possible work is declared in advance.** Where an owner knows that a piece
//! of work may be needed, it declares the row before it knows, and drops the
//! [`Pending`] when the work is not needed. A row may still be declared at any
//! time while its parent runs, for work that could not be foreseen.
//!
//! # Handles with no row
//!
//! A handle may have no row behind it; its row operations then do nothing, and
//! the work calls them all the same, with no test of its own:
//!
//! - [`Scope::none()`] is the scope of a build that reports nowhere.
//! - Past a depth limit ([`Pending::with_depth_limit`],
//!   [`Scope::with_depth_limit`]), a handle has no row, but still answers
//!   [`Scope::cancelled`] and still writes log lines, which are attributed to
//!   the nearest row that exists. A host chooses with the limit how much
//!   detail it wants.
//! - A row declared under a row that has ended has no row behind it.
//!
//! # The observer and the log sink
//!
//! The host gives two optional things when it starts a sheet, and nothing
//! else of the host's is ever called. The [`Observer`] holds three optional
//! functions: one called when a row starts, one when it ends (each returns
//! the time to store on the row, from the host's clock), and one before each
//! question about the cancellation; none is called when a count advances. The
//! [`LogSink`] receives the log lines that the work writes through its scopes
//! ([`Scope::log`]). The sheet reads no clock of its own and never computes
//! with the times.
//!
//! # Costs
//!
//! - [`Counted::advance`] is one atomic addition: no lock, no allocation, no
//!   clock, no call to the host.
//! - Any operation on a handle with no row is one test of that fact: no
//!   allocation, no formatting, no lock.
//! - Declaring a row costs one allocation for the row, one more for the copy
//!   of its label when it has one, and a short lock; starting and ending it
//!   each cost one test and, when the host set that function of the observer,
//!   one call of it, made outside any lock.
//! - [`Scope::cancelled`] costs one read of the flag, one test and, when the
//!   host set a checkpoint, one call of it, also past a depth limit.
//! - Passing a [`Kind`] copies no text; a label is copied only when a row is
//!   behind the handle.
//!
//! The module uses `std::sync` only and reads no clock, so it builds for
//! WebAssembly. Every handle is `Send` and `Sync`.
//!
//! # The estimate of the overall percentage
//!
//! The sheet holds facts only. The percentage of a build is an estimate, made
//! by the host with two functions that keep no memory and read no clock:
//! [`estimate`] weighs the rows of a snapshot by a table of expected durations
//! ([`Expected`]), which [`measure`] reads from the last snapshot of an
//! earlier build.

mod estimate;
mod handles;
mod kind;
mod log;
mod sheet;

pub use estimate::{estimate, measure, Expected, KindDurations};
pub use handles::{Counted, Pending, Scope, Task};
pub use kind::{Kind, SharedText};
pub use log::{LogLevel, LogRecord, LogSink};
pub use sheet::{
    CheckpointFn, Count, Observer, RowFn, RowId, RowSnapshot, RowState, Sheet, Snapshot, Unit,
};
