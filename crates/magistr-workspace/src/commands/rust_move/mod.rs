//! Rust-aware `files.move` planning (retained specs/010). Rust imports name
//! modules, not files: the module path of a `.rs` file is derived from its
//! location by the file-module conventions and confirmed by the actual
//! `mod <name>;` declaration, then every contiguous `crate::`/`super::`/
//! `self::`/uniform path that resolves into the moved module is rewritten at
//! segment boundaries. Pure: it works over a listing and a read function.
mod declaration_change;
mod declaration_item;
mod declaration_kind;
mod declarations;
mod manifest;
mod module_location;
mod module_tree;
mod paths;
mod plan;
mod planner;
mod rewrite;
mod rewrite_context;
mod unrewritten;
mod write;

pub use declaration_change::RustDeclarationChange;
pub use declaration_kind::RustDeclarationKind;
pub use plan::RustMovePlan;
pub(crate) use planner::plan_rust_move;
pub use unrewritten::RustUnrewritten;
pub use write::RustMoveWrite;
