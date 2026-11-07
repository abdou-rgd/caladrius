//! The measure mode of the brief (`CALADRIUS_MEASURE=<file>`): 100 drags through the handler a
//! pointer drag uses, then a check of every result cell, three seconds of waiting, one JSON file.
//!
//! Definitions, both on the CPU side of the app, in milliseconds:
//! - `input_to_paint_ms`: from the call of the drag handler to the end of the frame that shows
//!   its result (the engine run, the scene, the layout and the painting commands).
//! - `frame_ms`: the duration of that whole frame's `App::ui`.
//!
//! The GPU presentation after the frame is not in these two numbers.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::Context;
use serde_json::json;

use crate::state::{Handle, Screen};

/// The 50 subjects of 20 points, read at build time from the prototypes folder.
const NCA50: &str = include_str!("../../../prototypes/nca50/nca50.csv");
const DRAGS: usize = 100;
const CYCLE: [usize; 4] = [3, 4, 5, 6];

#[derive(Debug)]
enum Stage {
    First,
    Load,
    Drags,
    Wait(Instant),
    Done,
}

pub struct Measure {
    path: PathBuf,
    process_start: Instant,
    stage: Stage,
    k: usize,
    issued: Option<Instant>,
    first_frame_ms: f64,
    input_to_paint_ms: Vec<f64>,
    frame_ms: Vec<f64>,
    cells_checked: usize,
    cells_equal: usize,
    errors: Vec<String>,
}

impl Measure {
    pub fn new(path: PathBuf, process_start: Instant) -> Measure {
        Measure {
            path,
            process_start,
            stage: Stage::First,
            k: 0,
            issued: None,
            first_frame_ms: 0.0,
            input_to_paint_ms: Vec::new(),
            frame_ms: Vec::new(),
            cells_checked: 0,
            cells_equal: 0,
            errors: Vec::new(),
        }
    }

    /// Before the panes are drawn: load the data or make the next drag.
    pub fn before_frame(&mut self, screen: &mut Screen, ctx: &Context) {
        match self.stage {
            Stage::Load => {
                screen.all_open = true;
                match screen.load_csv("nca50", NCA50, "1") {
                    Ok(()) => {
                        self.stage = Stage::Drags;
                        // The load frame also carries the first drag.
                        self.drag(screen);
                    }
                    Err(message) => {
                        self.errors.push(format!("load nca50: {message}"));
                        self.stage = Stage::Wait(Instant::now());
                    }
                }
                ctx.request_repaint();
            }
            Stage::Drags => self.drag(screen),
            _ => {}
        }
    }

    /// Drag k: subject 1 + k % 50, the start of the λz range on the last 3, 4, 5 or 6 points.
    fn drag(&mut self, screen: &mut Screen) {
        let k = self.k;
        let subject = format!("{}", 1 + k % 50);
        screen.select_subject(&subject, false);
        let times = screen.eligible_times();
        let n = CYCLE[k % CYCLE.len()];
        let Some(start) = times
            .len()
            .checked_sub(n)
            .and_then(|i| times.get(i).copied())
        else {
            self.errors.push(format!(
                "drag {k}: subject {subject} has fewer than {n} points"
            ));
            self.issued = Some(Instant::now());
            return;
        };
        screen.notice = None;
        self.issued = Some(Instant::now());
        if !screen.drag_handle(Handle::Start, start) {
            let why = screen.notice.clone().unwrap_or_else(|| "no run".to_owned());
            self.errors.push(format!(
                "drag {k}: the handler did not run the engine ({why})"
            ));
        }
        if let Some(e) = &screen.error {
            self.errors.push(format!("drag {k}: {e}"));
        }
    }

    /// After the panes are drawn: record the frame and move to the next stage.
    pub fn after_frame(&mut self, screen: &mut Screen, ctx: &Context, frame_start: Instant) {
        match self.stage {
            Stage::First => {
                self.first_frame_ms = self.process_start.elapsed().as_secs_f64() * 1000.0;
                self.stage = Stage::Load;
                ctx.request_repaint();
            }
            Stage::Drags => {
                if let Some(t) = self.issued.take() {
                    self.input_to_paint_ms
                        .push(t.elapsed().as_secs_f64() * 1000.0);
                }
                self.frame_ms
                    .push(frame_start.elapsed().as_secs_f64() * 1000.0);
                self.k += 1;
                if self.k >= DRAGS {
                    self.check(screen);
                    self.stage = Stage::Wait(Instant::now());
                }
                ctx.request_repaint();
            }
            Stage::Wait(since) => {
                if since.elapsed() >= Duration::from_secs(3) {
                    self.write();
                    self.stage = Stage::Done;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
            Stage::Load | Stage::Done => {}
        }
    }

    /// Compares each cell drawn on the screen with the value formatted from the JSON of the answer.
    fn check(&mut self, screen: &Screen) {
        let expected = screen.expected_cells();
        if expected.is_empty() {
            self.errors
                .push("check: the last answer has no parameters".to_owned());
        }
        for (key, text) in &expected {
            self.cells_checked += 1;
            match screen.shown.iter().find(|(k, _)| k == key) {
                Some((_, shown)) if shown == text => self.cells_equal += 1,
                Some((_, shown)) => {
                    self.errors
                        .push(format!("cell {key}: screen `{shown}`, answer `{text}`"));
                }
                None => self.errors.push(format!("cell {key}: not drawn")),
            }
        }
    }

    fn write(&mut self) {
        let version = std::process::Command::new("git")
            .args(["describe", "--always", "--dirty"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_else(|| "unknown".to_owned());
        let doc = json!({
            "stack": "egui",
            "version": version,
            "first_frame_ms": self.first_frame_ms,
            "drags": DRAGS,
            "input_to_paint_ms": self.input_to_paint_ms,
            "frame_ms": self.frame_ms,
            "cells_checked": self.cells_checked,
            "cells_equal": self.cells_equal,
            "errors": self.errors,
        });
        // The same line as the browser prototypes print, so a shell can keep the file when this
        // program is not allowed to write one (it is refused under target/ on this PC).
        println!("CALADRIUS_MEASURE {doc}");
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        if let Err(e) = std::fs::write(&self.path, text) {
            eprintln!("measure: cannot write {}: {e}", self.path.display());
        }
    }
}
