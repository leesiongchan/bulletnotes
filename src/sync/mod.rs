//! SilverBullet protocol; background scheduling and reconciliation are separate owners.
mod client;
mod protocol;
pub use client::SilverBulletClient;
pub use protocol::SecretToken;

pub mod probe;

pub mod reconcile;

pub mod text_merge;

pub mod merge_executor;

pub mod worker;

mod schedule;

mod catalogue;

pub(crate) use schedule::Retry;
