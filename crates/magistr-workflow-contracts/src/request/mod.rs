//! Canonical request records and typed mutation events, without host effects.

mod frame;
mod promotion;
mod records;
pub use promotion::RequestPromotion;

pub use frame::RequestFrame;
pub use records::{
    Enrichment, REQUESTS_SCHEMA_VERSION, Request, RequestKind, RequestStatus, Requests,
    merge_request_stores, request_wins,
};
