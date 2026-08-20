use json_ruster::graph::{build_graph, Graph};
use json_ruster::layout::{
    field_text, layout as compute_layout, truncate_display, NodeLayout, MORE_MARKER,
};
use json_ruster::model::DataNode;
use json_ruster::parsers::{self, Format};
use leptos::ev::{MouseEvent, PointerEvent, WheelEvent};
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

#[component]
fn App() -> impl IntoView {
    let (format, set_format) = signal(Format::Json);
    let (input, set_input) = signal(Format::Json.sample().to_string());

    let parsed = Memo::new(move |_| parsers::parse(format.get(), &input.get()));

    let on_format_change = move |ev| {
        if let Some(new_format) = Format::from_label(&event_target_value(&ev)) {
            set_format.set(new_format);
            set_input.set(new_format.sample().to_string());
        }
    };

    view! {
        <div style="display:flex; flex-direction:column; height:100vh; width:100vw; font-family: sans-serif;">
            <div style="padding:6px 10px; background:#1a202c; border-bottom:1px solid #2d3748;">
                <label style="color:#a0aec0; font-size:13px; margin-right:6px;">"Format"</label>
                <select on:change=on_format_change>
                    {Format::ALL
                        .iter()
                        .map(|f| view! { <option value=f.label()>{f.label()}</option> })
                        .collect::<Vec<_>>()}
                </select>
            </div>
            <div style="display:flex; flex:1; min-height:0;">
                <textarea
                    style="width:40%; height:100%; box-sizing:border-box; font-family: monospace; font-size:13px; padding:1em;"
                    prop:value=move || input.get()
                    on:input=move |ev| set_input.set(event_target_value(&ev))
                />
                <div style="width:60%; height:100%; overflow:hidden; background:#0f1117;">
                    {move || match parsed.get() {
                        Ok(data) => view! { <GraphView data=data /> }.into_any(),
                        Err(e) => view! {
                            <p style="color:#ff6b6b; padding:1em; font-family: monospace; white-space:pre-wrap;">{e}</p>
                        }.into_any(),
                    }}
                </div>
            </div>
        </div>
    }
}

#[component]
fn GraphView(data: DataNode) -> impl IntoView {
    let graph = StoredValue::new(build_graph(&data));

    let (collapsed, set_collapsed) = signal(HashSet::<usize>::new());
    let (selected, set_selected) = signal(None::<usize>);
    let (inspect, set_inspect) = signal(None::<String>);

    let (scale, set_scale) = signal(1.0_f64);
    let (tx, set_tx) = signal(0.0_f64);
    let (ty, set_ty) = signal(0.0_f64);
    let (is_dragging, set_is_dragging) = signal(false);
    let drag_start = StoredValue::new((0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64));

    let on_pointer_down = move |ev: PointerEvent| {
        set_is_dragging.set(true);
        drag_start.set_value((
            ev.client_x() as f64,
            ev.client_y() as f64,
            tx.get_untracked(),
            ty.get_untracked(),
        ));
    };

    window_event_listener(leptos::ev::pointermove, move |ev: PointerEvent| {
        if is_dragging.get_untracked() {
            let (sx, sy, start_tx, start_ty) = drag_start.get_value();
            set_tx.set(start_tx + (ev.client_x() as f64 - sx));
            set_ty.set(start_ty + (ev.client_y() as f64 - sy));
        }
    });
    window_event_listener(leptos::ev::pointerup, move |_: PointerEvent| {
        set_is_dragging.set(false);
    });

    let on_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        let factor = (-ev.delta_y() * 0.001).exp();
        set_scale.update(|s| *s = (*s * factor).clamp(0.2, 3.0));
    };

    let toggle = move |id: usize| {
        set_collapsed.update(|set| {
            if !set.remove(&id) {
                set.insert(id);
            }
        });
    };
    let select = move |id: usize| set_selected.set(Some(id));

    view! {
        <div
            style=move || format!(
                "width:100%; height:100%; position:relative; cursor:{};",
                if is_dragging.get() { "grabbing" } else { "grab" }
            )
            on:pointerdown=on_pointer_down
            on:wheel=on_wheel
        >
            <div style="position:absolute; top:0; left:0; right:0; padding:6px 10px; font-family:monospace; font-size:12px; color:#a0aec0; background:rgba(15,17,23,0.85); z-index:1; pointer-events:none;">
                {move || {
                    selected.get()
                        .map(|id| graph.with_value(|g| g.path_to(id)))
                        .unwrap_or_else(|| "Cliquez un noeud pour le selectionner (clic = plier/deplier)".to_string())
                }}
            </div>
            <svg width="100%" height="100%">
                <g style=move || format!("transform: translate({}px, {}px) scale({})", tx.get(), ty.get(), scale.get())>
                    {move || {
                        let collapsed_set = collapsed.get();
                        let sel = selected.get();
                        graph.with_value(|g| {
                            let positions = compute_layout(g, &collapsed_set);
                            let edges = render_edges(g, &positions);
                            let nodes = render_nodes(
                                g,
                                &positions,
                                &collapsed_set,
                                sel,
                                toggle,
                                select,
                                set_inspect,
                            );
                            view! {
                                <g transform="translate(20, 20)">
                                    {edges}
                                    {nodes}
                                </g>
                            }
                        })
                    }}
                </g>
            </svg>
            {move || inspect.get().map(|text| view! {
                <div style="position:absolute; bottom:0; left:0; right:0; max-height:45%; overflow:auto; background:rgba(15,17,23,0.97); border-top:1px solid #4a5568; padding:10px 36px 10px 10px; font-family:monospace; font-size:12px; color:#e2e8f0; white-space:pre-wrap; word-break:break-word; z-index:2;">
                    <button
                        on:click=move |ev: MouseEvent| { ev.stop_propagation(); set_inspect.set(None); }
                        style="position:absolute; top:6px; right:8px; background:none; border:none; color:#a0aec0; font-size:16px; cursor:pointer; line-height:1;"
                    >"×"</button>
                    {text}
                </div>
            })}
        </div>
    }
}

fn render_edges(graph: &Graph, positions: &HashMap<usize, NodeLayout>) -> Vec<impl IntoView> {
    positions
        .keys()
        .flat_map(|&id| {
            let from = positions[&id];
            graph.nodes[id].children.iter().filter_map(move |&child_id| {
                positions.get(&child_id).map(|&to| {
                    let x1 = from.x + from.width / 2.0;
                    let y1 = from.y + from.height;
                    let x2 = to.x + to.width / 2.0;
                    let y2 = to.y;
                    let mid_y = (y1 + y2) / 2.0;
                    let d = format!("M {x1} {y1} C {x1} {mid_y}, {x2} {mid_y}, {x2} {y2}");
                    view! { <path d=d fill="none" stroke="#4a5568" stroke-width="1.5" /> }
                })
            })
        })
        .collect::<Vec<_>>()
}

/// A text line with an optional trailing clickable `[...]` marker that
/// reveals the untruncated text in the inspect panel.
fn truncatable_line(
    x: &'static str,
    y: String,
    color: &'static str,
    weight: Option<&'static str>,
    full: String,
    display: String,
    truncated: bool,
    set_inspect: WriteSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <text x=x y=y fill=color font-size="12" font-family="monospace" font-weight=weight>
            {display}
            {truncated.then(|| {
                let full = full.clone();
                view! {
                    <tspan
                        fill="#63b3ed"
                        style="cursor:pointer; text-decoration:underline;"
                        on:click=move |ev: MouseEvent| {
                            ev.stop_propagation();
                            set_inspect.set(Some(full.clone()));
                        }
                    >
                        {MORE_MARKER}
                    </tspan>
                }
            })}
        </text>
    }
}

fn render_nodes(
    graph: &Graph,
    positions: &HashMap<usize, NodeLayout>,
    collapsed: &HashSet<usize>,
    selected: Option<usize>,
    toggle: impl Fn(usize) + Copy + 'static,
    select: impl Fn(usize) + Copy + 'static,
    set_inspect: WriteSignal<Option<String>>,
) -> Vec<impl IntoView> {
    let mut ids: Vec<usize> = positions.keys().copied().collect();
    ids.sort_unstable();

    ids.into_iter()
        .map(|id| {
            let node = &graph.nodes[id];
            let pos = positions[&id];
            let title_full = node.title.clone();
            let (title_display, title_truncated) = truncate_display(&node.title);
            let has_children = !node.children.is_empty();
            let is_collapsed = collapsed.contains(&id);
            let is_selected = selected == Some(id);

            let marker = if !has_children {
                String::new()
            } else if is_collapsed {
                format!("+{}", node.children.len())
            } else {
                "-".to_string()
            };

            let field_lines: Vec<_> = node
                .fields
                .iter()
                .enumerate()
                .map(|(i, (k, v))| {
                    let full = if k.is_empty() {
                        v.clone()
                    } else {
                        format!("{k}: {v}")
                    };
                    let (display, truncated) = field_text(k, v);
                    let y = 30.0 + i as f64 * 20.0;
                    truncatable_line("10", y.to_string(), "#e2e8f0", None, full, display, truncated, set_inspect)
                })
                .collect();

            let stroke = if is_selected { "#f6ad55" } else { "#4a5568" };
            let cursor = if has_children { "pointer" } else { "default" };
            let marker_x = pos.width - (marker.len() as f64 * 7.0) - 10.0;

            view! {
                <g
                    transform=format!("translate({}, {})", pos.x, pos.y)
                    style=format!("cursor:{cursor}")
                    on:click=move |ev: MouseEvent| {
                        ev.stop_propagation();
                        select(id);
                        if has_children {
                            toggle(id);
                        }
                    }
                >
                    <rect
                        width=pos.width
                        height=pos.height
                        rx="6"
                        fill="#1a202c"
                        stroke=stroke
                        stroke-width="1.5"
                    />
                    {truncatable_line(
                        "10",
                        "16".to_string(),
                        "#63b3ed",
                        Some("bold"),
                        title_full,
                        title_display,
                        title_truncated,
                        set_inspect,
                    )}
                    {(!marker.is_empty()).then(|| view! {
                        <text x=marker_x y="16" fill="#a0aec0" font-size="11" font-family="monospace">{marker}</text>
                    })}
                    {field_lines}
                </g>
            }
        })
        .collect::<Vec<_>>()
}

fn main() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);
    leptos::mount::mount_to_body(App);
}
