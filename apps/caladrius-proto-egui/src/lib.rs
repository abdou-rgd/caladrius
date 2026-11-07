//! The NCA screen in egui 0.37, for the comparison of wave 3.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]

pub mod app;
pub mod measure;
pub mod model;
pub mod plot;
pub mod state;
pub mod theme;

impl eframe::App for app::App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        app::App::ui(self, ui);
    }
}
