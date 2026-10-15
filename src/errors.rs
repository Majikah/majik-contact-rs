//! Errors (port of `errors.ts`).
//!
//! The TS classes carry a `message` and an optional `cause`; the Rust structs do the same and
//! expose the cause through [`std::error::Error::source`].

use std::error::Error as StdError;

/// Boxed underlying error, the Rust equivalent of `cause?: unknown`.
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

macro_rules! majik_error {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, thiserror::Error)]
        #[error("{message}")]
        pub struct $name {
            message: String,
            #[source]
            cause: Option<BoxError>,
        }

        impl $name {
            /// Creates an error with no underlying cause.
            pub fn new(message: impl Into<String>) -> Self {
                Self { message: message.into(), cause: None }
            }

            /// Creates an error wrapping an underlying cause.
            pub fn with_cause(
                message: impl Into<String>,
                cause: impl Into<BoxError>,
            ) -> Self {
                Self { message: message.into(), cause: Some(cause.into()) }
            }

            /// Human-readable error message.
            pub fn message(&self) -> &str {
                &self.message
            }

            /// Underlying cause, if any.
            pub fn cause(&self) -> Option<&(dyn StdError + Send + Sync + 'static)> {
                self.cause.as_deref()
            }
        }
    };
}

majik_error!(
    /// Error raised by [`MajikContact`](crate::MajikContact) operations.
    MajikContactError
);
majik_error!(
    /// Error raised by [`MajikContactGroup`](crate::MajikContactGroup) operations.
    MajikContactGroupError
);
