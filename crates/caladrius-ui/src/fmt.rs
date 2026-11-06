//! Display formatting: numbers for the eye, names for parameters. Never rounds a stored value.
//!
//! The code lives in `caladrius-format` (layer L1), so that the browser page and the desktop
//! webview format a number with the same functions as this interface (T-134). This module
//! re-exports it, so no call site changes. How numbers are shown is a setting of the person
//! ([`crate::settings`]); the application hands it to the crate at the start of every frame
//! ([`set_display`]).

pub use caladrius_format::*;
