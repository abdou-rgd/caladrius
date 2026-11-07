//! The root of the screen: the handle to the session that every panel holds, the top bar, the keyboard,
//! and the boot (datasets, theme, query parameters).
//!
//! The engine is `caladrius_bridge::Bridge` linked into this very WebAssembly module: a click calls
//! `Session::...`, which calls the engine in process (no worker, no message, no JSON through
//! JavaScript), and the whole answer is back before the next line runs. The session is a plain Rust
//! value (tested natively); what the panels read is a copy of its state in a signal.

use caladrius_bridge::Bridge;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::data_panel::DataPanel;
use crate::plot_panel::PlotPanel;
use crate::results_panel::ResultsPanel;
use crate::session::{Session, State, Status};
use crate::types::Outcome;

pub type Sess = Session<Bridge>;

pub const EDGE_CSV: &str = include_str!("../../../oracle/data/edge_blq.csv");
pub const NCA50_CSV: &str = include_str!("../../../prototypes/nca50/nca50.csv");
pub const DATASETS: [&str; 2] = ["edge_blq", "nca50"];

/// The text of a dataset of the brief and the subject the screen starts on.
pub fn dataset(name: &str) -> Option<(&'static str, Option<&'static str>)> {
    match name {
        "edge_blq" => Some((EDGE_CSV, Some("3"))),
        "nca50" => Some((NCA50_CSV, None)),
        _ => None,
    }
}

/// The handle every panel holds: the session (not reactive, a plain value) and a copy of its state
/// (reactive). `act` is the one way an action reaches the engine.
#[derive(Clone, Copy)]
pub struct Ctx {
    session: StoredValue<Sess, LocalStorage>,
    pub state: RwSignal<State>,
}

impl Ctx {
    pub fn new() -> Self {
        let session = Session::new(Bridge::new());
        let state = RwSignal::new(session.state().clone());
        Ctx {
            session: StoredValue::new_local(session),
            state,
        }
    }

    /// Runs an action of the session, then shows the state it leaves.
    pub fn act(&self, action: impl FnOnce(&mut Sess)) {
        self.session.update_value(action);
        self.state
            .set(self.session.with_value(|s| s.state().clone()));
    }

    /// Loads a dataset of the brief by name.
    pub fn load(&self, name: &str) {
        if let Some((csv, subject)) = dataset(name) {
            self.act(|s| s.load_dataset(name, csv, subject));
        }
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}

fn typing_in(ev: &web_sys::KeyboardEvent) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|el| matches!(el.tag_name().as_str(), "INPUT" | "SELECT" | "TEXTAREA"))
}

/// The keys of the whole screen (the handles, the rows and the candidates have their own).
fn keyboard(ctx: Ctx, ev: web_sys::KeyboardEvent) {
    if ev.ctrl_key() || ev.alt_key() || ev.meta_key() || typing_in(&ev) {
        return;
    }
    let step = |by: isize| {
        let (subjects, current) = ctx.state.with_untracked(|s| {
            (
                s.info
                    .as_ref()
                    .map(|i| i.subjects.clone())
                    .unwrap_or_default(),
                s.subject.clone(),
            )
        });
        let at = subjects.iter().position(|x| *x == current).unwrap_or(0) as isize;
        let next = (at + by).clamp(0, subjects.len().saturating_sub(1) as isize) as usize;
        if let Some(subject) = subjects.get(next) {
            ctx.act(|s| s.select_subject(subject));
        }
    };
    match ev.key().as_str() {
        "r" => ctx.act(|s| s.run()),
        "n" => step(1),
        "p" => step(-1),
        "l" => {
            let log = ctx.state.with_untracked(|s| s.log);
            ctx.act(|s| s.set_log(!log));
        }
        "b" => ctx.act(|s| s.back_to_automatic()),
        _ => return,
    }
    ev.prevent_default();
}

#[component]
pub fn App(ctx: Ctx) -> impl IntoView {
    let _ = window_event_listener(leptos::ev::keydown, move |ev| keyboard(ctx, ev));
    view! {
        <div class="app">
            <TopBar ctx=ctx/>
            <main class="grid">
                <DataPanel ctx=ctx/>
                <PlotPanel ctx=ctx/>
                <ResultsPanel ctx=ctx/>
            </main>
        </div>
    }
}

#[component]
fn TopBar(ctx: Ctx) -> impl IntoView {
    let state = ctx.state;
    let subjects = Memo::new(move |_| {
        state.with(|s| {
            s.info
                .as_ref()
                .map(|i| i.subjects.clone())
                .unwrap_or_default()
        })
    });
    let status_class = move || {
        let kind = match state.with(State::status) {
            Status::None => "none",
            Status::Fresh => "fresh",
            Status::Stale => "stale",
        };
        format!("badge badge-{kind}")
    };
    let status_text = move || match state.with(State::status) {
        Status::None => "not run",
        Status::Fresh => "fresh",
        Status::Stale => "stale",
    };
    let no_result = move || {
        state.with(|s| {
            matches!(
                s.subject_result().map(|r| &r.outcome),
                Some(Outcome::Error(_))
            )
        })
    };
    view! {
        <header class="topbar" role="toolbar" aria-label="Analysis">
            <strong class="brand">"Caladrius"</strong>
            <label class="field">
                <span>"Dataset"</span>
                <select aria-label="Dataset" on:change=move |ev| ctx.load(&event_target_value(&ev))>
                    {DATASETS
                        .iter()
                        .map(|name| {
                            view! {
                                <option value=*name prop:selected=move || state.with(|s| s.dataset_name == *name)>
                                    {*name}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <span class="analysis-label" title="The analysis">
                {move || state.with(|s| s.view.as_ref().map_or_else(|| "New NCA".to_owned(), |v| v.label.clone()))}
            </span>
            <label class="field">
                <span>"Subject"</span>
                <select
                    aria-label="Subject"
                    aria-keyshortcuts="n p"
                    on:change=move |ev| {
                        let subject = event_target_value(&ev);
                        ctx.act(|s| s.select_subject(&subject));
                    }
                >
                    <For
                        each=move || subjects.get()
                        key=|s| s.clone()
                        children=move |subject| {
                            let wanted = subject.clone();
                            let value = subject.clone();
                            view! {
                                <option value=value prop:selected=move || state.with(|s| s.subject == wanted)>
                                    {subject}
                                </option>
                            }
                        }
                    />
                </select>
            </label>
            <button type="button" class="primary" aria-keyshortcuts="r" on:click=move |_| ctx.act(|s| s.run())>
                "Run"
            </button>
            <span class=status_class role="status" aria-label="Status of the result">
                {status_text}
            </span>
            {move || no_result().then(|| view! { <span class="hint">"no result for this subject"</span> })}
            <span class="spacer"></span>
            <span class="hint keys">"keys: r run, n/p subject, l log, b automatic"</span>
        </header>
    }
}
