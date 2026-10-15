//! Shared, contact, and group types (port of `types.ts`).
//!
//! TypeScript `interface`/`type` shapes become plain structs. JSON field names are kept
//! byte-for-byte identical to the TS package via serde renames, so data persisted by the
//! TypeScript implementation round-trips unchanged.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::utils::now_iso;

/* -------------------------------
 * Shared / Cryptographic Types
 * ------------------------------- */

/// ISO 8601 date-time string (e.g. `2026-01-01T00:00:00.000Z`).
pub type ISODateString = String;
/// Base64-encoded public-key material (public information only).
pub type MajikKeyAddress = String;
/// Base64-encoded SHA-256 fingerprint of a MajikKey's X25519 public key (also the account `id`).
pub type MajikKeyFingerprint = String;
/// Base64-encoded Ed25519 public key.
pub type ED25519PublicKey = String;
/// Base64-encoded ML-KEM-768 public key.
pub type MLKEM768PublicKey = String;
/// Base64-encoded ML-DSA-87 public key.
pub type MLDSA87PublicKey = String;
/// Base64-encoded Bitcoin public key.
pub type BitcoinPublicKey = String;
/// Raw Ed25519 public-key bytes.
pub type ED25519RawPublicKey = Vec<u8>;
/// Raw ML-KEM-768 public-key bytes.
pub type MLKEM768RawPublicKey = Vec<u8>;
/// Raw ML-DSA-87 public-key bytes.
pub type MLDSA87RawPublicKey = Vec<u8>;
/// Raw Bitcoin public-key bytes.
pub type BitcoinRawPublicKey = Vec<u8>;

/// Wrapper for raw X25519 public-key bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct X25519RawKey {
    /// Raw X25519 public-key bytes.
    pub raw: Vec<u8>,
}

impl X25519RawKey {
    pub fn new(raw: impl Into<Vec<u8>>) -> Self {
        Self { raw: raw.into() }
    }
}

/* -------------------------------
 * API Types
 * ------------------------------- */

/// Common response envelope returned by Majikah API operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajikApiResponse {
    /// Whether the API operation completed successfully.
    pub success: bool,
    /// Human-readable response or status message.
    pub message: String,
    /// Optional machine-readable response code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Serialized identity payload used by Majik Message-compatible identity data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MajikMessageIdentityJSON {
    /// Account or identity identifier.
    pub id: String,
    /// User identifier associated with the identity.
    pub user_id: String,
    /// Base64-encoded X25519 public key.
    pub public_key: String,
    /// Serialized ML-KEM public key.
    pub ml_key: String,
    /// Public identity hash associated with the message identity.
    pub phash: String,
    /// Human-readable identity label.
    pub label: String,
    /// ISO-formatted identity timestamp.
    pub timestamp: String,
    /// Whether the identity is restricted.
    pub restricted: bool,
}

/* -------------------------------
 * Majik Contact Types
 * ------------------------------- */

/// Metadata attached to a [`MajikContact`](crate::MajikContact).
///
/// Every field is optional, so this struct doubles as `Partial<MajikContactMeta>` (used for
/// construction defaults and [`MajikContact::update_meta`](crate::MajikContact::update_meta)).
///
/// The TS `TMeta extends MajikContactMeta` generic is replaced by [`extra`](Self::extra):
/// application-specific fields are kept as JSON and survive serialization round-trips.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MajikContactMeta {
    /// Human-readable label used when displaying the contact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Free-form notes associated with the contact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Whether the contact is currently blocked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
    /// Contact creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<ISODateString>,
    /// Timestamp of the most recent metadata update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<ISODateString>,
    /// Application-specific metadata fields (the `TMeta` extension point).
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// JSON-serializable representation of a [`MajikContact`](crate::MajikContact).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedMajikContact {
    /// Application-level contact identifier.
    pub id: String,
    /// Cryptographic identity fingerprint.
    pub fingerprint: String,
    /// Persisted contact metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<MajikContactMeta>,
    /// Base64-encoded X25519 public-key material.
    pub public_key_base64: MajikKeyAddress,
    /// Serialized post-quantum ML-KEM public-key material.
    pub ml_key: String,
    /// Cached Majikah registration state (`None` = never recorded).
    #[serde(
        default,
        rename = "majikah_registered",
        skip_serializing_if = "Option::is_none"
    )]
    pub majikah_registered: Option<bool>,
    /// Base64-encoded Ed25519 public key.
    #[serde(
        default,
        rename = "edPublicKeyBase64",
        skip_serializing_if = "Option::is_none"
    )]
    pub ed_public_key_base64: Option<String>,
    /// Base64-encoded ML-DSA-87 public key.
    #[serde(
        default,
        rename = "mlDsaPublicKeyBase64",
        skip_serializing_if = "Option::is_none"
    )]
    pub ml_dsa_public_key_base64: Option<String>,
}

/// Initialization data used to construct a [`MajikContact`](crate::MajikContact).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MajikContactData {
    /// Application-level contact identifier.
    pub id: String,
    /// X25519 public-key material used by the contact.
    pub public_key: X25519RawKey,
    /// Cryptographic identity fingerprint.
    pub fingerprint: String,
    /// Serialized post-quantum ML-KEM public-key material.
    pub ml_key: String,
    /// Partial metadata; omitted fields get defaults.
    pub meta: Option<MajikContactMeta>,
    /// Cached Majikah registration state (`None` = unchecked).
    pub majikah_registered: Option<bool>,
    /// Base64-encoded Ed25519 public key.
    pub ed_public_key_base64: Option<String>,
    /// Base64-encoded ML-DSA-87 public key.
    pub ml_dsa_public_key_base64: Option<String>,
}

/// Compact public contact representation intended for sharing or exchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MajikContactCard {
    /// Application-level contact identifier.
    pub id: String,
    /// Base64-encoded X25519 public-key material.
    pub public_key: String,
    /// Cryptographic identity fingerprint.
    pub fingerprint: String,
    /// Human-readable contact label.
    pub label: String,
    /// Serialized post-quantum ML-KEM public-key material.
    pub ml_key: String,
    /// Base64-encoded Ed25519 public key (32 bytes before encoding).
    #[serde(
        default,
        rename = "edPublicKeyBase64",
        skip_serializing_if = "Option::is_none"
    )]
    pub ed_public_key_base64: Option<String>,
    /// Base64-encoded ML-DSA-87 public key (2592 bytes before encoding).
    #[serde(
        default,
        rename = "mlDsaPublicKeyBase64",
        skip_serializing_if = "Option::is_none"
    )]
    pub ml_dsa_public_key_base64: Option<String>,
}

/* -------------------------------
 * Contact Group Types
 * ------------------------------- */

fn default_now() -> String {
    now_iso()
}

/// Mutable metadata associated with a [`MajikContactGroup`](crate::MajikContactGroup).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MajikContactGroupMeta {
    /// Human-readable group name.
    pub name: String,
    /// Human-readable description of the group.
    #[serde(default)]
    pub description: String,
    /// Group photo normalized to a Base64 data URL; `None` serializes as `null`.
    #[serde(default)]
    pub photo_base64: Option<String>,
    /// Timestamp at which the group was created.
    #[serde(default = "default_now")]
    pub created_at: String,
    /// Timestamp of the most recent metadata or membership update.
    #[serde(default = "default_now")]
    pub updated_at: String,
    /// Optional UI/application color associated with the group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// `Partial<MajikContactGroupMeta>`: every field optional.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialMajikContactGroupMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub photo_base64: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub color: Option<String>,
}

/// Initialization data used to construct a [`MajikContactGroup`](crate::MajikContactGroup).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MajikContactGroupData {
    /// Application-level group identifier.
    pub id: String,
    /// Optional initial group metadata; missing fields are defaulted.
    pub meta: Option<PartialMajikContactGroupMeta>,
    /// Optional initial membership (non-empty IDs, no duplicates).
    pub member_ids: Option<Vec<String>>,
    /// Marks the group as system-managed.
    pub is_system: Option<bool>,
}

/// JSON-serializable representation of a [`MajikContactGroup`](crate::MajikContactGroup).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedMajikContactGroup {
    /// Application-level group identifier.
    pub id: String,
    /// Complete normalized group metadata.
    pub meta: MajikContactGroupMeta,
    /// Contact IDs belonging to the group.
    #[serde(default)]
    pub member_ids: Vec<String>,
    /// Whether this is a system-managed group.
    #[serde(default)]
    pub is_system: bool,
}

/// Options for static group set operations (`merge` / `intersect`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MajikContactGroupSetOptions {
    /// Override the ID of the resulting group (default: first source group's ID).
    pub id: Option<String>,
    /// Override the name of the resulting group (default: first source group's name).
    pub name: Option<String>,
}

/* -------------------------------
 * Photo input
 * ------------------------------- */

/// Photo sources accepted by [`normalize_photo_to_base64`](crate::normalize_photo_to_base64).
///
/// Replaces the TS `unknown` parameter: `Text` covers data-URLs, http(s) URLs and raw base64;
/// `Blob` covers `Blob`/`File`; `Bytes` covers `ArrayBuffer`/`Uint8Array`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoInput {
    /// A data-URL, an `http(s)://` URL, or a raw base64 string.
    Text(String),
    /// Raw image bytes, assumed to be JPEG.
    Bytes(Vec<u8>),
    /// A blob with an optional MIME type (`application/octet-stream` when absent).
    Blob {
        data: Vec<u8>,
        mime_type: Option<String>,
    },
}

impl From<&str> for PhotoInput {
    fn from(s: &str) -> Self {
        PhotoInput::Text(s.to_owned())
    }
}
impl From<String> for PhotoInput {
    fn from(s: String) -> Self {
        PhotoInput::Text(s)
    }
}
impl From<Vec<u8>> for PhotoInput {
    fn from(b: Vec<u8>) -> Self {
        PhotoInput::Bytes(b)
    }
}
impl From<&[u8]> for PhotoInput {
    fn from(b: &[u8]) -> Self {
        PhotoInput::Bytes(b.to_vec())
    }
}
