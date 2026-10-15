# Majik Contact

[![Developed by Zelijah](https://img.shields.io/badge/Developed%20by-Zelijah-red?logo=github&logoColor=white&style=for-the-badge)](https://thezelijah.world) ![GitHub Sponsors](https://img.shields.io/github/sponsors/jedlsf?style=for-the-badge&label=Sponsors&link=https%3A%2F%2Fgithub.com%2Fsponsors%2Fjedlsf)

[![Crates.io](https://img.shields.io/crates/v/majik-contact.svg?logo=rust&color=D34516)](https://crates.io/crates/majik-contact) [![Documentation](https://docs.rs/majik-contact/badge.svg)](https://docs.rs/majik-contact) ![crates downloads](https://img.shields.io/crates/d/majik-contact?logo=rust&color=D34516) [![License](https://img.shields.io/badge/License-Apache_2.0-D34516.svg)](https://opensource.org/licenses/Apache-2.0) ![Rust](https://img.shields.io/badge/Rust-Ready-D34516) 

Pure-Rust port of `@majikah/majik-contact`: a cryptographically secure contact and identity
model for Majikah (X25519 + ML-KEM / Ed25519 + ML-DSA public key material, metadata, and contact groups).

A lightweight, cryptographically secure contact model for building zero-trust, end-to-end encrypted systems.

Majik Contact provides a structured way to represent identities using public keys, fingerprints, and post-quantum–ready cryptography (ML-KEM, ML-DSA). It is designed for use within the Majikah ecosystem and other decentralized messaging applications.

## Layout (mirrors the TypeScript package)

| TypeScript                      | Rust                                 |
|---------------------------------|--------------------------------------|
| `src/index.ts`                  | `src/mod.rs` (crate root, see below) |
| `src/types.ts`                  | `src/types.rs` (interfaces → structs)|
| `src/constants.ts`              | `src/constants.rs`                   |
| `src/errors.ts`                 | `src/errors.rs`                      |
| `src/utils.ts`                  | `src/utils.rs`                       |
| `src/contacts/majik-contact.ts` | `src/contacts/majik_contact.rs`      |
| `src/contacts/majik-contact-group.ts` | `src/contacts/majik_contact_group.rs` |
| *(folder)*                      | `src/contacts/mod.rs`                |
| `test/majik-contact.test.ts`    | `tests/majik_contact.rs`             |
| `test/helpers/crypto.ts`        | `tests/helpers/mod.rs`               |

The crate root is `src/mod.rs` via `[lib] path = "src/mod.rs"` in `Cargo.toml`. To use the
conventional name instead, rename it to `lib.rs` and delete the `path` line.

## Crates

`serde` / `serde_json` (wire-compatible JSON), `base64`, `indexmap` (insertion-ordered member set,
like JS `Set`), `thiserror` (errors with `cause`), `time` (ISO-8601 timestamps), and optional
`reqwest` (feature `fetch`, on by default) for http(s) group photos.

## Porting notes

- **Wire format is unchanged.** Field names are identical to the TS JSON (`publicKeyBase64`,
  `majikah_registered`, `memberIds`, `isSystem`, …), so records persisted by the TS package load as-is.
- **Naming.** `camelCase` → `snake_case`; `getAddress()`/`getDisplayName()` → `address()`/`display_name()`.
  `MajikContact.isBlocked(contact)` (static) → `MajikContact::is_contact_blocked`.
  `MAJIK_API_RESPONSE` → `MajikApiResponse`. `SYSTEM_GROUP_IDS`/`SYSTEM_GROUP_NAMES` → `SystemGroupId` enum
  (`as_str()`, `name()`) plus `SYSTEM_GROUP_ID_FAVORITES` / `SYSTEM_GROUP_ID_BLOCKED`.
- **Sync API.** The TS methods that were `async` only for legacy WebCrypto support
  (`toJSON`, `toContactCard`, `getAddress`, `fromIdentityJSON`) are plain functions. Only `set_photo` /
  `normalize_photo_to_base64` are `async` (URL fetching).
- **`TMeta` generic → `MajikContactMeta.extra`.** App-specific metadata fields are kept in a flattened
  JSON map and survive round-trips. `Partial<MajikContactMeta>` is `MajikContactMeta` itself (all fields optional).
- **Read-only fields** (`id`, `fingerprint`, keys) are private with accessors; `meta` stays public.
- **Dropped checks.** Runtime `typeof`/`null`/`instanceof` validation and `Object.freeze` are enforced by Rust's types.
  Behavioural validation (non-empty IDs, 64-char names, reserved names, duplicates, …) is unchanged, including
  JS-compatible `trim()` and UTF-16 length counting.
- **Photos.** `PhotoInput::{Text, Bytes, Blob}` replaces the `unknown` parameter. As in TS, `svg+xml` data-URLs
  are rejected by the data-URL validator.



---

## Contributing

If you want to contribute or help extend support, reach out via email. All contributions are welcome!

---

## License

[Apache-2.0](LICENSE) — free for personal and commercial use.

---

## Author

Developed by **Josef Elijah Fabian (Zelijah)** | [Majikah Solutions OPC](https://majikah.solutions/about)

**Developer**: Josef Elijah Fabian  
**GitHub**: [https://github.com/jedlsf](https://github.com/jedlsf)  
**Project Repository**: [https://github.com/Majikah/majik-contact-rs](https://github.com/Majikah/majik-contact-rs)

---

## Contact

- **Business Email**: [business@thezelijah.world](mailto:business@thezelijah.world)
- **Official Website**: [https://www.thezelijah.world](https://www.thezelijah.world)