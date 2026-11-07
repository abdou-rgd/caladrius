//! The NCA screen in Leptos 0.8 (T-137): client side rendering, the engine linked into this module.

mod app;
mod data_panel;
mod dom;
mod measure;
mod plot_panel;
mod plotmap;
mod results;
mod results_panel;
mod roles;
mod session;
mod shell;
mod theme;
mod types;

#[cfg(test)]
mod tests;

use app::{App, Ctx};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::session::CellRole;

const VERSION: &str = env!("CALADRIUS_VERSION");

/// The style sheet made from `theme.json`, put in the head before anything is drawn.
fn inject_theme() {
    let Some(doc) = dom::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(style) = doc.create_element("style") else {
        return;
    };
    style.set_text_content(Some(&theme::css()));
    if let Some(head) = doc.head() {
        let _ = head.append_child(&style);
    }
    if let (Some(root), Some(choice)) = (doc.document_element(), dom::query("theme"))
        && (choice == "light" || choice == "dark")
    {
        let _ = root.set_attribute("data-theme", &choice);
    }
}

/// `?stale=1`: edit one concentration so the analysis shown is out of date (for the screenshots).
fn make_stale(ctx: Ctx) {
    let target = ctx.state.with_untracked(|s| {
        s.rows
            .iter()
            .find(|r| r.subject == s.subject && r.conc.is_some_and(|c| c > 0.0))
            .map(|r| (r.index, r.conc.unwrap_or(0.0)))
    });
    if let Some((index, conc)) = target {
        let text = caladrius_format::exact(conc * 2.0);
        ctx.act(|s| s.edit_cell(index, CellRole::Concentration, &text));
    }
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// The measure of the wave 3 brief: see `measure.rs` for the definitions.
async fn run_measure(ctx: Ctx, first_frame: f64) -> measure::Report {
    let mut report = measure::Report::new(VERSION, round3(first_frame));
    ctx.load("nca50");
    let _ = dom::next_paint(2000).await;
    for k in 0..measure::DRAGS {
        let subject = measure::subject_of(k);
        ctx.act(|s| s.select_subject(&subject));
        let previous = dom::next_paint(2000).await.0;
        let samples = ctx.state.with_untracked(|s| s.samples());
        let start = match measure::start_of(k, &samples) {
            Ok(t) => t,
            Err(e) => {
                report.errors.push(e);
                continue;
            }
        };
        let began = dom::now();
        ctx.act(|s| s.set_range_start(start));
        let (painted, timed_out) = dom::next_paint(2000).await;
        if timed_out {
            report.errors.push(format!("drag {k}: no frame within 2 s"));
        }
        report.input_to_paint_ms.push(round3(painted - began));
        report.frame_ms.push(round3(painted - previous));
    }
    if let Some(err) = ctx.state.with_untracked(|s| s.error.clone()) {
        report.errors.push(format!("last state error: {err}"));
    }
    if let Some(doc) = dom::window().and_then(|w| w.document()) {
        if let Ok(Some(details)) = doc.query_selector("details[data-measure-open]") {
            let _ = details.set_attribute("open", "");
        }
        let _ = dom::next_paint(2000).await;
        let expected = ctx.state.with_untracked(measure::expected_cells);
        let mut shown = Vec::new();
        if let Ok(nodes) = doc.query_selector_all("[data-cell]") {
            for i in 0..nodes.length() {
                if let Some(el) = nodes
                    .item(i)
                    .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
                {
                    shown.push((
                        el.get_attribute("data-cell").unwrap_or_default(),
                        el.text_content().unwrap_or_default(),
                    ));
                }
            }
        }
        let (checked, equal, errors) = measure::compare_cells(&expected, &shown);
        report.cells_checked = checked;
        report.cells_equal = equal;
        report.errors.extend(errors);
    }
    report
}

async fn boot(ctx: Ctx) {
    let (frame, _) = dom::next_paint(2000).await;
    let in_tauri = shell::in_tauri();
    let (requested, first_frame) = if in_tauri {
        match shell::measure_requested().await {
            Some(ask) => (ask.requested, ask.since_start_ms + (frame - dom::now())),
            None => (false, frame),
        }
    } else {
        (dom::query("measure").as_deref() == Some("1"), frame)
    };
    if !requested {
        return;
    }
    let report = run_measure(ctx, first_frame).await;
    dom::sleep(3000).await;
    let json = report.json();
    web_sys::console::log_1(&format!("CALADRIUS_MEASURE {json}").into());
    if let Some(win) = dom::window() {
        let _ = js_sys::Reflect::set(&win, &"__CALADRIUS_MEASURE__".into(), &json.as_str().into());
    }
    if in_tauri && let Err(e) = shell::measure_write(&json).await {
        web_sys::console::error_1(&e.into());
    }
}

fn main() {
    inject_theme();
    mount_to_body(|| {
        let ctx = Ctx::new();
        ctx.load(&dom::query("dataset").unwrap_or_else(|| "edge_blq".to_owned()));
        if dom::query("stale").as_deref() == Some("1") {
            make_stale(ctx);
        }
        leptos::task::spawn_local(boot(ctx));
        view! { <App ctx=ctx/> }
    });
}
