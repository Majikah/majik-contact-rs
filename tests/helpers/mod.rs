//! Shared test helpers (port of `test/helpers/crypto.ts`).
//!
//! The TS suite generated real identities with `@majikah/majik-key`. `majik-contact` has no
//! dependency on a Rust MajikKey, so fixtures here are deterministic, correctly *shaped*
//! identities (32-byte X25519 key, base64 fingerprint, etc.). The contact model only stores and
//! encodes this material, so behaviour under test is identical.
#![allow(dead_code)]

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use majik_contact::{MajikContact, X25519RawKey};

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

/// Minimal executor for futures that never actually suspend (every non-URL photo path).
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(Arc::new(Noop));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// Deterministic pseudo-random bytes (xorshift64*), distinct per seed.
pub fn bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8
        })
        .collect()
}

pub fn b64(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD.encode(data)
}

/// A fully populated, valid contact for fixture `seed`.
pub fn test_contact(seed: u64) -> MajikContact {
    let public_key = bytes(seed, 32);
    let fingerprint = b64(&bytes(seed + 1000, 32));
    MajikContact::create(
        fingerprint.clone(),
        X25519RawKey::new(public_key),
        b64(&bytes(seed + 2000, 64)),
        fingerprint,
        None,
        Some(b64(&bytes(seed + 3000, 32))),
        Some(b64(&bytes(seed + 4000, 64))),
    )
    .expect("fixture contact is valid")
}
