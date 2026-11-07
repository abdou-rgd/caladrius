//! The results: the terminal phase in words, the flags, the headline parameters, "All parameters"
//! with Copy as TSV, the errors in place, and the seal line.

use caladrius_format as fmt;
use leptos::prelude::*;

use crate::app::Ctx;
use crate::dom;
use crate::results::{Cell, all_cells, headline_cells, results_tsv, terminal_phase_sentence};
use crate::session::{Status, stale_sentence};
use crate::types::Outcome;

fn rows(cells: Vec<Cell>, prefix: &'static str) -> impl IntoView {
    cells
        .into_iter()
        .map(|c| {
            let value_class = if c.calculated { "value" } else { "value absent" };
            view! {
                <tr class=if c.flagged { "flagged" } else { "" } title=c.sentence.clone()>
                    <th scope="row">{c.label.clone()}</th>
                    <td class=value_class data-cell=format!("{prefix}:{}", c.key)>{c.text.clone()}</td>
                    <td class="unit" data-cell=format!("{prefix}-unit:{}", c.key)>{c.unit.clone()}</td>
                    <td class="flagmark">
                        {c.flagged.then(|| view! { <span title="A flag concerns this parameter" aria-label="flagged">"!"</span> })}
                    </td>
                </tr>
            }
        })
        .collect_view()
}

#[component]
pub fn ResultsPanel(ctx: Ctx) -> impl IntoView {
    let state = ctx.state;
    let copied = RwSignal::new(String::new());
    let headline = move || {
        state.with(|s| match (s.subject_result(), s.ok()) {
            (Some(subject), Some(ok)) => headline_cells(subject, ok, &s.units(), true),
            _ => Vec::new(),
        })
    };
    let all = move || {
        state.with(|s| match (s.subject_result(), s.ok()) {
            (Some(subject), Some(ok)) => all_cells(subject, ok, &s.units(), true),
            _ => Vec::new(),
        })
    };
    let has_ok = move || state.with(|s| s.ok().is_some());
    let failure = move || {
        state.with(|s| match s.subject_result().map(|r| &r.outcome) {
            Some(Outcome::Error(sentence)) => Some(fmt::with_labels(sentence)),
            _ => None,
        })
    };
    let copy = move |cells: Vec<Cell>| {
        let text = results_tsv(&cells);
        leptos::task::spawn_local(async move {
            let done = dom::copy_text(&text).await;
            copied.set(if done {
                "Copied as tab-separated text, with a header row.".to_owned()
            } else {
                "The clipboard is not available here.".to_owned()
            });
        });
    };
    let phase = move || {
        state.with(|s| {
            terminal_phase_sentence(
                s.selected_candidate(),
                s.is_manual(),
                s.units().time.as_deref(),
            )
        })
    };
    let flags = move || {
        state.with(|s| {
            s.subject_result()
                .map(|r| {
                    r.flag_messages
                        .iter()
                        .map(|m| fmt::with_labels(m))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    };
    let empty = move || {
        state.with(|s| {
            let nothing = s.ok().is_none()
                && s.error.is_none()
                && !matches!(
                    s.subject_result().map(|r| &r.outcome),
                    Some(Outcome::Error(_))
                );
            nothing.then_some({
                if s.ready {
                    "Run the analysis to see the results."
                } else {
                    "Loading the engine..."
                }
            })
        })
    };
    view! {
        <section class="panel results-panel" aria-label="Results">
            <div class="panel-head">
                <span>{move || state.with(|s| format!("Results · subject {}", s.subject))}</span>
                <span class="spacer"></span>
                <button type="button" disabled=move || headline().is_empty() on:click=move |_| copy(headline())>
                    "Copy"
                </button>
            </div>
            <div class="scroll pad">
                {move || state.with(|s| s.error.clone()).map(|e| view! { <p class="error" role="alert">{e}</p> })}
                {move || failure().map(|e| view! { <p class="error" role="alert">{e}</p> })}
                {move || {
                    state
                        .with(|s| (s.status() == Status::Stale).then(|| stale_sentence(&s.stale_reasons)))
                        .map(|t| view! { <p class="stale-note" role="status">{t}</p> })
                }}
                <Show when=has_ok>
                    <p class="phase">{phase}</p>
                    <ul class="flags" aria-label="Flags" hidden=move || flags().is_empty()>
                        {move || flags().into_iter().map(|m| view! { <li>{m}</li> }).collect_view()}
                    </ul>
                    <table class="results" aria-label="Headline parameters">
                        <thead>
                            <tr>
                                <th>"Parameter"</th>
                                <th>"Value"</th>
                                <th>"Unit"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>{move || rows(headline(), "h")}</tbody>
                    </table>
                    <details data-measure-open>
                        <summary>"All parameters"</summary>
                        <div class="row-actions">
                            <button type="button" on:click=move |_| copy(all())>"Copy all"</button>
                        </div>
                        <table class="results" aria-label="All parameters">
                            <tbody>{move || rows(all(), "a")}</tbody>
                        </table>
                    </details>
                </Show>
                {move || empty().map(|t| view! { <p class="empty">{t}</p> })}
                <p class="hint" role="status" aria-live="polite">{move || copied.get()}</p>
            </div>
            <footer class="seal" aria-label="Seal">
                {move || {
                    state.with(|s| match &s.seal {
                        Some(seal) => {
                            let short: String = seal.sha256.chars().take(12).collect();
                            let engine = seal.engine.clone().unwrap_or_else(|| "?".to_owned());
                            let command = s.command.clone().unwrap_or_else(|| "-".to_owned());
                            view! {
                                <span>{format!("seal {short}")}</span>
                                <span>{format!("engine {engine}")}</span>
                                <span>{format!("command {command}")}</span>
                            }
                            .into_any()
                        }
                        None => view! { <span class="muted">"no seal yet"</span> }.into_any(),
                    })
                }}
            </footer>
        </section>
    }
}
