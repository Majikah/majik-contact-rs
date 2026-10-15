//! Port of `test/majik-contact.test.ts` (behavioural parity with the TypeScript suite).
//!
//! JS-only cases (runtime `typeof` checks, `Object.freeze`, `null`/`undefined` inputs, API
//! surface reflection) are not representable in Rust and are covered by the type system.

mod helpers;

use helpers::{b64, block_on, bytes, test_contact};
use majik_contact::*;
use std::{thread::sleep, time::Duration};

fn tick() {
    sleep(Duration::from_millis(5));
}

fn contact_err<T: std::fmt::Debug>(r: Result<T, MajikContactError>, msg: &str) {
    let e = r.expect_err("expected MajikContactError");
    assert!(e.message().contains(msg), "got: {e}");
}

fn group_err<T: std::fmt::Debug>(r: Result<T, MajikContactGroupError>, msg: &str) {
    let e = r.expect_err("expected MajikContactGroupError");
    assert!(e.message().contains(msg), "got: {e}");
}

fn data(seed: u64) -> MajikContactData {
    let c = test_contact(seed);
    MajikContactData {
        id: c.id().into(),
        public_key: c.public_key().clone(),
        fingerprint: c.fingerprint().into(),
        ml_key: c.ml_key().into(),
        ed_public_key_base64: Some(c.ed_public_key_base64().into()),
        ml_dsa_public_key_base64: Some(c.ml_dsa_public_key_base64().into()),
        ..Default::default()
    }
}

fn group(id: &str, name: &str, members: &[&str]) -> MajikContactGroup {
    let ids = (!members.is_empty()).then(|| members.iter().map(|s| s.to_string()).collect());
    MajikContactGroup::create(id, name, None, ids).unwrap()
}

/* ============================== MajikContact ============================== */

mod contact {
    use super::*;

    #[test]
    fn construction_applies_defaults() {
        let c = MajikContact::new(data(1)).unwrap();
        assert_eq!(c.meta.label.as_deref(), Some(""));
        assert_eq!(c.meta.notes.as_deref(), Some(""));
        assert_eq!(c.meta.blocked, Some(false));
        assert_eq!(c.meta.created_at, c.meta.updated_at);
        let ts = c.meta.created_at.as_deref().unwrap();
        assert_eq!(ts.len(), 24); // 2026-01-01T00:00:00.000Z
        assert!(ts.ends_with('Z') && ts.as_bytes()[10] == b'T');
        assert!(!c.is_majikah_identity_checked());
        assert!(!c.is_majikah_registered());
    }

    #[test]
    fn construction_merges_partial_meta() {
        let mut d = data(1);
        d.meta = Some(MajikContactMeta {
            label: Some("Alice".into()),
            created_at: Some("2025-01-01T00:00:00.000Z".into()),
            ..Default::default()
        });
        let c = MajikContact::new(d).unwrap();
        assert_eq!(c.meta.label.as_deref(), Some("Alice"));
        assert_eq!(c.meta.notes.as_deref(), Some(""));
        assert_eq!(
            c.meta.created_at.as_deref(),
            Some("2025-01-01T00:00:00.000Z")
        );
        assert_eq!(c.meta.blocked, Some(false));
    }

    #[test]
    fn missing_optional_keys_become_empty_strings() {
        let mut d = data(1);
        d.ed_public_key_base64 = None;
        d.ml_dsa_public_key_base64 = None;
        let c = MajikContact::new(d).unwrap();
        assert_eq!(c.ed_public_key_base64(), "");
        assert_eq!(c.ml_dsa_public_key_base64(), "");
    }

    #[test]
    fn create_factory_matches_constructor() {
        let src = test_contact(2);
        let c = MajikContact::create(
            src.id(),
            src.public_key().clone(),
            src.ml_key(),
            src.fingerprint(),
            Some(MajikContactMeta {
                label: Some("Bob".into()),
                ..Default::default()
            }),
            Some("ed".into()),
            Some("mldsa".into()),
        )
        .unwrap();
        assert_eq!(c.id(), src.id());
        assert_eq!(c.meta.label.as_deref(), Some("Bob"));
        assert_eq!(c.ed_public_key_base64(), "ed");
        assert_eq!(c.ml_dsa_public_key_base64(), "mldsa");
    }

    #[test]
    fn validation_rejects_empty_fields() {
        let mut d = data(1);
        d.id = String::new();
        contact_err(
            MajikContact::new(d),
            "Contact ID must be a non-empty string",
        );

        let mut d = data(1);
        d.ml_key = String::new();
        contact_err(MajikContact::new(d), "ML Key must be a non-empty string");

        let mut d = data(1);
        d.fingerprint = String::new();
        contact_err(
            MajikContact::new(d),
            "Fingerprint must be a non-empty string",
        );

        contact_err(MajikContact::new(MajikContactData::default()), "Contact ID");
    }

    #[test]
    fn whitespace_only_id_is_accepted_like_the_ts_version() {
        let mut d = data(1);
        d.id = "   ".into();
        assert_eq!(MajikContact::new(d).unwrap().id(), "   ");
    }

    #[test]
    fn metadata_mutation_updates_timestamp_and_chains() {
        let mut c = test_contact(1);
        let before = c.meta.updated_at.clone().unwrap();
        tick();
        c.update_label("Alice").update_notes("Verified");
        assert_eq!(c.meta.label.as_deref(), Some("Alice"));
        assert_eq!(c.meta.notes.as_deref(), Some("Verified"));
        assert!(c.meta.updated_at.as_deref().unwrap() > before.as_str());
    }

    #[test]
    fn update_meta_merges_and_preserves_untouched_fields() {
        let mut c = test_contact(1);
        c.update_label("Alice");
        let mut patch = MajikContactMeta {
            notes: Some("n".into()),
            ..Default::default()
        };
        patch.extra.insert("tier".into(), serde_json::json!("gold"));
        c.update_meta(patch);
        assert_eq!(c.meta.label.as_deref(), Some("Alice"));
        assert_eq!(c.meta.notes.as_deref(), Some("n"));
        assert_eq!(c.meta.extra["tier"], "gold");
    }

    #[test]
    fn blocking_is_idempotent() {
        let mut c = test_contact(1);
        assert!(!c.is_blocked());
        c.block();
        assert!(c.is_blocked() && MajikContact::is_contact_blocked(&c));
        let ts = c.meta.updated_at.clone();
        tick();
        c.block(); // no-op: timestamp untouched
        assert_eq!(c.meta.updated_at, ts);
        c.unblock();
        assert!(!c.is_blocked());
        let ts = c.meta.updated_at.clone();
        tick();
        c.unblock();
        assert_eq!(c.meta.updated_at, ts);
        c.set_blocked(true);
        assert!(c.is_blocked());
    }

    #[test]
    fn majikah_status_distinguishes_unknown_from_unregistered() {
        let mut c = test_contact(1);
        assert!(!c.is_majikah_identity_checked());
        c.set_majikah_status(false);
        assert!(c.is_majikah_identity_checked());
        assert!(!c.is_majikah_registered());
        c.set_majikah_status(true);
        assert!(c.is_majikah_registered());
    }

    #[test]
    fn address_is_base64_of_public_key() {
        let c = test_contact(1);
        assert_eq!(c.address(), b64(&bytes(1, 32)));
    }

    #[test]
    fn display_name_prefers_label_then_address() {
        let mut c = test_contact(1);
        assert_eq!(c.display_name(), c.address());
        c.update_label("Alice");
        assert_eq!(c.display_name(), "Alice");
        c.update_label("");
        assert_eq!(c.display_name(), c.address());
    }

    #[test]
    fn contact_card_contains_public_identity_only() {
        let mut c = test_contact(1);
        c.update_label("Alice").update_notes("secret note");
        let card = c.to_contact_card();
        assert_eq!(card.id, c.id());
        assert_eq!(card.label, "Alice");
        assert_eq!(card.public_key, c.address());
        assert_eq!(card.fingerprint, c.fingerprint());
        assert_eq!(card.ml_key, c.ml_key());
        let json = serde_json::to_value(&card).unwrap();
        assert_eq!(json["publicKey"], c.address());
        assert!(json.get("notes").is_none());
        assert!(json.get("meta").is_none());
    }

    #[test]
    fn json_shape_matches_the_typescript_wire_format() {
        let mut d = data(1);
        d.majikah_registered = Some(true);
        d.meta = Some(MajikContactMeta {
            label: Some("Alice".into()),
            notes: Some("Test".into()),
            blocked: Some(true),
            created_at: Some("2025-01-01T00:00:00.000Z".into()),
            updated_at: Some("2025-01-02T00:00:00.000Z".into()),
            ..Default::default()
        });
        let c = MajikContact::new(d).unwrap();
        let v = serde_json::to_value(c.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "id": c.id(),
                "fingerprint": c.fingerprint(),
                "meta": {
                    "label": "Alice", "notes": "Test", "blocked": true,
                    "createdAt": "2025-01-01T00:00:00.000Z",
                    "updatedAt": "2025-01-02T00:00:00.000Z"
                },
                "publicKeyBase64": c.address(),
                "majikah_registered": true,
                "mlKey": c.ml_key(),
                "edPublicKeyBase64": c.ed_public_key_base64(),
                "mlDsaPublicKeyBase64": c.ml_dsa_public_key_base64(),
            })
        );
    }

    #[test]
    fn unchecked_status_is_omitted_and_false_is_kept() {
        let v = serde_json::to_value(test_contact(1).to_json()).unwrap();
        assert!(v.get("majikah_registered").is_none());
        let mut c = test_contact(1);
        c.set_majikah_status(false);
        assert_eq!(
            serde_json::to_value(c.to_json()).unwrap()["majikah_registered"],
            false
        );
    }

    #[test]
    fn json_round_trip_preserves_everything_including_custom_meta() {
        let mut c = test_contact(3);
        let mut patch = MajikContactMeta {
            label: Some("Carol".into()),
            ..Default::default()
        };
        patch
            .extra
            .insert("tier".into(), serde_json::json!({"level": 2}));
        c.update_meta(patch).block();
        c.set_majikah_status(true);

        let text = serde_json::to_string(&c.to_json()).unwrap();
        let back = MajikContact::from_json(serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.meta.extra["tier"]["level"], 2);
    }

    #[test]
    fn from_json_accepts_ts_persisted_records_without_meta() {
        let raw = serde_json::json!({
            "id": "x", "fingerprint": "f", "publicKeyBase64": b64(&[1, 2, 3]), "mlKey": "ml"
        });
        let c = MajikContact::from_json(serde_json::from_value(raw).unwrap()).unwrap();
        assert_eq!(c.public_key().raw, vec![1, 2, 3]);
        assert_eq!(c.meta.blocked, Some(false));
        assert!(!c.is_majikah_identity_checked());
    }

    #[test]
    fn from_json_rejects_invalid_base64_and_wraps_errors() {
        let mut s = test_contact(1).to_json();
        s.public_key_base64 = "!!!not base64!!!".into();
        let e = MajikContact::from_json(s).unwrap_err();
        assert_eq!(e.message(), "Failed to deserialize MajikContact");
        assert!(std::error::Error::source(&e).is_some());

        let mut s = test_contact(1).to_json();
        s.id = String::new();
        let e = MajikContact::from_json(s).unwrap_err();
        assert_eq!(e.message(), "Failed to deserialize MajikContact");
    }

    #[test]
    fn from_json_decodes_like_atob() {
        let mut s = test_contact(1).to_json();
        s.public_key_base64 = "AQID".into();
        assert_eq!(
            MajikContact::from_json(s.clone()).unwrap().public_key().raw,
            [1, 2, 3]
        );
        s.public_key_base64 = "AQI".into(); // missing padding accepted
        assert_eq!(
            MajikContact::from_json(s.clone()).unwrap().public_key().raw,
            [1, 2]
        );
        s.public_key_base64 = "AQ ID\n".into(); // whitespace ignored
        assert_eq!(
            MajikContact::from_json(s.clone()).unwrap().public_key().raw,
            [1, 2, 3]
        );
        s.public_key_base64 = String::new(); // empty key is structurally valid
        assert!(MajikContact::from_json(s)
            .unwrap()
            .public_key()
            .raw
            .is_empty());
    }

    fn identity() -> MajikMessageIdentityJSON {
        MajikMessageIdentityJSON {
            id: "identity-001".into(),
            user_id: "user-001".into(),
            public_key: b64(&bytes(1, 32)),
            ml_key: "ml".into(),
            phash: "ph".into(),
            label: "Remote User".into(),
            timestamp: "2026-01-05T12:00:00.000Z".into(),
            restricted: false,
        }
    }

    #[test]
    fn from_identity_json_maps_fields() {
        let c = MajikContact::from_identity_json(&identity()).unwrap();
        assert_eq!(c.id(), "identity-001");
        assert_eq!(c.fingerprint(), "identity-001");
        assert_eq!(c.ml_key(), "ml");
        assert_eq!(c.public_key().raw, bytes(1, 32));
        assert_eq!(c.meta.label.as_deref(), Some("Remote User"));
        assert_eq!(
            c.meta.created_at.as_deref(),
            Some("2026-01-05T12:00:00.000Z")
        );
        assert_eq!(
            c.meta.updated_at.as_deref(),
            Some("2026-01-05T12:00:00.000Z")
        );
        assert!(c.is_majikah_identity_checked() && c.is_majikah_registered());
        assert!(!c.is_blocked());
        assert_eq!(c.ed_public_key_base64(), "");
    }

    #[test]
    fn from_identity_json_maps_restricted_to_blocked() {
        let mut i = identity();
        i.restricted = true;
        assert!(MajikContact::from_identity_json(&i).unwrap().is_blocked());
    }

    #[test]
    fn from_identity_json_wraps_failures() {
        let mut i = identity();
        i.public_key = "***".into();
        let e = MajikContact::from_identity_json(&i).unwrap_err();
        assert_eq!(
            e.message(),
            "Failed to create MajikContact from MajikMessageIdentityJSON"
        );
        assert!(std::error::Error::source(&e).is_some());

        let mut i = identity();
        i.id = String::new();
        assert!(MajikContact::from_identity_json(&i).is_err());
    }

    #[test]
    fn identity_json_serde_uses_snake_case() {
        let v = serde_json::to_value(identity()).unwrap();
        assert_eq!(v["user_id"], "user-001");
        assert_eq!(v["ml_key"], "ml");
        let back: MajikMessageIdentityJSON = serde_json::from_value(v).unwrap();
        assert_eq!(back, identity());
    }
}

/* ============================ MajikContactGroup ============================ */

mod group {
    use super::*;

    #[test]
    fn creates_a_standard_custom_group() {
        let g = group("group-001", "Test Group", &[]);
        assert_eq!(g.id(), "group-001");
        assert!(!g.is_system());
        assert_eq!(g.meta.name, "Test Group");
        assert_eq!(g.meta.description, "");
        assert_eq!(g.meta.photo_base64, None);
        assert_eq!(g.meta.color, None);
        assert_eq!(g.meta.created_at, g.meta.updated_at);
        assert!(g.is_empty() && g.member_count() == 0);
    }

    #[test]
    fn create_accepts_meta_and_members() {
        let g = MajikContactGroup::create(
            "g",
            "Team",
            Some(PartialMajikContactGroupMeta {
                description: Some("d".into()),
                color: Some("#fff".into()),
                created_at: Some("2025-01-01T00:00:00.000Z".into()),
                ..Default::default()
            }),
            Some(vec!["a".into(), "b".into()]),
        )
        .unwrap();
        assert_eq!(g.meta.description, "d");
        assert_eq!(g.meta.color.as_deref(), Some("#fff"));
        assert_eq!(g.meta.created_at, "2025-01-01T00:00:00.000Z");
        assert_eq!(g.list_member_ids(), ["a", "b"]);
    }

    #[test]
    fn construction_validation() {
        group_err(
            MajikContactGroup::create("", "N", None, None),
            "Group ID must be a non-empty string",
        );
        group_err(MajikContactGroup::create("  ", "N", None, None), "Group ID");
        group_err(
            MajikContactGroup::create("g", "", None, None),
            "Group name must be a non-empty string",
        );
        group_err(
            MajikContactGroup::create("g", "   ", None, None),
            "Group name",
        );
        group_err(
            MajikContactGroup::create("g", "x".repeat(65), None, None),
            "Group name must not exceed 64 characters",
        );
        assert!(MajikContactGroup::create("g", "x".repeat(64), None, None).is_ok());
        group_err(
            MajikContactGroup::create("g", "n", None, Some(vec!["a".into(), "".into()])),
            "Invalid member ID at index 1",
        );
        group_err(
            MajikContactGroup::create("g", "n", None, Some(vec!["a".into(), "a".into()])),
            "Member IDs must not contain duplicates",
        );
    }

    #[test]
    fn name_length_is_measured_in_utf16_units_like_js() {
        // 33 emoji = 66 UTF-16 units (> 64) although only 33 chars.
        group_err(
            MajikContactGroup::create("g", "😀".repeat(33), None, None),
            "must not exceed 64",
        );
        assert!(MajikContactGroup::create("g", "😀".repeat(32), None, None).is_ok());
    }

    #[test]
    fn reserved_names_are_rejected_case_insensitively() {
        for n in ["Favorites", "favorites", "  BLOCKED  "] {
            group_err(
                MajikContactGroup::create("g", n, None, None),
                "reserved system group name",
            );
        }
    }

    #[test]
    fn system_groups() {
        let f = MajikContactGroup::create_favorites();
        assert!(f.is_system() && f.is_favorites() && !f.is_blocked());
        assert_eq!(f.id(), SYSTEM_GROUP_ID_FAVORITES);
        assert_eq!(f.meta.name, "Favorites");
        let b = MajikContactGroup::create_blocked();
        assert!(b.is_system() && b.is_blocked() && !b.is_favorites());
        assert_eq!(b.id(), "system::blocked");
        assert_eq!(b.meta.name, SystemGroupId::Blocked.name());
    }

    #[test]
    fn system_groups_force_canonical_names() {
        let g = MajikContactGroup::new(MajikContactGroupData {
            id: SYSTEM_GROUP_ID_FAVORITES.into(),
            is_system: Some(true),
            meta: Some(PartialMajikContactGroupMeta {
                name: Some("Forged".into()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(g.meta.name, "Favorites");
    }

    #[test]
    fn unknown_system_ids_skip_name_validation() {
        let g = MajikContactGroup::new(MajikContactGroupData {
            id: "custom-system".into(),
            is_system: Some(true),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(g.meta.name, "");
        assert!(g.is_system());
    }

    #[test]
    fn system_groups_cannot_be_renamed_but_members_can_change() {
        let mut f = MajikContactGroup::create_favorites();
        group_err(
            f.update_name("Mine").map(|_| ()),
            "System group \"Favorites\" cannot be renamed",
        );
        f.add_member("alice").unwrap();
        assert!(f.has_member("alice").unwrap());
    }

    #[test]
    fn is_system_group_id_helper() {
        assert!(is_system_group_id("system::favorites"));
        assert!(is_system_group_id("system::blocked"));
        assert!(!is_system_group_id("system::other"));
        assert_eq!(
            SystemGroupId::from_id("system::blocked"),
            Some(SystemGroupId::Blocked)
        );
    }

    #[test]
    fn metadata_mutation() {
        let mut g = group("g", "Old", &[]);
        let ts = g.meta.updated_at.clone();
        tick();
        g.update_name("  New Name  ")
            .unwrap()
            .update_description("  desc  ")
            .set_color(Some("  #002968 "));
        assert_eq!(g.meta.name, "New Name");
        assert_eq!(g.meta.description, "desc");
        assert_eq!(g.meta.color.as_deref(), Some("#002968"));
        assert!(g.meta.updated_at > ts);
        g.clear_color();
        assert_eq!(g.meta.color, None);
        group_err(g.update_name("").map(|_| ()), "Group name");
        group_err(g.update_name("favorites").map(|_| ()), "reserved");
        assert_eq!(g.meta.name, "New Name");
    }

    #[test]
    fn photo_management() {
        let mut g = group("g", "G", &[]);
        assert!(!g.has_photo());

        block_on(g.set_photo("aW1hZ2VkYXRh")).unwrap();
        assert_eq!(
            g.meta.photo_base64.as_deref(),
            Some("data:image/jpeg;base64,aW1hZ2VkYXRh")
        );
        assert!(g.has_photo());

        let url = "data:image/png;base64,aW1hZ2VkYXRh";
        block_on(g.set_photo(url)).unwrap();
        assert_eq!(g.meta.photo_base64.as_deref(), Some(url));

        block_on(g.set_photo(vec![1u8, 2, 3, 4])).unwrap();
        assert_eq!(
            g.meta.photo_base64.as_deref(),
            Some("data:image/jpeg;base64,AQIDBA==")
        );

        block_on(g.set_photo(PhotoInput::Blob {
            data: vec![1, 2, 3, 4],
            mime_type: Some("image/png".into()),
        }))
        .unwrap();
        assert_eq!(
            g.meta.photo_base64.as_deref(),
            Some("data:image/png;base64,AQIDBA==")
        );

        block_on(g.set_photo(PhotoInput::Blob {
            data: vec![9],
            mime_type: None,
        }))
        .unwrap();
        assert!(g
            .meta
            .photo_base64
            .as_deref()
            .unwrap()
            .starts_with("data:application/octet-stream;base64,"));

        g.clear_photo();
        assert_eq!(g.meta.photo_base64, None);
        assert!(!g.has_photo());
    }

    #[test]
    fn photo_rejections_leave_state_untouched() {
        let mut g = group("g", "G", &[]);
        let ts = g.meta.updated_at.clone();
        for bad in [
            "not base64!!",
            "",
            "data:image/png;base64,",
            "data:text/plain,hello",
            "data:/png;base64,AAAA",
        ] {
            group_err(block_on(g.set_photo(bad)).map(|_| ()), "");
        }
        group_err(
            block_on(g.set_photo("abc===")).map(|_| ()),
            "not a valid base64 string",
        );
        group_err(
            block_on(g.set_photo(PhotoInput::Blob {
                data: vec![],
                mime_type: None,
            }))
            .map(|_| ()),
            "Invalid data-URL format",
        );
        assert_eq!(g.meta.photo_base64, None);
        assert_eq!(g.meta.updated_at, ts);
    }

    #[test]
    fn data_url_validation_matches_the_ts_regex() {
        let ok = |s: &str| block_on(normalize_photo_to_base64(s)).is_ok();
        assert!(ok("data:image/png;base64,AAAA"));
        assert!(!ok("data:image/svg+xml;base64,AAAA")); // '+' is not in the media-type class
        assert!(ok("data:application/x-foo_bar;base64,AA=="));
        assert!(ok("data:image/png;base64,AA-_"));
        assert!(!ok("data:image/png;base64,AA=A"));
        assert!(!ok("data:image/png;charset=x;base64,AAAA"));
    }

    #[test]
    fn raw_base64_accepts_url_safe_alphabet() {
        assert_eq!(
            block_on(normalize_photo_to_base64("ab-_cd==")).unwrap(),
            "data:image/jpeg;base64,ab-_cd=="
        );
    }

    #[test]
    fn membership_management() {
        let mut g = group("g", "G", &[]);
        g.add_member("a").unwrap().add_member("b").unwrap();
        group_err(
            g.add_member("a").map(|_| ()),
            "Contact \"a\" is already a member of group \"G\"",
        );
        group_err(
            g.add_member("").map(|_| ()),
            "Contact ID must be a non-empty string",
        );
        assert_eq!(g.member_count(), 2);

        g.add_member_if_absent("a")
            .unwrap()
            .add_member_if_absent("c")
            .unwrap();
        assert_eq!(g.list_member_ids(), ["a", "b", "c"]);

        group_err(
            g.add_members(&["x", "a", "b"]).map(|_| ()),
            "already members: a, b",
        );
        assert!(
            !g.has_member("x").unwrap(),
            "strict batch adds nothing on failure"
        );
        group_err(g.add_members(&["x", "x"]).map(|_| ()), "duplicates");
        g.add_members(&["x", "y"]).unwrap();
        assert_eq!(g.list_member_ids(), ["a", "b", "c", "x", "y"]);

        g.remove_member("b").unwrap();
        group_err(
            g.remove_member("b").map(|_| ()),
            "Contact \"b\" is not a member of group \"G\"",
        );
        g.remove_member_if_present("b")
            .unwrap()
            .remove_member_if_present("c")
            .unwrap();
        assert_eq!(
            g.list_member_ids(),
            ["a", "x", "y"],
            "removal preserves insertion order"
        );
        group_err(g.has_member("  ").map(|_| ()), "Contact ID");

        g.clear_members();
        assert!(g.is_empty());
    }

    #[test]
    fn idempotent_membership_ops_do_not_touch_the_timestamp() {
        let mut g = group("g", "G", &["a"]);
        let ts = g.meta.updated_at.clone();
        tick();
        g.add_member_if_absent("a")
            .unwrap()
            .remove_member_if_present("zzz")
            .unwrap();
        assert_eq!(g.meta.updated_at, ts);
        g.add_member("b").unwrap();
        assert!(g.meta.updated_at > ts);
    }

    #[test]
    fn list_member_ids_returns_a_copy() {
        let g = group("g", "G", &["a"]);
        let mut ids = g.list_member_ids();
        ids.push("hacked".into());
        assert_eq!(g.member_count(), 1);
    }

    #[test]
    fn static_merge_unions_members_in_order() {
        let a = group("a", "Alpha", &["1", "2"]);
        let b = group("b", "Beta", &["2", "3"]);
        let c = group("c", "Gamma", &["4"]);
        let m = MajikContactGroup::merge(&[&a, &b, &c], None).unwrap();
        assert_eq!(m.id(), "a");
        assert_eq!(m.meta.name, "Alpha");
        assert!(!m.is_system());
        assert_eq!(m.list_member_ids(), ["1", "2", "3", "4"]);
        // sources untouched
        assert_eq!(a.list_member_ids(), ["1", "2"]);
    }

    #[test]
    fn static_merge_options_and_inheritance() {
        let mut a = group("a", "Alpha", &["1"]);
        a.update_description("desc");
        block_on(a.set_photo("aW1hZ2VkYXRh")).unwrap();
        a.set_color(Some("#fff"));
        let b = group("b", "Beta", &["2"]);

        let m = MajikContactGroup::merge(
            &[&a, &b],
            Some(&MajikContactGroupSetOptions {
                id: Some("  all  ".into()),
                name: Some(" All Staff ".into()),
            }),
        )
        .unwrap();
        assert_eq!(m.id(), "all");
        assert_eq!(m.meta.name, "All Staff");
        assert_eq!(m.meta.description, "desc");
        assert_eq!(m.meta.photo_base64, a.meta.photo_base64);
        assert_eq!(m.meta.color, None, "color is not inherited");
    }

    #[test]
    fn static_set_operation_validation() {
        let a = group("a", "Alpha", &["1"]);
        let b = group("b", "Beta", &["1"]);
        group_err(
            MajikContactGroup::merge(&[&a], None),
            "merge: requires at least two groups, but received 1",
        );
        group_err(
            MajikContactGroup::intersect(&[], None),
            "intersect: requires at least two groups, but received 0",
        );
        let opt = |id: Option<&str>, name: Option<&str>| MajikContactGroupSetOptions {
            id: id.map(Into::into),
            name: name.map(Into::into),
        };
        group_err(
            MajikContactGroup::merge(&[&a, &b], Some(&opt(Some(" "), None))),
            "overrideId must be a non-empty string",
        );
        group_err(
            MajikContactGroup::merge(&[&a, &b], Some(&opt(None, Some("")))),
            "overrideName must be a non-empty string",
        );
        group_err(
            MajikContactGroup::merge(&[&a, &b], Some(&opt(None, Some(&"x".repeat(65))))),
            "overrideName must not exceed 64 characters",
        );
        group_err(
            MajikContactGroup::intersect(&[&a, &b], Some(&opt(None, Some("Favorites")))),
            "overrideName: \"Favorites\" is a reserved system group name",
        );
    }

    #[test]
    fn merging_a_system_group_requires_a_name_override() {
        let fav = MajikContactGroup::create_favorites();
        let other = group("o", "Other", &["1"]);
        group_err(
            MajikContactGroup::merge(&[&fav, &other], None),
            "reserved system group name",
        );
        let m = MajikContactGroup::merge(
            &[&fav, &other],
            Some(&MajikContactGroupSetOptions {
                id: Some("combo".into()),
                name: Some("Combo".into()),
            }),
        )
        .unwrap();
        assert!(!m.is_system());
    }

    #[test]
    fn static_intersect() {
        let a = group("a", "Alpha", &["1", "2", "3"]);
        let b = group("b", "Beta", &["3", "2", "9"]);
        let c = group("c", "Gamma", &["2", "3"]);
        let i = MajikContactGroup::intersect(&[&a, &b, &c], None).unwrap();
        assert_eq!(
            i.list_member_ids(),
            ["2", "3"],
            "order follows the first group"
        );
        let none = MajikContactGroup::intersect(&[&a, &group("d", "Delta", &["7"])], None).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn merge_with_mutates_in_place() {
        let mut a = group("a", "Alpha", &["1"]);
        let b = group("b", "Beta", &["1", "2"]);
        let c = group("c", "Gamma", &["3"]);
        group_err(
            a.merge_with(&[]).map(|_| ()),
            "mergeWith requires at least one other group",
        );
        a.merge_with(&[&b, &c]).unwrap();
        assert_eq!(a.list_member_ids(), ["1", "2", "3"]);
        assert_eq!(b.list_member_ids(), ["1", "2"]);
    }

    #[test]
    fn intersect_with_mutates_in_place() {
        let mut a = group("a", "Alpha", &["1", "2", "3"]);
        let b = group("b", "Beta", &["2", "3"]);
        let c = group("c", "Gamma", &["3", "4"]);
        group_err(
            a.intersect_with(&[]).map(|_| ()),
            "intersectWith requires at least one other group",
        );
        a.intersect_with(&[&b, &c]).unwrap();
        assert_eq!(a.list_member_ids(), ["3"]);
    }

    #[test]
    fn json_round_trip_and_wire_format() {
        let mut g = MajikContactGroup::create(
            "eng",
            "Engineering",
            Some(PartialMajikContactGroupMeta {
                description: Some("d".into()),
                ..Default::default()
            }),
            Some(vec!["a".into(), "b".into()]),
        )
        .unwrap();
        g.set_color(Some("#002968"));
        let v = serde_json::to_value(g.to_json()).unwrap();
        assert_eq!(v["id"], "eng");
        assert_eq!(v["isSystem"], false);
        assert_eq!(v["memberIds"], serde_json::json!(["a", "b"]));
        assert_eq!(v["meta"]["name"], "Engineering");
        assert_eq!(v["meta"]["color"], "#002968");
        assert!(v["meta"]["photoBase64"].is_null());
        assert!(v["meta"]["createdAt"].is_string());

        let back = MajikContactGroup::from_json_str(&g.to_json_string()).unwrap();
        assert_eq!(back, g);
    }

    #[test]
    fn color_is_omitted_from_json_when_unset() {
        let v = serde_json::to_value(group("g", "G", &[]).to_json()).unwrap();
        assert!(v["meta"].get("color").is_none());
    }

    #[test]
    fn from_json_applies_constructor_validation() {
        let mut s = group("g", "G", &["a"]).to_json();
        s.member_ids = vec!["a".into(), "a".into()];
        group_err(MajikContactGroup::from_json(s), "duplicates");

        let mut s = group("g", "G", &[]).to_json();
        s.meta.name = "Favorites".into();
        group_err(MajikContactGroup::from_json(s), "reserved");

        group_err(
            MajikContactGroup::from_json_str("[]"),
            "Invalid serialized group data",
        );
        group_err(
            MajikContactGroup::from_json_str("not json"),
            "Invalid serialized group data",
        );
    }

    #[test]
    fn from_json_is_lenient_about_missing_fields_like_the_ts_version() {
        let g = MajikContactGroup::from_json_str(r#"{"id":"g","meta":{"name":"G"}}"#).unwrap();
        assert!(g.is_empty());
        assert!(!g.is_system());
        assert_eq!(g.meta.description, "");
        assert_eq!(g.meta.created_at.len(), 24);
    }

    #[test]
    fn system_group_json_round_trip() {
        let f = MajikContactGroup::create_favorites();
        let back = MajikContactGroup::from_json(f.to_json()).unwrap();
        assert!(back.is_system() && back.is_favorites());
        assert_eq!(back.meta.name, "Favorites");
    }
}

/* ============================ Integration ============================ */

#[test]
fn contacts_work_as_group_members_and_set_ops() {
    let (a, b, c) = (test_contact(1), test_contact(2), test_contact(3));
    assert_ne!(a.id(), b.id());
    assert_ne!(b.id(), c.id());

    let eng = group("engineering", "Engineering", &[a.id(), b.id()]);
    let sec = group("security", "Security", &[b.id(), c.id()]);
    assert!(eng.has_member(a.id()).unwrap());
    assert!(!eng.has_member(c.id()).unwrap());

    let opts = |id: &str, name: &str| MajikContactGroupSetOptions {
        id: Some(id.into()),
        name: Some(name.into()),
    };
    let merged =
        MajikContactGroup::merge(&[&eng, &sec], Some(&opts("all-staff", "All Staff"))).unwrap();
    let shared =
        MajikContactGroup::intersect(&[&eng, &sec], Some(&opts("shared", "Shared"))).unwrap();
    assert_eq!(merged.list_member_ids(), [a.id(), b.id(), c.id()]);
    assert_eq!(shared.list_member_ids(), [b.id()]);
}

#[test]
fn errors_expose_message_cause_and_display() {
    let e = MajikContactError::new("boom");
    assert_eq!(e.to_string(), "boom");
    assert!(e.cause().is_none());
    let e = MajikContactGroupError::with_cause("outer", MajikContactError::new("inner"));
    assert_eq!(e.to_string(), "outer");
    assert_eq!(e.cause().unwrap().to_string(), "inner");
}
