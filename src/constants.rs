//! Constants (port of `constants.ts`).

/// ID of the reserved Favorites group.
pub const SYSTEM_GROUP_ID_FAVORITES: &str = "system::favorites";
/// ID of the reserved Blocked group.
pub const SYSTEM_GROUP_ID_BLOCKED: &str = "system::blocked";

/// System-reserved group IDs (`SYSTEM_GROUP_IDS` / `SystemGroupId` in TS).
///
/// These groups are always present and cannot be renamed or deleted. Membership within them
/// is still fully manageable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemGroupId {
    Favorites,
    Blocked,
}

impl SystemGroupId {
    /// Every system group, in declaration order.
    pub const ALL: [SystemGroupId; 2] = [SystemGroupId::Favorites, SystemGroupId::Blocked];

    /// The reserved string ID, e.g. `"system::favorites"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            SystemGroupId::Favorites => SYSTEM_GROUP_ID_FAVORITES,
            SystemGroupId::Blocked => SYSTEM_GROUP_ID_BLOCKED,
        }
    }

    /// Human-readable locked name (`SYSTEM_GROUP_NAMES[id]` in TS).
    pub const fn name(self) -> &'static str {
        match self {
            SystemGroupId::Favorites => "Favorites",
            SystemGroupId::Blocked => "Blocked",
        }
    }

    /// Parses a reserved ID string.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.as_str() == id)
    }
}
