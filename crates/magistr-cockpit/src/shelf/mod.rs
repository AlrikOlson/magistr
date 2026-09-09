//! The deliverables shelf's effects: reading a golden
//! off disk into a [`ShelfStill`](crate::cockpit::ShelfStill), writing a
//! [`Shelf`](crate::cockpit::Shelf) as files — the export bundle, with
//! the chapter beside the receipts — and writing the
//! [`Chapter`](crate::cockpit::Chapter) as one page — the share route.
//!
//! Out of `cockpit` on purpose: the core is pure (`tests/purity.rs`),
//! so the shelf's *data* lives there and the file system is reached
//! here, by the entry point's job — the way the `golden` comparer
//! writes PNGs beside a pure `view`.

mod bundle;
mod load_still;
mod page;

pub use bundle::write_bundle;
pub use load_still::load_still;
pub use page::write_page;
