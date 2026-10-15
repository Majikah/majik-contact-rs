//! Utilities (port of `utils.ts`).

use base64::{
    alphabet,
    engine::{general_purpose, DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
    Engine as _,
};
use time::OffsetDateTime;

use crate::constants::SystemGroupId;
use crate::errors::MajikContactGroupError;
use crate::types::PhotoInput;

/* ---------------------------------
 * Time
 * --------------------------------- */

/// Current time as an ISO 8601 string with millisecond precision and `Z` suffix,
/// identical in shape to JavaScript's `new Date().toISOString()`.
pub(crate) fn now_iso() -> String {
    let t = OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
        t.millisecond()
    )
}

/* ---------------------------------
 * JS-compatible string helpers
 * --------------------------------- */

/// ECMAScript `WhiteSpace` + `LineTerminator` (differs from Rust's `char::is_whitespace`:
/// includes U+FEFF, excludes U+0085).
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

/// Equivalent of JavaScript's `String.prototype.trim`.
pub(crate) fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// Equivalent of JavaScript's `String.prototype.length` (UTF-16 code units), so length limits
/// agree with the TypeScript implementation for emoji and other astral characters.
pub(crate) fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/* ---------------------------------
 * Base64 helpers
 * --------------------------------- */

/// Decoder that behaves like `atob`: standard alphabet, optional padding, ignores trailing bits.
const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

/// `arrayToBase64` / `arrayBufferToBase64`.
pub(crate) fn array_to_base64(data: &[u8]) -> String {
    general_purpose::STANDARD.encode(data)
}

/// `base64ToUint8Array` / `base64ToArrayBuffer` (ASCII whitespace is ignored, like `atob`).
pub(crate) fn base64_to_bytes(base64: &str) -> Result<Vec<u8>, base64::DecodeError> {
    let cleaned: String = base64
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    LENIENT.decode(cleaned)
}

/// `base64ToUtf8` (lossy, like `TextDecoder`).
#[allow(dead_code)]
pub(crate) fn base64_to_utf8(base64: &str) -> Result<String, base64::DecodeError> {
    Ok(String::from_utf8_lossy(&base64_to_bytes(base64)?).into_owned())
}

/// `utf8ToBase64`.
#[allow(dead_code)]
pub(crate) fn utf8_to_base64(s: &str) -> String {
    array_to_base64(s.as_bytes())
}

/// `concatUint8Arrays` / `concatArrayBuffers`.
#[allow(dead_code)]
pub(crate) fn concat_bytes(a: &[u8], b: &[u8]) -> Vec<u8> {
    [a, b].concat()
}

/* ---------------------------------
 * Photo normalization
 * --------------------------------- */

fn group_err(msg: &str) -> MajikContactGroupError {
    MajikContactGroupError::new(msg)
}

/// Accepts any photo input format and always returns a base64 data-URL string.
///
/// Supported inputs:
///  - Already a base64 data-URL: validated and returned as-is
///  - Raw base64 string (no prefix): prefixed with `data:image/jpeg;base64,`
///  - `http(s)` URL: fetched and converted (requires the `fetch` feature)
///  - Blob: converted using its MIME type
///  - Raw bytes: converted as `image/jpeg`
///
/// The `null`/`undefined` and "unsupported type" errors of the TS version cannot occur: the
/// type system rules them out.
pub async fn normalize_photo_to_base64(
    input: impl Into<PhotoInput>,
) -> Result<String, MajikContactGroupError> {
    let input = input.into();
    if let PhotoInput::Text(s) = &input {
        if !s.starts_with("data:") && has_http_scheme(s) {
            return fetch_photo(s).await;
        }
    }
    normalize_without_network(input)
}

fn normalize_without_network(input: PhotoInput) -> Result<String, MajikContactGroupError> {
    match input {
        PhotoInput::Text(s) => {
            if s.starts_with("data:") {
                assert_valid_data_url(&s)?;
                return Ok(s);
            }
            // (http(s) URLs are handled by the caller)
            if !is_valid_base64(&s) {
                return Err(group_err(
                    "Photo string is not a valid base64 string or recognized URL format",
                ));
            }
            Ok(format!("data:image/jpeg;base64,{s}"))
        }
        PhotoInput::Bytes(bytes) => Ok(format!(
            "data:image/jpeg;base64,{}",
            array_to_base64(&bytes)
        )),
        PhotoInput::Blob { data, mime_type } => blob_to_data_url(&data, mime_type.as_deref()),
    }
}

fn blob_to_data_url(data: &[u8], mime: Option<&str>) -> Result<String, MajikContactGroupError> {
    // FileReader.readAsDataURL: an empty blob type becomes application/octet-stream.
    let mime = mime
        .filter(|m| !m.is_empty())
        .unwrap_or("application/octet-stream");
    let url = format!("data:{mime};base64,{}", array_to_base64(data));
    assert_valid_data_url(&url)?;
    Ok(url)
}

fn has_http_scheme(s: &str) -> bool {
    let b = s.as_bytes();
    let starts = |p: &str| b.len() >= p.len() && b[..p.len()].eq_ignore_ascii_case(p.as_bytes());
    starts("http://") || starts("https://")
}

#[cfg(feature = "fetch")]
async fn fetch_photo(url: &str) -> Result<String, MajikContactGroupError> {
    const FAIL: &str = "Failed to fetch photo from URL";
    let response = reqwest::get(url)
        .await
        .map_err(|e| MajikContactGroupError::with_cause(FAIL, e))?;
    if !response.status().is_success() {
        return Err(MajikContactGroupError::new(format!(
            "Failed to fetch photo from URL: HTTP {}",
            response.status().as_u16()
        )));
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
    let bytes = response
        .bytes()
        .await
        .map_err(|e| MajikContactGroupError::with_cause(FAIL, e))?;
    blob_to_data_url(&bytes, mime.as_deref())
}

#[cfg(not(feature = "fetch"))]
async fn fetch_photo(_url: &str) -> Result<String, MajikContactGroupError> {
    let cause: crate::errors::BoxError = "the `fetch` cargo feature is disabled".into();
    Err(MajikContactGroupError::with_cause(
        "Failed to fetch photo from URL",
        cause,
    ))
}

/// `/^[A-Za-z0-9+/\-_]*={0,2}$/` and non-empty. Allows standard and URL-safe base64.
fn is_valid_base64(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let body = s.trim_end_matches('=');
    s.len() - body.len() <= 2 && body.bytes().all(is_b64_char)
}

fn is_b64_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_')
}

/// Media-type token: `[a-zA-Z0-9][a-zA-Z0-9!#$&\-^_]*`.
fn is_media_token(s: &str) -> bool {
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    bytes.all(|b| {
        b.is_ascii_alphanumeric() || matches!(b, b'!' | b'#' | b'$' | b'&' | b'-' | b'^' | b'_')
    })
}

/// Validates `data:<type>/<subtype>;base64,<payload>=*` (payload: one or more base64 chars).
fn assert_valid_data_url(data_url: &str) -> Result<(), MajikContactGroupError> {
    let valid = (|| {
        let rest = data_url.strip_prefix("data:")?;
        let (ty, rest) = rest.split_once('/')?;
        let (subtype, rest) = rest.split_once(';')?;
        let payload = rest.strip_prefix("base64,")?;
        let body = payload.trim_end_matches('=');
        (is_media_token(ty)
            && is_media_token(subtype)
            && !body.is_empty()
            && body.bytes().all(is_b64_char))
        .then_some(())
    })();
    valid.ok_or_else(|| {
        group_err("Invalid data-URL format. Expected: data:<mediatype>;base64,<data>")
    })
}

/// `isSystemGroupId`.
pub fn is_system_group_id(id: &str) -> bool {
    SystemGroupId::from_id(id).is_some()
}
