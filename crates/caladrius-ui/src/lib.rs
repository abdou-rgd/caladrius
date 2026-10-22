#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! Layer L3: the egui interface. It talks to the engine only through the commands of
//! `caladrius-engine`, holds no numerical code and reads no file: the program around it passes
//! the bytes of a CSV file in ([`UiApp::load_csv`]) and asks for a file picker ([`Request`]).
//!
//! - [`logo`]: the Caladrius logo, embedded as PNG renderings of `assets/logo/`.
//! - [`theme`]: the tokens (colours, radii, spacing, font sizes) read from `theme.json`, light and dark.
//! - [`model`]: typed views of the engine's JSON answers.
//! - [`plotdata`]: what the profile plot draws, including the safe semi-log view.
//! - [`rawplot`]: the raw data of a worksheet, linear and semi-log, before any analysis.
//! - [`app`]: the application, its project tree and its screens ([`import`], [`entry`], [`sheet`],
//!   [`nca`]).

pub mod app;
pub mod entry;
pub mod entryflow;
pub mod fit;
pub mod fitform;
pub mod fitplots;
pub mod fitpreset;
pub mod fitresult;
pub mod flow;
pub mod fmt;
pub mod import;
pub mod logo;
pub mod model;
pub mod modelinfo;
pub mod modelpick;
pub mod nca;
pub mod palette;
pub mod plot;
pub mod plotdata;
pub mod projectfile;
pub mod projectmenu;
pub mod rawplot;
pub mod settings;
pub mod settingsflow;
pub mod settingspage;
pub mod sheet;
pub mod sim;
pub mod theme;
pub mod widgets;

pub use app::{Action, Request, Selection, UiApp, UiState};

/// The version of Caladrius, shown on the welcome page and in the status bar.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests;
pub use theme::{ThemeMode, Tokens};
#[cfg(test)]
mod tests_entry;
#[cfg(test)]
mod tests_fitpreset;
#[cfg(test)]
mod tests_palette;
#[cfg(test)]
mod tests_pk2;
#[cfg(test)]
mod tests_project;
#[cfg(test)]
mod tests_settings;
