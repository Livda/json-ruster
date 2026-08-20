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
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

/// Clicks a throwaway `<a download>` link -- there is no direct "save file"
/// API available to a plain WASM/CSR app.
fn trigger_download(filename: &str, url: &str) {
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Ok(element) = document.create_element("a") {
            if let Ok(anchor) = element.dyn_into::<web_sys::HtmlAnchorElement>() {
                anchor.set_href(url);
                anchor.set_download(filename);
                anchor.click();
            }
        }
    }
}

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
    trigger_download(filename, &url);
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Draws `svg_markup` (a standalone `<svg>...</svg>` document, with explicit
/// pixel `width`/`height`) into an offscreen canvas and downloads the result
/// as a PNG. Loading the SVG into an `<img>` is inherently async, so the
/// download only happens once `onload` fires; the closure is leaked via
/// `forget()` since it only ever needs to run once.
fn export_png(svg_markup: &str, width: f64, height: f64) {
    let Ok(img) = web_sys::HtmlImageElement::new() else {
        return;
    };
    let encoded = js_sys::encode_uri_component(svg_markup);
    img.set_src(&format!("data:image/svg+xml;charset=utf-8,{encoded}"));

    let img_for_draw = img.clone();
    let onload = Closure::once(move || {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Ok(canvas) = document
            .create_element("canvas")
            .and_then(|c| c.dyn_into::<web_sys::HtmlCanvasElement>().map_err(Into::into))
        else {
            return;
        };
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        let Ok(Some(ctx)) = canvas.get_context("2d") else {
            return;
        };
        let Ok(ctx) = ctx.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
            return;
        };
        if ctx.draw_image_with_html_image_element(&img_for_draw, 0.0, 0.0).is_ok() {
            if let Ok(png_url) = canvas.to_data_url_with_type("image/png") {
                trigger_download("graph.png", &png_url);
            }
        }
    });
    img.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
}

fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Renders the graph as a standalone SVG document, independent of the
/// interactive view's current pan/zoom (so the export always shows the
/// full graph, not whatever happens to be scrolled into view) and without
/// the clickable expand/collapse affordances (a static export has nothing
/// to click). Truncation/wrapping still mirrors `render_nodes` so the text
/// fits the same box sizes computed by `layout::layout`.
fn render_static_svg(
    graph: &Graph,
    positions: &HashMap<usize, NodeLayout>,
    expanded: &HashSet<(usize, FieldRef)>,
) -> (String, f64, f64) {
    let width = positions.values().map(|p| p.x + p.width).fold(0.0_f64, f64::max) + 40.0;
    let height = positions.values().map(|p| p.y + p.height).fold(0.0_f64, f64::max) + 40.0;

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"{width}\" height=\"{height}\" fill=\"#0f1117\" />\n\
         <g transform=\"translate(20, 20)\">\n"
    );

    for (&id, from) in positions {
        for &child_id in &graph.nodes[id].children {
            if let Some(to) = positions.get(&child_id) {
                let x1 = from.x + from.width / 2.0;
                let y1 = from.y + from.height;
                let x2 = to.x + to.width / 2.0;
                let y2 = to.y;
                let mid_y = (y1 + y2) / 2.0;
                svg.push_str(&format!(
                    "<path d=\"M {x1} {y1} C {x1} {mid_y}, {x2} {mid_y}, {x2} {y2}\" fill=\"none\" stroke=\"#4a5568\" stroke-width=\"1.5\" />\n"
                ));
            }
        }
    }

    let mut ids: Vec<usize> = positions.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let node = &graph.nodes[id];
        let pos = positions[&id];
        svg.push_str(&format!(
            "<g transform=\"translate({}, {})\">\n<rect width=\"{}\" height=\"{}\" rx=\"6\" fill=\"#1a202c\" stroke=\"#4a5568\" stroke-width=\"1.5\" />\n",
            pos.x, pos.y, pos.width, pos.height
        ));

        let title_lines = if expanded.contains(&(id, FieldRef::Title)) {
            wrap_text(&node.title)
        } else {
            vec![truncate_display(&node.title).0]
        };
        for (i, line) in title_lines.iter().enumerate() {
            let y = 16.0 + i as f64 * LINE_HEIGHT;
            svg.push_str(&format!(
                "<text x=\"10\" y=\"{y}\" fill=\"#63b3ed\" font-size=\"12\" font-family=\"monospace\" font-weight=\"bold\">{}</text>\n",
                escape_xml_text(line)
            ));
        }

        let mut y_cursor = 30.0 + (title_lines.len() as f64 - 1.0) * LINE_HEIGHT;
        for (i, (k, v)) in node.fields.iter().enumerate() {
            let full = field_full_text(k, v);
            let lines = if expanded.contains(&(id, FieldRef::Field(i))) {
                wrap_text(&full)
            } else {
                vec![truncate_display(&full).0]
            };
            for line in &lines {
                svg.push_str(&format!(
                    "<text x=\"10\" y=\"{y_cursor}\" fill=\"#e2e8f0\" font-size=\"12\" font-family=\"monospace\">{}</text>\n",
                    escape_xml_text(line)
                ));
                y_cursor += LINE_HEIGHT;
            }
        }

        svg.push_str("</g>\n");
    }

    svg.push_str("</g>\n</svg>\n");
    (svg, width, height)
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

    // window_event_listener does not auto-cleanup: without on_cleanup, this
    // closure keeps running against this GraphView's signals after it has
    // been unmounted (e.g. a new GraphView is mounted after a format
    // change), which panics on a disposed signal the next time the mouse
    // moves anywhere on the page.
    let pointermove_handle = window_event_listener(leptos::ev::pointermove, move |ev: PointerEvent| {
        if is_dragging.get_untracked() {
            let (sx, sy, start_tx, start_ty) = drag_start.get_value();
            set_tx.set(start_tx + (ev.client_x() as f64 - sx));
            set_ty.set(start_ty + (ev.client_y() as f64 - sy));
        }
    });
    let pointerup_handle = window_event_listener(leptos::ev::pointerup, move |_: PointerEvent| {
        set_is_dragging.set(false);
    });
    on_cleanup(move || {
        pointermove_handle.remove();
        pointerup_handle.remove();
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

    let render_export_svg = move || {
        let collapsed_set = collapsed.get_untracked();
        let expanded_set = expanded.get_untracked();
        graph.with_value(|g| {
            let positions = compute_layout(g, &collapsed_set, &expanded_set);
            render_static_svg(g, &positions, &expanded_set)
        })
    };
    let on_export_svg = move |_: MouseEvent| {
        let (markup, _, _) = render_export_svg();
        download_text("graph.svg", "image/svg+xml", &markup);
    };
    let on_export_png = move |_: MouseEvent| {
        let (markup, width, height) = render_export_svg();
        export_png(&markup, width, height);
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
            <div style="position:absolute; top:0; left:0; right:0; display:flex; justify-content:space-between; align-items:center; padding:6px 10px; font-family:monospace; font-size:12px; color:#a0aec0; background:rgba(15,17,23,0.85); z-index:1;">
                <span style="pointer-events:none;">
                    {move || {
                        selected.get()
                            .map(|id| graph.with_value(|g| g.path_to(id)))
                            .unwrap_or_else(|| "Click a node to select it (click = collapse/expand)".to_string())
                    }}
                </span>
                <span>
                    <button on:click=on_export_svg>"Export SVG"</button>
                    <button on:click=on_export_png>"Export PNG"</button>
                </span>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
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
