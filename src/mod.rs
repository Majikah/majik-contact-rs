//! `majik-contact`: a cryptographically secure contact and identity model for Majikah.
//!
//! Crate root, the Rust counterpart of `index.ts` (wired up via `[lib] path` in `Cargo.toml`).

pub mod constants;
pub mod contacts;
pub mod errors;
pub mod types;
mod utils;

pub use constants::*;
pub use contacts::majik_contact::MajikContact;
pub use contacts::majik_contact_group::MajikContactGroup;
pub use errors::*;
pub use types::*;
pub use utils::{is_system_group_id, normalize_photo_to_base64};
