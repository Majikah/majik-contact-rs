//! `MajikContactGroup`: a named collection of contact IDs
//! (port of `contacts/majik-contact-group.ts`).

use indexmap::IndexSet;

use crate::constants::SystemGroupId;
use crate::errors::MajikContactGroupError;
use crate::types::{
    MajikContactGroupData, MajikContactGroupMeta, MajikContactGroupSetOptions,
    PartialMajikContactGroupMeta, PhotoInput, SerializedMajikContactGroup,
};
use crate::utils::{js_len, js_trim, normalize_photo_to_base64, now_iso};

type Result<T> = std::result::Result<T, MajikContactGroupError>;

fn err(msg: impl Into<String>) -> MajikContactGroupError {
    MajikContactGroupError::new(msg)
}

/// Represents a collection of contact identities.
///
/// Groups are either **custom** (editable name/metadata) or **system** groups (Favorites,
/// Blocked) whose names are locked. Membership is an insertion-ordered set, so each contact ID
/// occurs at most once and [`list_member_ids`](Self::list_member_ids) is stable.
///
/// Mutators return `&mut Self` for chaining. Static [`merge`](Self::merge) /
/// [`intersect`](Self::intersect) create a **new** group; [`merge_with`](Self::merge_with) /
/// [`intersect_with`](Self::intersect_with) mutate the current one.
///
/// ```
/// use majik_contact::MajikContactGroup;
///
/// let mut team = MajikContactGroup::create("engineering", "Engineering", None, None)?;
/// team.update_description("Engineering contacts").add_member("alice")?.set_color(Some("#002968"));
/// assert_eq!(team.member_count(), 1);
/// # Ok::<(), majik_contact::MajikContactGroupError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MajikContactGroup {
    id: String,
    is_system: bool,
    /// Mutable group metadata (name, description, photo, timestamps, color).
    pub meta: MajikContactGroupMeta,
    member_ids: IndexSet<String>,
}

impl MajikContactGroup {
    /// Creates a group from its data structure. Defaults are applied for omitted fields and
    /// recognised system groups always receive their reserved name.
    ///
    /// # Errors
    /// [`MajikContactGroupError`] for an invalid ID, name (custom groups only) or member IDs.
    pub fn new(data: MajikContactGroupData) -> Result<Self> {
        Self::assert_id(&data.id)?;

        let is_system = data.is_system.unwrap_or(false);
        let m = data.meta.unwrap_or_default();
        let mut meta = MajikContactGroupMeta {
            name: m.name.unwrap_or_default(),
            description: m.description.unwrap_or_default(),
            photo_base64: m.photo_base64,
            created_at: m.created_at.unwrap_or_else(now_iso),
            updated_at: m.updated_at.unwrap_or_else(now_iso),
            color: m.color,
        };

        // System groups always get their locked names; user-supplied names are ignored.
        if is_system {
            if let Some(sys) = SystemGroupId::from_id(&data.id) {
                meta.name = sys.name().to_owned();
            }
        } else {
            Self::assert_name(&meta.name)?;
        }

        let raw_ids = data.member_ids.unwrap_or_default();
        Self::assert_member_ids(&raw_ids)?;

        Ok(Self {
            id: data.id,
            is_system,
            meta,
            member_ids: raw_ids.into_iter().collect(),
        })
    }

    /* ================================
     * Static Factories
     * ================================ */

    /// Creates a new custom (non-system) group. `meta.name`, if set, is overridden by `name`.
    pub fn create(
        id: impl Into<String>,
        name: impl Into<String>,
        meta: Option<PartialMajikContactGroupMeta>,
        member_ids: Option<Vec<String>>,
    ) -> Result<Self> {
        let mut meta = meta.unwrap_or_default();
        meta.name = Some(name.into());
        Self::new(MajikContactGroupData {
            id: id.into(),
            meta: Some(meta),
            member_ids,
            is_system: Some(false),
        })
    }

    /// Creates the system **Favorites** group.
    pub fn create_favorites() -> Self {
        Self::create_system(SystemGroupId::Favorites)
    }

    /// Creates the system **Blocked** group.
    pub fn create_blocked() -> Self {
        Self::create_system(SystemGroupId::Blocked)
    }

    fn create_system(id: SystemGroupId) -> Self {
        Self::new(MajikContactGroupData {
            id: id.as_str().to_owned(),
            is_system: Some(true),
            ..Default::default()
        })
        .expect("system group construction is infallible")
    }

    /* ---------------- Accessors ---------------- */

    /// Application-level group identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether this is a system-managed group.
    pub fn is_system(&self) -> bool {
        self.is_system
    }

    /* ================================
     * Metadata Mutation
     * ================================ */

    /// Renames the group (trimmed). System groups cannot be renamed.
    ///
    /// # Errors
    /// System group, empty name, name over 64 characters, or a reserved system-group name.
    pub fn update_name(&mut self, name: &str) -> Result<&mut Self> {
        if self.is_system {
            return Err(err(format!(
                "System group \"{}\" cannot be renamed",
                self.meta.name
            )));
        }
        Self::assert_name(name)?;
        self.meta.name = js_trim(name).to_owned();
        self.update_timestamp();
        Ok(self)
    }

    /// Updates the description (trimmed).
    pub fn update_description(&mut self, description: &str) -> &mut Self {
        self.meta.description = js_trim(description).to_owned();
        self.update_timestamp();
        self
    }

    /// Sets the color (trimmed); `None` clears it.
    pub fn set_color(&mut self, color: Option<&str>) -> &mut Self {
        self.meta.color = color.map(|c| js_trim(c).to_owned());
        self.update_timestamp();
        self
    }

    /// Clears the color.
    pub fn clear_color(&mut self) -> &mut Self {
        self.set_color(None)
    }

    /// Normalizes `input` into a base64 data URL and stores it as the group photo.
    ///
    /// Async because `http(s)` URLs are fetched (with the default `fetch` feature).
    pub async fn set_photo(&mut self, input: impl Into<PhotoInput>) -> Result<&mut Self> {
        self.meta.photo_base64 = Some(normalize_photo_to_base64(input).await?);
        self.update_timestamp();
        Ok(self)
    }

    /// Removes the group photo.
    pub fn clear_photo(&mut self) -> &mut Self {
        self.meta.photo_base64 = None;
        self.update_timestamp();
        self
    }

    /// Whether a non-empty photo is stored.
    pub fn has_photo(&self) -> bool {
        self.meta
            .photo_base64
            .as_deref()
            .is_some_and(|p| !p.is_empty())
    }

    /* ================================
     * Membership Management
     * ================================ */

    /// Adds a contact. Strict: errors when the contact is already a member.
    pub fn add_member(&mut self, contact_id: &str) -> Result<&mut Self> {
        Self::assert_contact_id(contact_id)?;
        if self.member_ids.contains(contact_id) {
            return Err(err(format!(
                "Contact \"{contact_id}\" is already a member of group \"{}\"",
                self.meta.name
            )));
        }
        self.member_ids.insert(contact_id.to_owned());
        self.update_timestamp();
        Ok(self)
    }

    /// Adds a contact only when absent (idempotent; timestamp unchanged when already present).
    pub fn add_member_if_absent(&mut self, contact_id: &str) -> Result<&mut Self> {
        Self::assert_contact_id(contact_id)?;
        if self.member_ids.insert(contact_id.to_owned()) {
            self.update_timestamp();
        }
        Ok(self)
    }

    /// Adds several contacts. Strict batch: errors, adding nothing, if any is already a member.
    pub fn add_members<S: AsRef<str>>(&mut self, contact_ids: &[S]) -> Result<&mut Self> {
        let ids: Vec<String> = contact_ids.iter().map(|s| s.as_ref().to_owned()).collect();
        Self::assert_member_ids(&ids)?;
        let duplicates: Vec<&str> = ids
            .iter()
            .filter(|id| self.member_ids.contains(*id))
            .map(String::as_str)
            .collect();
        if !duplicates.is_empty() {
            return Err(err(format!(
                "The following contacts are already members: {}",
                duplicates.join(", ")
            )));
        }
        self.member_ids.extend(ids);
        self.update_timestamp();
        Ok(self)
    }

    /// Removes a contact. Strict: errors when the contact is not a member.
    pub fn remove_member(&mut self, contact_id: &str) -> Result<&mut Self> {
        Self::assert_contact_id(contact_id)?;
        if !self.member_ids.shift_remove(contact_id) {
            return Err(err(format!(
                "Contact \"{contact_id}\" is not a member of group \"{}\"",
                self.meta.name
            )));
        }
        self.update_timestamp();
        Ok(self)
    }

    /// Removes a contact when present (idempotent).
    pub fn remove_member_if_present(&mut self, contact_id: &str) -> Result<&mut Self> {
        Self::assert_contact_id(contact_id)?;
        if self.member_ids.shift_remove(contact_id) {
            self.update_timestamp();
        }
        Ok(self)
    }

    /// Whether a contact belongs to the group.
    pub fn has_member(&self, contact_id: &str) -> Result<bool> {
        Self::assert_contact_id(contact_id)?;
        Ok(self.member_ids.contains(contact_id))
    }

    /// Member IDs in insertion order (a copy).
    pub fn list_member_ids(&self) -> Vec<String> {
        self.member_ids.iter().cloned().collect()
    }

    /// Number of members.
    pub fn member_count(&self) -> usize {
        self.member_ids.len()
    }

    /// Whether the group has no members.
    pub fn is_empty(&self) -> bool {
        self.member_ids.is_empty()
    }

    /// Removes all members.
    pub fn clear_members(&mut self) -> &mut Self {
        self.member_ids.clear();
        self.update_timestamp();
        self
    }

    /* ================================
     * System Group Helpers
     * ================================ */

    /// Whether this is the system Favorites group.
    pub fn is_favorites(&self) -> bool {
        self.id == SystemGroupId::Favorites.as_str()
    }

    /// Whether this is the system Blocked group.
    pub fn is_blocked(&self) -> bool {
        self.id == SystemGroupId::Blocked.as_str()
    }

    /* ================================
     * Set Operations: Static
     * ================================ */

    /// New non-system group holding the union of all groups' members.
    ///
    /// ID, name, description and photo come from the first group unless overridden through
    /// `options`.
    ///
    /// # Errors
    /// Fewer than two groups, an invalid override, or an invalid resulting group
    /// (e.g. the first group is a system group and no name override is given).
    pub fn merge(
        groups: &[&MajikContactGroup],
        options: Option<&MajikContactGroupSetOptions>,
    ) -> Result<Self> {
        Self::assert_group_slice(groups, "merge")?;
        let union: IndexSet<String> = groups
            .iter()
            .flat_map(|g| g.member_ids.iter().cloned())
            .collect();
        Self::build_set_result(groups[0], options, union)
    }

    /// New non-system group holding only members shared by every group.
    ///
    /// Same ID/name/description/photo rules as [`merge`](Self::merge).
    pub fn intersect(
        groups: &[&MajikContactGroup],
        options: Option<&MajikContactGroupSetOptions>,
    ) -> Result<Self> {
        Self::assert_group_slice(groups, "intersect")?;
        let mut shared = groups[0].member_ids.clone();
        for g in &groups[1..] {
            shared.retain(|id| g.member_ids.contains(id));
        }
        Self::build_set_result(groups[0], options, shared)
    }

    fn build_set_result(
        source: &MajikContactGroup,
        options: Option<&MajikContactGroupSetOptions>,
        member_ids: IndexSet<String>,
    ) -> Result<Self> {
        let opt_id = options.and_then(|o| o.id.as_deref());
        let opt_name = options.and_then(|o| o.name.as_deref());
        if let Some(id) = opt_id {
            Self::assert_static_id(id, "overrideId")?;
        }
        if let Some(name) = opt_name {
            Self::assert_static_name(name, "overrideName")?;
        }
        let id = opt_id.map_or_else(|| source.id.clone(), |i| js_trim(i).to_owned());
        let name = opt_name.map_or_else(|| source.meta.name.clone(), |n| js_trim(n).to_owned());

        Self::new(MajikContactGroupData {
            id,
            is_system: Some(false),
            meta: Some(PartialMajikContactGroupMeta {
                name: Some(name),
                description: Some(source.meta.description.clone()),
                photo_base64: source.meta.photo_base64.clone(),
                ..Default::default()
            }),
            member_ids: Some(member_ids.into_iter().collect()),
        })
    }

    /* ================================
     * Set Operations: Instance
     * ================================ */

    /// In-place union: adds every member of `others` to this group.
    pub fn merge_with(&mut self, others: &[&MajikContactGroup]) -> Result<&mut Self> {
        if others.is_empty() {
            return Err(err("mergeWith requires at least one other group"));
        }
        for other in others {
            self.member_ids.extend(other.member_ids.iter().cloned());
        }
        self.update_timestamp();
        Ok(self)
    }

    /// In-place intersection: keeps only members present in every group of `others`.
    pub fn intersect_with(&mut self, others: &[&MajikContactGroup]) -> Result<&mut Self> {
        if others.is_empty() {
            return Err(err("intersectWith requires at least one other group"));
        }
        for other in others {
            self.member_ids.retain(|id| other.member_ids.contains(id));
        }
        self.update_timestamp();
        Ok(self)
    }

    /* ================================
     * Serialization
     * ================================ */

    /// Serializes the group (membership as an ordered array).
    pub fn to_json(&self) -> SerializedMajikContactGroup {
        SerializedMajikContactGroup {
            id: self.id.clone(),
            meta: self.meta.clone(),
            member_ids: self.list_member_ids(),
            is_system: self.is_system,
        }
    }

    /// Reconstructs a group through the normal constructor, so deserialized groups obey the
    /// same rules as new ones.
    pub fn from_json(serialized: SerializedMajikContactGroup) -> Result<Self> {
        Self::new(MajikContactGroupData {
            id: serialized.id,
            meta: Some(PartialMajikContactGroupMeta {
                name: Some(serialized.meta.name),
                description: Some(serialized.meta.description),
                photo_base64: serialized.meta.photo_base64,
                created_at: Some(serialized.meta.created_at),
                updated_at: Some(serialized.meta.updated_at),
                color: serialized.meta.color,
            }),
            member_ids: Some(serialized.member_ids),
            is_system: Some(serialized.is_system),
        })
    }

    /// Parses a JSON string and reconstructs the group.
    pub fn from_json_str(json: &str) -> Result<Self> {
        let serialized: SerializedMajikContactGroup = serde_json::from_str(json)
            .map_err(|e| MajikContactGroupError::with_cause("Invalid serialized group data", e))?;
        Self::from_json(serialized)
    }

    /// Serializes the group to a JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(&self.to_json()).expect("group serialization is infallible")
    }

    /* ================================
     * Assertions / Validation
     * ================================ */

    fn update_timestamp(&mut self) {
        self.meta.updated_at = now_iso();
    }

    fn assert_id(id: &str) -> Result<()> {
        if js_trim(id).is_empty() {
            return Err(err("Group ID must be a non-empty string"));
        }
        Ok(())
    }

    fn is_reserved_name(name: &str) -> bool {
        let lowered = js_trim(name).to_lowercase();
        SystemGroupId::ALL
            .iter()
            .any(|g| g.name().to_lowercase() == lowered)
    }

    fn assert_name(name: &str) -> Result<()> {
        if js_trim(name).is_empty() {
            return Err(err("Group name must be a non-empty string"));
        }
        if js_len(js_trim(name)) > 64 {
            return Err(err("Group name must not exceed 64 characters"));
        }
        if Self::is_reserved_name(name) {
            return Err(err(format!(
                "\"{}\" is a reserved system group name and cannot be used",
                js_trim(name)
            )));
        }
        Ok(())
    }

    fn assert_contact_id(id: &str) -> Result<()> {
        if js_trim(id).is_empty() {
            return Err(err("Contact ID must be a non-empty string"));
        }
        Ok(())
    }

    fn assert_member_ids(ids: &[String]) -> Result<()> {
        for (index, id) in ids.iter().enumerate() {
            if js_trim(id).is_empty() {
                return Err(err(format!(
                    "Invalid member ID at index {index}: must be a non-empty string"
                )));
            }
        }
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        if unique.len() != ids.len() {
            return Err(err("Member IDs must not contain duplicates"));
        }
        Ok(())
    }

    fn assert_group_slice(groups: &[&MajikContactGroup], op: &str) -> Result<()> {
        if groups.len() < 2 {
            return Err(err(format!(
                "{op}: requires at least two groups, but received {}",
                groups.len()
            )));
        }
        Ok(())
    }

    fn assert_static_id(id: &str, field: &str) -> Result<()> {
        if js_trim(id).is_empty() {
            return Err(err(format!("{field} must be a non-empty string")));
        }
        Ok(())
    }

    fn assert_static_name(name: &str, field: &str) -> Result<()> {
        if js_trim(name).is_empty() {
            return Err(err(format!("{field} must be a non-empty string")));
        }
        if js_len(js_trim(name)) > 64 {
            return Err(err(format!("{field} must not exceed 64 characters")));
        }
        if Self::is_reserved_name(name) {
            return Err(err(format!(
                "{field}: \"{}\" is a reserved system group name and cannot be used",
                js_trim(name)
            )));
        }
        Ok(())
    }
}
