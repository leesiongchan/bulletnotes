//! SilverBullet protocol; background scheduling and reconciliation are separate owners.
mod client;
mod protocol;
pub use client::SilverBulletClient;
pub use protocol::SecretToken;

pub mod probe;
