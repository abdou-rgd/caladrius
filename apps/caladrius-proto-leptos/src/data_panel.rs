//! The data table: one row per sample of the subject, the time and the concentration edited in place,
//! and the chips that say what each sample is in the analysis.

use std::collections::BTreeMap;

use caladrius_format as fmt;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::app::Ctx;
use crate::roles::{Chip, chips_for};
use crate::session::{CellRole, State};

/// The shown text of a cell.
fn shown(state: &State, index: usize, role: CellRole) -> String {
    match (state.rows.get(index), role) {
        (Some(row), CellRole::Time) => fmt::exact(row.time),
        (Some(row), CellRole::Concentration) => row.conc.map_or_else(String::new, fmt::exact),
        (None, _) => String::new(),
    }
}

/// A cell edited in place: Enter or leaving it commits, Escape undoes.
#[component]
fn NumberCell(ctx: Ctx, index: usize, role: CellRole) -> impl IntoView {
    let state = ctx.state;
    let text = move || state.with(|s| shown(s, index, role));
    let label = match role {
        CellRole::Time => format!("Time of row {}", fmt::number((index + 1) as f64)),
        CellRole::Concentration => {
            format!("Concentration of row {}", fmt::number((index + 1) as f64))
        }
    };
    let commit = move |input: &web_sys::HtmlInputElement| {
        let typed = input.value();
        let now = text();
        if typed.trim() != now {
            ctx.act(|s| s.edit_cell(index, role, &typed));
        }
        // A refused value leaves the sheet as it was: show what the sheet holds.
        input.set_value(&text());
    };
    view! {
        <input
            class="num"
            inputmode="decimal"
            aria-label=label
            prop:value=text
            on:blur=move |ev| {
                if let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                    commit(&input);
                }
            }
            on:keydown=move |ev| {
                let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
                match ev.key().as_str() {
                    "Enter" => {
                        commit(&input);
                        let _ = input.blur();
                    }
                    "Escape" => {
                        input.set_value(&text());
                        let _ = input.blur();
                    }
                    _ => {}
                }
            }
        />
    }
}

#[component]
pub fn DataPanel(ctx: Ctx) -> impl IntoView {
    let state = ctx.state;
    let indexes = move || {
        state.with(|s| {
            s.rows
                .iter()
                .filter(|r| r.subject == s.subject)
                .map(|r| r.index)
                .collect::<Vec<_>>()
        })
    };
    let chips: Memo<BTreeMap<usize, Vec<Chip>>> = Memo::new(move |_| {
        state.with(|s| {
            let rows: Vec<_> = s
                .rows
                .iter()
                .filter(|r| r.subject == s.subject)
                .cloned()
                .collect();
            let tmax = s
                .ok()
                .and_then(|o| o.parameters.iter().find(|p| p.name == "tmax"))
                .and_then(|p| p.value.value);
            chips_for(&rows, s.scene.as_ref(), s.ok(), tmax)
        })
    });
    let title = move || {
        state.with(|s| {
            s.info
                .as_ref()
                .map_or_else(|| "No data".to_owned(), |i| i.name.clone())
        })
    };
    let sub = move || {
        state.with(|s| {
            s.info
                .as_ref()
                .map(|i| {
                    format!(
                        " · {} rows, subject {}",
                        fmt::number(i.rows as f64),
                        s.subject
                    )
                })
                .unwrap_or_default()
        })
    };
    view! {
        <section class="panel data-panel" aria-label="Data">
            <div class="panel-head">
                <span>{title} <span class="muted">{sub}</span></span>
            </div>
            <div class="scroll">
                <table class="data" aria-label="Time, concentration and role of each point">
                    <thead>
                        <tr>
                            <th>"Time"</th>
                            <th>"Concentration"</th>
                            <th>"Role"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <For
                            each=indexes
                            key=|i| *i
                            children=move |index| {
                                view! {
                                    <tr
                                        tabindex="0"
                                        data-row=index
                                        title="Space or Enter: take this point in or out of the terminal phase"
                                        on:keydown=move |ev| row_keys(ctx, index, ev)
                                    >
                                        <td><NumberCell ctx=ctx index=index role=CellRole::Time/></td>
                                        <td><NumberCell ctx=ctx index=index role=CellRole::Concentration/></td>
                                        <td class="chips">
                                            {move || {
                                                chips
                                                    .with(|m| m.get(&index).cloned().unwrap_or_default())
                                                    .into_iter()
                                                    .map(|c| view! { <span class=format!("chip chip-{}", c.kind.css())>{c.text}</span> })
                                                    .collect_view()
                                            }}
                                        </td>
                                    </tr>
                                }
                            }
                        />
                    </tbody>
                </table>
            </div>
            <p class="hint pad">"Edit a cell and the analysis is stale until it runs again."</p>
        </section>
    }
}

/// The keys of a row: Up and Down move between rows, Space and Enter toggle the point.
fn row_keys(ctx: Ctx, index: usize, ev: web_sys::KeyboardEvent) {
    // Only when the row itself has the focus, not an input inside it.
    let on_row = match (ev.target(), ev.current_target()) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    };
    if !on_row {
        return;
    }
    let sibling = |next: bool| {
        let row = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())?;
        let other = if next {
            row.next_element_sibling()
        } else {
            row.previous_element_sibling()
        };
        other.and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    };
    match ev.key().as_str() {
        "ArrowDown" => {
            if let Some(el) = sibling(true) {
                let _ = el.focus();
            }
        }
        "ArrowUp" => {
            if let Some(el) = sibling(false) {
                let _ = el.focus();
            }
        }
        " " | "Enter" => {
            let pick = ctx.state.with_untracked(|s| {
                s.rows
                    .get(index)
                    .filter(|r| r.conc.is_some_and(|c| c > 0.0))
                    .map(|r| r.time)
            });
            if let Some(time) = pick {
                ctx.act(|s| s.toggle_point(time));
            }
        }
        _ => return,
    }
    ev.prevent_default();
}
