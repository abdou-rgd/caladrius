//! `cargo run -p caladrius-proto-egui`: the NCA screen on edge_blq, subject 3.
//! With `CALADRIUS_MEASURE=<file>` it runs the measure of the brief and exits.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]

use std::time::Instant;

use caladrius_proto_egui::{app::App, measure::Measure};

fn main() -> eframe::Result {
    let process_start = Instant::now();
    let dark = std::env::var("CALADRIUS_DARK").is_ok_and(|v| v == "1");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Caladrius, NCA (egui)")
            .with_inner_size([1440.0, 900.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "Caladrius, NCA (egui)",
        options,
        Box::new(move |_cc| {
            let mut app = App::new(dark);
            if let Ok(path) = std::env::var("CALADRIUS_MEASURE") {
                app.measure = Some(Measure::new(path.into(), process_start));
            }
            Ok(Box::new(app))
        }),
    )
}
