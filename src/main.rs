use json_ruster::convert;
use json_ruster::graph::{build_graph, Graph};
use json_ruster::layout::{
    field_full_text, layout as compute_layout, truncate_display, wrap_text, FieldRef, NodeLayout,
    LESS_MARKER, LINE_HEIGHT, MORE_MARKER,
};
use json_ruster::model::DataNode;
use json_ruster::parsers::{self, Format};
use leptos::ev::{MouseEvent, PointerEvent, WheelEvent};
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::JsCast;

const GRAPH_SVG_ID: &str = "jr-graph-svg";

/// Triggers a browser download of `contents` as `filename` by creating an
/// object URL and clicking a throwaway `<a download>` link -- there is no
/// direct "save file" API available to a plain WASM/CSR app.
fn download_text(filename: &str, mime: &str, contents: &str) {
    let parts = js_sys::Array::new();
    parts.push(&wasm_bindgen::JsValue::from_str(contents));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };

    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Ok(element) = document.create_element("a") {
            if let Ok(anchor) = element.dyn_into::<web_sys::HtmlAnchorElement>() {
                anchor.set_href(&url);
                anchor.set_download(filename);
                anchor.click();
            }
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

fn export_svg() {
    let Some(svg) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(GRAPH_SVG_ID))
    else {
        return;
    };
    download_text("graph.svg", "image/svg+xml", &svg.outer_html());
}

#[component]
fn App() -> impl IntoView {
    let (format, set_format) = signal(Format::Json);
    let (input, set_input) = signal(Format::Json.sample().to_string());
    let (convert_target, set_convert_target) = signal(Format::Yaml);
    let (convert_error, set_convert_error) = signal(None::<String>);

    let parsed = Memo::new(move |_| parsers::parse(format.get(), &input.get()));

    let on_format_change = move |ev| {
        if let Some(new_format) = Format::from_label(&event_target_value(&ev)) {
            set_format.set(new_format);
            set_input.set(new_format.sample().to_string());
            set_convert_error.set(None);
        }
    };

    let on_convert_target_change = move |ev| {
        if let Some(new_format) = Format::from_label(&event_target_value(&ev)) {
            set_convert_target.set(new_format);
        }
    };

    let on_convert_click = move |_| {
        let target = convert_target.get_untracked();
        match parsed.get_untracked() {
            Ok(data) => match convert::convert(&data, target) {
                Ok(text) => {
                    set_format.set(target);
                    set_input.set(text);
                    set_convert_error.set(None);
                }
                Err(e) => set_convert_error.set(Some(e)),
            },
            Err(_) => set_convert_error.set(Some("Fix the parsing error before converting".to_string())),
        }
    };

    view! {
        <div style="display:flex; flex-direction:column; height:100vh; width:100vw; font-family: sans-serif;">
            <div style="display:flex; align-items:center; flex-wrap:wrap; gap:6px; padding:6px 10px; background:#1a202c; border-bottom:1px solid #2d3748;">
                <label style="color:#a0aec0; font-size:13px;">"Format"</label>
                <select on:change=on_format_change>
                    {Format::ALL
                        .iter()
                        .map(|f| view! { <option value=f.label()>{f.label()}</option> })
                        .collect::<Vec<_>>()}
                </select>

                <span style="color:#4a5568; margin:0 4px;">"|"</span>

                <label style="color:#a0aec0; font-size:13px;">"Convert to"</label>
                <select on:change=on_convert_target_change>
                    {Format::ALL
                        .iter()
                        .map(|f| view! { <option value=f.label()>{f.label()}</option> })
                        .collect::<Vec<_>>()}
                </select>
                <button on:click=on_convert_click>"Convert"</button>

                <span style="color:#4a5568; margin:0 4px;">"|"</span>
                <button on:click=move |_| export_svg()>"Export SVG"</button>

                {move || convert_error.get().map(|e| view! {
                    <span style="color:#ff6b6b; font-size:12px;">{e}</span>
                })}
            </div>
            <div style="display:flex; flex:1; min-height:0;">
                <textarea
                    style="width:40%; height:100%; box-sizing:border-box; font-family: monospace; font-size:13px; padding:1em;"
                    prop:value=move || input.get()
                    on:input=move |ev| {
                        set_input.set(event_target_value(&ev));
                        set_convert_error.set(None);
                    }
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
    let (expanded, set_expanded) = signal(HashSet::<(usize, FieldRef)>::new());

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
        // Collapsing/expanding reshapes the whole layout, so the clicked
        // node can shift far away in local layout coordinates (e.g. a root
        // centered over a wide subtree jumps back to x=0 once collapsed).
        // Compensate the pan by exactly that shift so the clicked node
        // stays under the cursor instead of the view jumping around.
        let expanded_set = expanded.get_untracked();
        let before = collapsed.get_untracked();
        let pos_before = graph.with_value(|g| compute_layout(g, &before, &expanded_set));

        set_collapsed.update(|set| {
            if !set.remove(&id) {
                set.insert(id);
            }
        });

        let after = collapsed.get_untracked();
        let pos_after = graph.with_value(|g| compute_layout(g, &after, &expanded_set));

        if let (Some(before_pos), Some(after_pos)) = (pos_before.get(&id), pos_after.get(&id)) {
            let s = scale.get_untracked();
            set_tx.update(|v| *v += s * (before_pos.x - after_pos.x));
            set_ty.update(|v| *v += s * (before_pos.y - after_pos.y));
        }
    };
    let select = move |id: usize| set_selected.set(Some(id));
    let toggle_expand = move |key: (usize, FieldRef)| {
        set_expanded.update(|set| {
            if !set.remove(&key) {
                set.insert(key);
            }
        });
    };

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
                        .unwrap_or_else(|| "Click a node to select it (click = collapse/expand)".to_string())
                }}
            </div>
            <svg id=GRAPH_SVG_ID xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
                <g style=move || format!("transform: translate({}px, {}px) scale({})", tx.get(), ty.get(), scale.get())>
                    {move || {
                        let collapsed_set = collapsed.get();
                        let expanded_set = expanded.get();
                        let sel = selected.get();
                        graph.with_value(|g| {
                            let positions = compute_layout(g, &collapsed_set, &expanded_set);
                            let edges = render_edges(g, &positions);
                            let nodes = render_nodes(
                                g,
                                &positions,
                                &collapsed_set,
                                &expanded_set,
                                sel,
                                toggle,
                                select,
                                toggle_expand,
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

/// Renders `full` starting at `start_y`, either as a single truncated line
/// with a clickable `[...]` marker, or -- when `(node_id, field)` is in
/// `expanded_set` -- wrapped in place over several lines with a trailing
/// clickable `[-]` marker to collapse it back. Returns the views and how
/// many lines they occupy, so the caller can position what comes next.
fn truncatable_lines(
    x: &'static str,
    start_y: f64,
    color: &'static str,
    weight: Option<&'static str>,
    full: String,
    key: (usize, FieldRef),
    expanded_set: &HashSet<(usize, FieldRef)>,
    toggle_expand: impl Fn((usize, FieldRef)) + Copy + 'static,
) -> (Vec<AnyView>, usize) {
    if expanded_set.contains(&key) {
        let lines = wrap_text(&full);
        let count = lines.len();
        let views = lines
            .into_iter()
            .enumerate()
            .map(|(i, line)| {
                let y = start_y + i as f64 * LINE_HEIGHT;
                let is_last = i + 1 == count;
                view! {
                    <text x=x y=y.to_string() fill=color font-size="12" font-family="monospace" font-weight=weight>
                        {line}
                        {is_last.then(|| view! {
                            <tspan
                                fill="#63b3ed"
                                style="cursor:pointer;"
                                on:click=move |ev: MouseEvent| {
                                    ev.stop_propagation();
                                    toggle_expand(key);
                                }
                            >
                                {LESS_MARKER}
                            </tspan>
                        })}
                    </text>
                }
                .into_any()
            })
            .collect::<Vec<_>>();
        (views, count)
    } else {
        let (display, truncated) = truncate_display(&full);
        let view = view! {
            <text x=x y=start_y.to_string() fill=color font-size="12" font-family="monospace" font-weight=weight>
                {display}
                {truncated.then(|| view! {
                    <tspan
                        fill="#63b3ed"
                        style="cursor:pointer;"
                        on:click=move |ev: MouseEvent| {
                            ev.stop_propagation();
                            toggle_expand(key);
                        }
                    >
                        {MORE_MARKER}
                    </tspan>
                })}
            </text>
        }
        .into_any();
        (vec![view], 1)
    }
}

fn render_nodes(
    graph: &Graph,
    positions: &HashMap<usize, NodeLayout>,
    collapsed: &HashSet<usize>,
    expanded: &HashSet<(usize, FieldRef)>,
    selected: Option<usize>,
    toggle: impl Fn(usize) + Copy + 'static,
    select: impl Fn(usize) + Copy + 'static,
    toggle_expand: impl Fn((usize, FieldRef)) + Copy + 'static,
) -> Vec<impl IntoView> {
    let mut ids: Vec<usize> = positions.keys().copied().collect();
    ids.sort_unstable();

    ids.into_iter()
        .map(|id| {
            let node = &graph.nodes[id];
            let pos = positions[&id];
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

            let (title_views, title_line_count) = truncatable_lines(
                "10",
                16.0,
                "#63b3ed",
                Some("bold"),
                node.title.clone(),
                (id, FieldRef::Title),
                expanded,
                toggle_expand,
            );

            let mut y_cursor = 30.0 + (title_line_count as f64 - 1.0) * LINE_HEIGHT;
            let mut field_views = Vec::new();
            for (i, (k, v)) in node.fields.iter().enumerate() {
                let full = field_full_text(k, v);
                let (views, count) = truncatable_lines(
                    "10",
                    y_cursor,
                    "#e2e8f0",
                    None,
                    full,
                    (id, FieldRef::Field(i)),
                    expanded,
                    toggle_expand,
                );
                field_views.extend(views);
                y_cursor += count as f64 * LINE_HEIGHT;
            }

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
                    {title_views}
                    {(!marker.is_empty()).then(|| view! {
                        <text x=marker_x y="16" fill="#a0aec0" font-size="11" font-family="monospace">{marker}</text>
                    })}
                    {field_views}
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
