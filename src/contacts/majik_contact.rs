//! `MajikContact`: a cryptographic contact identity (port of `contacts/majik-contact.ts`).

use crate::errors::MajikContactError;
use crate::types::{
    MajikContactCard, MajikContactData, MajikContactMeta, MajikKeyAddress,
    MajikMessageIdentityJSON, SerializedMajikContact, X25519RawKey,
};
use crate::utils::{array_to_base64, base64_to_bytes, now_iso};

type Result<T> = std::result::Result<T, MajikContactError>;

/// Represents a cryptographic contact identity.
///
/// Stores the public identity material required to identify and communicate with a peer,
/// together with user-defined metadata and contact status, including post-quantum key material
/// (`ml_key`, `ml_dsa_public_key_base64`).
///
/// Identity fields are read-only (private with accessors); `meta` is public and mutable, and
/// the `update_*` / `block` family update `meta.updated_at` and return `&mut Self` for chaining.
///
/// ```
/// use majik_contact::{MajikContact, MajikContactMeta, X25519RawKey};
///
/// let mut contact = MajikContact::create(
///     "user-uuid",
///     X25519RawKey::new(vec![1u8; 32]),
///     "ml-kem-public-key",
///     "fingerprint",
///     Some(MajikContactMeta { label: Some("Alice".into()), ..Default::default() }),
///     None,
///     None,
/// )?;
/// contact.update_label("Alice (Lead Engineer)").block();
/// assert!(contact.is_blocked());
/// # Ok::<(), majik_contact::MajikContactError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MajikContact {
    id: String,
    public_key: X25519RawKey,
    fingerprint: String,
    ml_key: String,
    ed_public_key_base64: String,
    ml_dsa_public_key_base64: String,
    /// Mutable metadata attached to the contact (defaults applied at construction).
    pub meta: MajikContactMeta,
    majikah_registered: Option<bool>,
}

/// `{ ...target, ...patch }`: fields present in `patch` replace those in `target`.
fn merge_meta(target: &mut MajikContactMeta, patch: MajikContactMeta) {
    if patch.label.is_some() {
        target.label = patch.label;
    }
    if patch.notes.is_some() {
        target.notes = patch.notes;
    }
    if patch.blocked.is_some() {
        target.blocked = patch.blocked;
    }
    if patch.created_at.is_some() {
        target.created_at = patch.created_at;
    }
    if patch.updated_at.is_some() {
        target.updated_at = patch.updated_at;
    }
    target.extra.extend(patch.extra);
}

impl MajikContact {
    /// Creates a new contact from validated contact data. Metadata defaults are applied
    /// for omitted fields.
    ///
    /// # Errors
    /// [`MajikContactError`] when the contact ID, public key, ML key or fingerprint is empty.
    pub fn new(data: MajikContactData) -> Result<Self> {
        Self::assert_id(&data.id)?;
        Self::assert_public_key(&data.public_key)?;
        Self::assert_ml_key(&data.ml_key)?;
        Self::assert_fingerprint(&data.fingerprint)?;

        let now = now_iso();
        let mut meta = MajikContactMeta {
            label: Some(String::new()),
            notes: Some(String::new()),
            blocked: Some(false),
            created_at: Some(now.clone()),
            updated_at: Some(now),
            extra: Default::default(),
        };
        if let Some(patch) = data.meta {
            merge_meta(&mut meta, patch);
        }

        Ok(Self {
            id: data.id,
            public_key: data.public_key,
            fingerprint: data.fingerprint,
            ml_key: data.ml_key,
            ed_public_key_base64: data.ed_public_key_base64.unwrap_or_default(),
            ml_dsa_public_key_base64: data.ml_dsa_public_key_base64.unwrap_or_default(),
            meta,
            majikah_registered: data.majikah_registered,
        })
    }

    /// Preferred convenience factory when constructing a contact programmatically.
    pub fn create(
        id: impl Into<String>,
        public_key: X25519RawKey,
        ml_key: impl Into<String>,
        fingerprint: impl Into<String>,
        meta: Option<MajikContactMeta>,
        ed_public_key_base64: Option<String>,
        ml_dsa_public_key_base64: Option<String>,
    ) -> Result<Self> {
        Self::new(MajikContactData {
            id: id.into(),
            public_key,
            fingerprint: fingerprint.into(),
            ml_key: ml_key.into(),
            meta,
            majikah_registered: None,
            ed_public_key_base64,
            ml_dsa_public_key_base64,
        })
    }

    /* ---------------- Accessors (TS `readonly` fields) ---------------- */

    /// Application-level identifier (independent of the key material).
    pub fn id(&self) -> &str {
        &self.id
    }
    /// X25519 public key material.
    pub fn public_key(&self) -> &X25519RawKey {
        &self.public_key
    }
    /// Cryptographic identity fingerprint.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// Serialized ML-KEM public-key material.
    pub fn ml_key(&self) -> &str {
        &self.ml_key
    }
    /// Base64 Ed25519 public key (empty string when none was supplied).
    pub fn ed_public_key_base64(&self) -> &str {
        &self.ed_public_key_base64
    }
    /// Base64 ML-DSA public key (empty string when none was supplied).
    pub fn ml_dsa_public_key_base64(&self) -> &str {
        &self.ml_dsa_public_key_base64
    }

    /* ---------------- Validation ---------------- */

    fn assert_id(id: &str) -> Result<()> {
        if id.is_empty() {
            return Err(MajikContactError::new(
                "Contact ID must be a non-empty string",
            ));
        }
        Ok(())
    }

    fn assert_ml_key(key: &str) -> Result<()> {
        if key.is_empty() {
            return Err(MajikContactError::new("ML Key must be a non-empty string"));
        }
        Ok(())
    }

    // The TS check also accepted WebCrypto `CryptoKey` objects; in Rust the only
    // representation is raw bytes, so this is always valid (kept for structural parity).
    fn assert_public_key(_key: &X25519RawKey) -> Result<()> {
        Ok(())
    }

    fn assert_fingerprint(fingerprint: &str) -> Result<()> {
        if fingerprint.is_empty() {
            return Err(MajikContactError::new(
                "Fingerprint must be a non-empty string",
            ));
        }
        Ok(())
    }

    fn update_timestamp(&mut self) {
        self.meta.updated_at = Some(now_iso());
    }

    /* ---------------- Metadata ---------------- */

    /// Partially updates the metadata: only fields present in `updates` are replaced
    /// (including custom fields in `extra`). Always refreshes `updated_at`.
    pub fn update_meta(&mut self, updates: MajikContactMeta) -> &mut Self {
        merge_meta(&mut self.meta, updates);
        self.update_timestamp();
        self
    }

    /// Updates the contact's display label.
    pub fn update_label(&mut self, label: impl Into<String>) -> &mut Self {
        self.meta.label = Some(label.into());
        self.update_timestamp();
        self
    }

    /// Updates the contact's notes.
    pub fn update_notes(&mut self, notes: impl Into<String>) -> &mut Self {
        self.meta.notes = Some(notes.into());
        self.update_timestamp();
        self
    }

    /// Whether the contact is currently blocked.
    pub fn is_blocked(&self) -> bool {
        self.meta.blocked.unwrap_or(false)
    }

    /// Sets the blocked state.
    pub fn set_blocked(&mut self, blocked: bool) -> &mut Self {
        self.meta.blocked = Some(blocked);
        self.update_timestamp();
        self
    }

    /// Blocks the contact (idempotent: no timestamp change when already blocked).
    pub fn block(&mut self) -> &mut Self {
        if !self.is_blocked() {
            self.set_blocked(true);
        }
        self
    }

    /// Unblocks the contact (idempotent).
    pub fn unblock(&mut self) -> &mut Self {
        if self.is_blocked() {
            self.set_blocked(false);
        }
        self
    }

    /// Static form of [`is_blocked`](Self::is_blocked).
    pub fn is_contact_blocked(contact: &MajikContact) -> bool {
        contact.meta.blocked.unwrap_or(false)
    }

    /* ---------------- Majikah registration ---------------- */

    /// Whether a registration state has been explicitly recorded
    /// (distinguishes "unknown" from "known unregistered").
    pub fn is_majikah_identity_checked(&self) -> bool {
        self.majikah_registered.is_some()
    }

    /// Known registration state; an unchecked contact reports `false`.
    pub fn is_majikah_registered(&self) -> bool {
        self.majikah_registered.unwrap_or(false)
    }

    /// Sets the cached Majikah registration state.
    pub fn set_majikah_status(&mut self, status: bool) -> &mut Self {
        self.majikah_registered = Some(status);
        self
    }

    /* ---------------- Display / export ---------------- */

    /// Base64 address of the contact's public key.
    pub fn address(&self) -> MajikKeyAddress {
        array_to_base64(&self.public_key.raw)
    }

    /// The metadata label when non-empty, otherwise the address.
    pub fn display_name(&self) -> String {
        match self.meta.label.as_deref() {
            Some(label) if !label.is_empty() => label.to_owned(),
            _ => self.address(),
        }
    }

    /// Converts the contact into a shareable card (no notes/timestamps).
    pub fn to_contact_card(&self) -> MajikContactCard {
        MajikContactCard {
            id: self.id.clone(),
            label: self.meta.label.clone().unwrap_or_default(),
            public_key: self.address(),
            fingerprint: self.fingerprint.clone(),
            ml_key: self.ml_key.clone(),
            ed_public_key_base64: Some(self.ed_public_key_base64.clone()),
            ml_dsa_public_key_base64: Some(self.ml_dsa_public_key_base64.clone()),
        }
    }

    /// Serializes the contact into its persistent JSON-compatible form.
    pub fn to_json(&self) -> SerializedMajikContact {
        SerializedMajikContact {
            id: self.id.clone(),
            fingerprint: self.fingerprint.clone(),
            meta: Some(self.meta.clone()),
            public_key_base64: self.address(),
            majikah_registered: self.majikah_registered,
            ml_key: self.ml_key.clone(),
            ed_public_key_base64: Some(self.ed_public_key_base64.clone()),
            ml_dsa_public_key_base64: Some(self.ml_dsa_public_key_base64.clone()),
        }
    }

    /// Reconstructs a contact from its serialized representation.
    ///
    /// # Errors
    /// [`MajikContactError`] ("Failed to deserialize MajikContact") when the public key is not
    /// valid base64 or the data fails validation.
    pub fn from_json(serialized: SerializedMajikContact) -> Result<Self> {
        const FAIL: &str = "Failed to deserialize MajikContact";
        let raw = base64_to_bytes(&serialized.public_key_base64)
            .map_err(|e| MajikContactError::with_cause(FAIL, e))?;
        Self::new(MajikContactData {
            id: serialized.id,
            fingerprint: serialized.fingerprint,
            meta: serialized.meta,
            public_key: X25519RawKey { raw },
            majikah_registered: serialized.majikah_registered,
            ml_key: serialized.ml_key,
            ed_public_key_base64: serialized.ed_public_key_base64,
            ml_dsa_public_key_base64: serialized.ml_dsa_public_key_base64,
        })
        .map_err(|e| MajikContactError::with_cause(FAIL, e))
    }

    /// Creates a contact from a Majik Message-style identity payload.
    ///
    /// The identity `id` doubles as the fingerprint and the contact is marked as
    /// Majikah-registered; `restricted` maps to `blocked`.
    pub fn from_identity_json(identity: &MajikMessageIdentityJSON) -> Result<Self> {
        const FAIL: &str = "Failed to create MajikContact from MajikMessageIdentityJSON";
        let raw = base64_to_bytes(&identity.public_key)
            .map_err(|e| MajikContactError::with_cause(FAIL, e))?;
        Self::new(MajikContactData {
            id: identity.id.clone(),
            public_key: X25519RawKey { raw },
            fingerprint: identity.id.clone(),
            meta: Some(MajikContactMeta {
                label: Some(identity.label.clone()),
                created_at: Some(identity.timestamp.clone()),
                updated_at: Some(identity.timestamp.clone()),
                blocked: Some(identity.restricted),
                ..Default::default()
            }),
            majikah_registered: Some(true),
            ml_key: identity.ml_key.clone(),
            ed_public_key_base64: None,
            ml_dsa_public_key_base64: None,
        })
        .map_err(|e| MajikContactError::with_cause(FAIL, e))
    }
}
