//! The plot, drawn as SVG from `plot.scene`, the two handles of the terminal phase, and the list of
//! candidate ranges. The scene holds a log axis already in log10: `plotmap` applies one affine map.
//!
//! The handles are two elements that stay in the document (their position is a reactive attribute), so
//! a pointer capture or a keyboard focus survives the redraw that every drag step causes.

use caladrius_format as fmt;
use leptos::html;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::app::Ctx;
use crate::dom;
use crate::plotmap::{
    Frame, coord, pick_point, plot_rect, polyline, px_of_x, py_of_y, snap_sample, step_sample,
};
use crate::results::{adj_text, count, number};
use crate::types::{Role, ScenePanel, SceneSeries, Style};

/// left, right, top, bottom
const MARGIN: (f64, f64, f64, f64) = (64.0, 18.0, 16.0, 44.0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Start,
    End,
}

impl Which {
    fn name(self) -> &'static str {
        match self {
            Which::Start => "start",
            Which::End => "end",
        }
    }
}

fn frame_of(size: (f64, f64)) -> Frame {
    Frame {
        width: size.0,
        height: size.1,
        left: MARGIN.0,
        right: MARGIN.1,
        top: MARGIN.2,
        bottom: MARGIN.3,
    }
}

const ORDER: [Role; 9] = [
    Role::Predicted,
    Role::Profile,
    Role::LambdaZFit,
    Role::Observed,
    Role::Subject,
    Role::Excluded,
    Role::Replaced,
    Role::LambdaZUsed,
    Role::LambdaZStart,
];

fn order_of(role: Role) -> usize {
    ORDER.iter().position(|r| *r == role).unwrap_or(ORDER.len())
}

fn markers(frame: &Frame, panel: &ScenePanel, series: &SceneSeries, radius: f64) -> impl IntoView {
    let class = format!("pts pts-{}", series.role.css());
    let circles = series
        .points
        .iter()
        .map(|p| {
            view! {
                <circle
                    cx=coord(px_of_x(frame, panel, p.x))
                    cy=coord(py_of_y(frame, panel, p.y))
                    r=radius
                    class=if p.blq { "blq" } else { "" }
                ></circle>
            }
        })
        .collect_view();
    view! { <g class=class>{circles}</g> }
}

/// The parts of the plot that depend only on the scene and the size: axes, grid, lines, markers.
fn drawing(frame: Frame, panel: &ScenePanel, marker: f64) -> impl IntoView {
    let rect = plot_rect(&frame);
    let xs = panel
        .x
        .ticks
        .iter()
        .map(|t| {
            let x = coord(px_of_x(&frame, panel, t.value));
            view! {
                <g>
                    <line class="grid" x1=x.clone() x2=x.clone() y1=rect.y0 y2=rect.y1></line>
                    <text class="tick" x=x y=rect.y1 + 16.0 text-anchor="middle">{t.label.clone()}</text>
                </g>
            }
        })
        .collect_view();
    let ys = panel
        .y
        .ticks
        .iter()
        .map(|t| {
            let y = py_of_y(&frame, panel, t.value);
            view! {
                <g>
                    <line class="grid" y1=coord(y) y2=coord(y) x1=rect.x0 x2=rect.x1></line>
                    <text class="tick" y=coord(y + 4.0) x=rect.x0 - 6.0 text-anchor="end">{t.label.clone()}</text>
                </g>
            }
        })
        .collect_view();
    let mut ordered: Vec<&SceneSeries> = panel.series.iter().collect();
    ordered.sort_by_key(|s| order_of(s.role));
    let series = ordered
        .into_iter()
        .map(|s| match s.style {
            Style::Line => view! {
                <polyline class=format!("line line-{}", s.role.css()) points=polyline(&frame, panel, s) fill="none"></polyline>
            }
            .into_any(),
            _ => {
                let radius = if matches!(s.role, Role::LambdaZUsed | Role::LambdaZStart) { marker + 1.5 } else { marker };
                markers(&frame, panel, s, radius).into_any()
            }
        })
        .collect_view();
    view! {
        <rect class="plot-bg" x=rect.x0 y=rect.y0 width=rect.x1 - rect.x0 height=rect.y1 - rect.y0></rect>
        {xs}
        {ys}
        <text class="axis-title" x=(rect.x0 + rect.x1) / 2.0 y=frame.height - 6.0 text-anchor="middle">{panel.x.title.clone()}</text>
        <text
            class="axis-title"
            transform=format!("translate(14 {}) rotate(-90)", (rect.y0 + rect.y1) / 2.0)
            text-anchor="middle"
        >
            {panel.y.title.clone()}
        </text>
        {series}
    }
}

#[component]
pub fn PlotPanel(ctx: Ctx) -> impl IntoView {
    let state = ctx.state;
    let boxed = NodeRef::<html::Div>::new();
    let size = RwSignal::new((640.0_f64, 360.0_f64));
    let drag: RwSignal<Option<(Which, f64)>> = RwSignal::new(None);

    // Follow the size of the box (a ResizeObserver; the closure lives as long as the page).
    Effect::new(move |_| {
        let Some(el) = boxed.get() else { return };
        let read = {
            let el = el.clone();
            move || {
                size.set((
                    f64::from(el.client_width()).max(200.0),
                    f64::from(el.client_height()).max(160.0),
                ))
            }
        };
        read();
        let callback = Closure::<dyn FnMut()>::new(read);
        if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            observer.observe(&el);
        }
        callback.forget();
    });

    let frame = move || frame_of(size.get());
    let hit = move || dom::token("--size-hit-radius", 12.0);
    let marker = move || dom::token("--size-marker-medium", 5.0);
    let local = move |client_x: f64, client_y: f64| -> (f64, f64) {
        boxed.get_untracked().map_or((client_x, client_y), |el| {
            let r = el.get_bounding_client_rect();
            (client_x - r.left(), client_y - r.top())
        })
    };

    // The time of a handle: the dragged one while dragging, else the end of the range in the state.
    let time_of = move |which: Which| -> Option<f64> {
        if let Some((w, t)) = drag.get()
            && w == which
        {
            return Some(t);
        }
        state
            .with(|s| s.range())
            .map(|(a, b)| if which == Which::Start { a } else { b })
    };
    let x_of = move |which: Which| -> Option<f64> {
        let t = time_of(which)?;
        let f = frame();
        state.with(|s| {
            s.scene
                .as_ref()
                .and_then(|sc| sc.panels.first())
                .map(|p| px_of_x(&f, p, t))
        })
    };

    let down = move |which: Which, ev: web_sys::PointerEvent| {
        let Some((a, b)) = state.with_untracked(|s| s.range()) else {
            return;
        };
        if let Some(el) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
        drag.set(Some((which, if which == Which::Start { a } else { b })));
        ev.stop_propagation();
    };
    let moved = move |ev: web_sys::PointerEvent| {
        let Some((which, time)) = drag.get_untracked() else {
            return;
        };
        let (x, _) = local(f64::from(ev.client_x()), f64::from(ev.client_y()));
        let f = frame_of(size.get_untracked());
        let snapped = state.with_untracked(|s| {
            s.scene
                .as_ref()
                .and_then(|sc| sc.panels.first())
                .and_then(|p| snap_sample(&f, p, x))
        });
        if let Some(p) = snapped
            && p.x != time
        {
            drag.set(Some((which, p.x)));
        }
    };
    let up = move |ev: web_sys::PointerEvent| {
        let Some((which, time)) = drag.get_untracked() else {
            return;
        };
        drag.set(None);
        if let Some(el) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        {
            let _ = el.release_pointer_capture(ev.pointer_id());
        }
        let current = state
            .with_untracked(|s| s.range())
            .map(|(a, b)| if which == Which::Start { a } else { b });
        if current != Some(time) {
            match which {
                Which::Start => ctx.act(|s| s.set_range_start(time)),
                Which::End => ctx.act(|s| s.set_range_end(time)),
            }
        }
    };
    let key = move |which: Which, ev: web_sys::KeyboardEvent| {
        let step = match ev.key().as_str() {
            "ArrowLeft" | "ArrowDown" => -1,
            "ArrowRight" | "ArrowUp" => 1,
            _ => return,
        };
        ev.prevent_default();
        let Some(from) = state
            .with_untracked(|s| s.range())
            .map(|(a, b)| if which == Which::Start { a } else { b })
        else {
            return;
        };
        let next = state.with_untracked(|s| {
            s.scene
                .as_ref()
                .and_then(|sc| sc.panels.first())
                .and_then(|p| step_sample(p, from, step))
        });
        if let Some(next) = next
            && next != from
        {
            match which {
                Which::Start => ctx.act(|s| s.set_range_start(next)),
                Which::End => ctx.act(|s| s.set_range_end(next)),
            }
        }
    };
    let click_plot = move |ev: web_sys::MouseEvent| {
        let (x, y) = local(f64::from(ev.client_x()), f64::from(ev.client_y()));
        let radius = hit();
        let f = frame_of(size.get_untracked());
        let picked = state.with_untracked(|s| {
            s.scene
                .as_ref()
                .and_then(|sc| sc.panels.first())
                .and_then(|p| pick_point(&f, p, x, y, radius))
        });
        if let Some(p) = picked {
            ctx.act(|s| s.toggle_point(p.x));
        }
    };

    let handle = move |which: Which| {
        let label = if which == Which::Start {
            "Start of the terminal phase"
        } else {
            "End of the terminal phase"
        };
        let shown = move || x_of(which).is_some();
        let x = move || x_of(which).unwrap_or(0.0);
        view! {
            <g
                class=format!("handle handle-{}", which.name())
                tabindex="0"
                role="slider"
                aria-label=label
                aria-valuenow=move || time_of(which).map(|t| t.to_string()).unwrap_or_default()
                aria-valuetext=move || time_of(which).map(|t| format!("t = {}", fmt::number(t))).unwrap_or_default()
                display=move || if shown() { "inline" } else { "none" }
                on:pointerdown=move |ev| down(which, ev)
                on:pointermove=moved
                on:pointerup=up
                on:keydown=move |ev| key(which, ev)
                on:click=|ev| ev.stop_propagation()
            >
                <line x1=move || coord(x()) x2=move || coord(x()) y1=move || plot_rect(&frame()).y0 y2=move || plot_rect(&frame()).y1></line>
                <rect x=move || coord(x() - 6.0) y=move || coord(plot_rect(&frame()).y0) width="12" height="22" rx="3"></rect>
            </g>
        }
    };

    let panel_title = move || {
        state.with(|s| {
            s.scene
                .as_ref()
                .and_then(|sc| sc.panels.first())
                .map(|p| p.title.clone())
                .unwrap_or_default()
        })
    };
    let band = move || {
        let (a, b) = (x_of(Which::Start)?, x_of(Which::End)?);
        let r = plot_rect(&frame());
        Some((a, (b - a).max(0.0), r.y0, r.y1 - r.y0))
    };
    let log = move || state.with(|s| s.log);
    let is_manual = move || state.with(|s| s.is_manual());
    let has_panel =
        move || state.with(|s| s.scene.as_ref().is_some_and(|sc| !sc.panels.is_empty()));
    let empty = move || {
        state.with(|s| {
            s.error.clone().unwrap_or_else(|| {
                if s.ready {
                    "No plot for this subject.".to_owned()
                } else {
                    "Loading the engine...".to_owned()
                }
            })
        })
    };

    view! {
        <section class="panel plot-panel" aria-label="Plot">
            <div class="panel-head">
                <span>{move || state.with(|s| s.scene.as_ref().map_or_else(|| "Plot".to_owned(), |sc| sc.title.clone()))}</span>
                <span class="spacer"></span>
                <div role="group" aria-label="Axis" class="seg" aria-keyshortcuts="l">
                    <button type="button" aria-pressed=move || log().to_string() on:click=move |_| ctx.act(|s| s.set_log(true))>
                        "Semi-log"
                    </button>
                    <button type="button" aria-pressed=move || (!log()).to_string() on:click=move |_| ctx.act(|s| s.set_log(false))>
                        "Linear"
                    </button>
                </div>
                <button type="button" aria-keyshortcuts="b" disabled=move || !is_manual() on:click=move |_| ctx.act(|s| s.back_to_automatic())>
                    "Back to automatic"
                </button>
            </div>
            <div class="plot-box" node_ref=boxed>
                <svg
                    width=move || size.get().0
                    height=move || size.get().1
                    viewBox=move || format!("0 0 {} {}", size.get().0, size.get().1)
                    role="img"
                    aria-label=move || format!("{}. Click a point to take it in or out of the terminal phase; drag the two handles to set the range.", panel_title())
                    display=move || if has_panel() { "block" } else { "none" }
                    on:click=click_plot
                >
                    {move || {
                        let f = frame();
                        let m = marker();
                        state.with(|s| s.scene.as_ref().and_then(|sc| sc.panels.first()).map(|p| drawing(f, p, m).into_any()))
                    }}
                    {move || band().map(|(x, w, y, h)| view! { <rect class="range-band" x=coord(x) y=y width=coord(w) height=h></rect> })}
                    {handle(Which::Start)}
                    {handle(Which::End)}
                </svg>
                <Show when=move || !has_panel()>
                    <p class="empty">{empty}</p>
                </Show>
            </div>
            <div class="legend" aria-label="Legend">
                {move || {
                    state.with(|s| {
                        s.scene.as_ref().map(|sc| {
                            sc.legend
                                .entries
                                .iter()
                                .map(|e| view! { <span class=format!("key key-{}", e.role.css())><i></i>" "{e.name.clone()}</span> })
                                .collect_view()
                        })
                    })
                }}
                {move || state.with(|s| s.scene.as_ref().and_then(|sc| sc.legend.below_limit.clone())).map(|t| view! { <span class="muted">{t}</span> })}
            </div>
            {move || {
                state
                    .with(|s| s.scene.as_ref().and_then(|sc| sc.panels.first()).and_then(|p| p.hidden.as_ref()).filter(|h| h.count > 0).map(|h| h.note.clone()))
                    .map(|n| view! { <p class="hint pad" data-testid="hidden-note">{n}</p> })
            }}
            {move || {
                state
                    .with(|s| s.scene.as_ref().map(|sc| sc.notes.iter().map(|n| fmt::with_labels(n)).collect::<Vec<_>>()).unwrap_or_default())
                    .into_iter()
                    .map(|n| view! { <p class="hint pad">{n}</p> })
                    .collect_view()
            }}
            <div class="candidates">
                <table aria-label="Candidate ranges for the terminal phase">
                    <thead>
                        <tr>
                            <th>"Points"</th>
                            <th>"From"</th>
                            <th>"To"</th>
                            <th>"λz"</th>
                            <th>"Adj. R²"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            state
                                .with(|s| s.ok().map(|o| o.lambda_z_candidates.clone()).unwrap_or_default())
                                .into_iter()
                                .enumerate()
                                .map(|(i, c)| {
                                    let pick = c.clone();
                                    let pick_key = c.clone();
                                    view! {
                                        <tr
                                            tabindex="0"
                                            class=if c.selected { "selected" } else { "" }
                                            aria-selected=c.selected.to_string()
                                            on:click=move |_| ctx.act(|s| s.pick_candidate(&pick))
                                            on:keydown=move |ev| {
                                                if ev.key() == "Enter" || ev.key() == " " {
                                                    ev.prevent_default();
                                                    ctx.act(|s| s.pick_candidate(&pick_key));
                                                }
                                            }
                                        >
                                            <td data-cell=format!("c{i}:n")>{count(c.n_points)}</td>
                                            <td data-cell=format!("c{i}:from")>{number(c.time_first)}</td>
                                            <td data-cell=format!("c{i}:to")>{number(c.time_last)}</td>
                                            <td data-cell=format!("c{i}:lambda_z")>{number(c.lambda_z)}</td>
                                            <td data-cell=format!("c{i}:adj")>{adj_text(&c)}</td>
                                        </tr>
                                    }
                                })
                                .collect_view()
                        }}
                    </tbody>
                </table>
                {move || state.with(|s| s.ok().is_some() && s.selected_candidate().is_none()).then(|| view! { <p class="hint pad">"No candidate is selected."</p> })}
            </div>
        </section>
    }
}
