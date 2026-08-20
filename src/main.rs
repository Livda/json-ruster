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

#[derive(Debug, Clone, Copy, PartialEq)]
struct Theme {
    page_bg: &'static str,
    toolbar_bg: &'static str,
    toolbar_border: &'static str,
    toolbar_text: &'static str,
    graph_bg: &'static str,
    node_bg: &'static str,
    node_border: &'static str,
    node_border_selected: &'static str,
    node_border_match: &'static str,
    edge_color: &'static str,
    title_color: &'static str,
    text_color: &'static str,
    error_color: &'static str,
}

impl Theme {
    fn dark() -> Self {
        Theme {
            page_bg: "#0f1117",
            toolbar_bg: "#1a202c",
            toolbar_border: "#2d3748",
            toolbar_text: "#a0aec0",
            graph_bg: "#0f1117",
            node_bg: "#1a202c",
            node_border: "#4a5568",
            node_border_selected: "#f6ad55",
            node_border_match: "#ecc94b",
            edge_color: "#4a5568",
            title_color: "#63b3ed",
            text_color: "#e2e8f0",
            error_color: "#ff6b6b",
        }
    }

    fn light() -> Self {
        Theme {
            page_bg: "#ffffff",
            toolbar_bg: "#f1f5f9",
            toolbar_border: "#cbd5e0",
            toolbar_text: "#4a5568",
            graph_bg: "#f8fafc",
            node_bg: "#ffffff",
            node_border: "#94a3b8",
            node_border_selected: "#dd6b20",
            node_border_match: "#b7791f",
            edge_color: "#94a3b8",
            title_color: "#2b6cb0",
            text_color: "#1a202c",
            error_color: "#c53030",
        }
    }
}

/// Inline style shared by `<select>`/`<button>`/`<input>` toolbar controls,
/// which otherwise keep the browser's default white background regardless
/// of theme.
fn control_style(theme: Theme) -> String {
    format!(
        "background:{}; color:{}; border:1px solid {}; border-radius:4px; padding:2px 6px; font-size:13px;",
        theme.node_bg, theme.text_color, theme.toolbar_border
    )
}

/// Nodes whose title or a field's key/value contains `query`
/// (case-insensitive). Searched across the whole graph regardless of
/// collapse state, since a match hidden under a collapsed ancestor should
/// still surface (and that ancestor gets auto-expanded, see `GraphView`).
fn find_matches(graph: &Graph, query: &str) -> HashSet<usize> {
    if query.is_empty() {
        return HashSet::new();
    }
    graph
        .nodes
        .iter()
        .filter(|n| {
            n.title.to_lowercase().contains(query)
                || n.fields
                    .iter()
                    .any(|(k, v)| k.to_lowercase().contains(query) || v.to_lowercase().contains(query))
        })
        .map(|n| n.id)
        .collect()
}

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
    theme: Theme,
) -> (String, f64, f64) {
    let width = positions.values().map(|p| p.x + p.width).fold(0.0_f64, f64::max) + 40.0;
    let height = positions.values().map(|p| p.y + p.height).fold(0.0_f64, f64::max) + 40.0;

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"{width}\" height=\"{height}\" fill=\"{}\" />\n\
         <g transform=\"translate(20, 20)\">\n",
        theme.graph_bg
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
                    "<path d=\"M {x1} {y1} C {x1} {mid_y}, {x2} {mid_y}, {x2} {y2}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\" />\n",
                    theme.edge_color
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
            "<g transform=\"translate({}, {})\">\n<rect width=\"{}\" height=\"{}\" rx=\"6\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\" />\n",
            pos.x, pos.y, pos.width, pos.height, theme.node_bg, theme.node_border
        ));

        let title_lines = if expanded.contains(&(id, FieldRef::Title)) {
            wrap_text(&node.title)
        } else {
            vec![truncate_display(&node.title).0]
        };
        for (i, line) in title_lines.iter().enumerate() {
            let y = 16.0 + i as f64 * LINE_HEIGHT;
            svg.push_str(&format!(
                "<text x=\"10\" y=\"{y}\" fill=\"{}\" font-size=\"12\" font-family=\"monospace\" font-weight=\"bold\">{}</text>\n",
                theme.title_color,
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
                    "<text x=\"10\" y=\"{y_cursor}\" fill=\"{}\" font-size=\"12\" font-family=\"monospace\">{}</text>\n",
                    theme.text_color,
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
    let (is_dark, set_is_dark) = signal(true);
    let (search, set_search) = signal(String::new());

    let theme = move || if is_dark.get() { Theme::dark() } else { Theme::light() };

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
        <div style=move || format!(
            "display:flex; flex-direction:column; height:100vh; width:100vw; font-family: sans-serif; background:{};",
            theme().page_bg
        )>
            <div style=move || format!(
                "display:flex; align-items:center; flex-wrap:wrap; gap:6px; padding:6px 10px; background:{}; border-bottom:1px solid {};",
                theme().toolbar_bg, theme().toolbar_border
            )>
                <label style=move || format!("color:{}; font-size:13px;", theme().toolbar_text)>"Format"</label>
                <select style=move || control_style(theme()) on:change=on_format_change>
                    {Format::ALL
                        .iter()
                        .map(|f| view! { <option value=f.label()>{f.label()}</option> })
                        .collect::<Vec<_>>()}
                </select>

                <span style=move || format!("color:{}; margin:0 4px;", theme().toolbar_border)>"|"</span>

                <label style=move || format!("color:{}; font-size:13px;", theme().toolbar_text)>"Convert to"</label>
                <select style=move || control_style(theme()) on:change=on_convert_target_change>
                    {Format::ALL
                        .iter()
                        .map(|f| view! { <option value=f.label()>{f.label()}</option> })
                        .collect::<Vec<_>>()}
                </select>
                <button style=move || control_style(theme()) on:click=on_convert_click>"Convert"</button>

                <span style=move || format!("color:{}; margin:0 4px;", theme().toolbar_border)>"|"</span>

                <label style=move || format!("color:{}; font-size:13px;", theme().toolbar_text)>"Search"</label>
                <input
                    type="text"
                    placeholder="key or value..."
                    style=move || control_style(theme())
                    prop:value=move || search.get()
                    on:input=move |ev| set_search.set(event_target_value(&ev))
                />

                {move || convert_error.get().map(|e| {
                    let color = theme().error_color;
                    view! {
                        <span style=format!("color:{color}; font-size:12px;")>{e}</span>
                    }
                })}
            </div>
            <div style="display:flex; flex:1; min-height:0;">
                <textarea
                    style=move || format!(
                        "width:40%; height:100%; box-sizing:border-box; font-family: monospace; font-size:13px; padding:1em; resize:none; border:1px solid {}; background:{}; color:{};",
                        theme().toolbar_border, theme().node_bg, theme().text_color
                    )
                    prop:value=move || input.get()
                    on:input=move |ev| {
                        set_input.set(event_target_value(&ev));
                        set_convert_error.set(None);
                    }
                />
                <div style=move || format!("width:60%; height:100%; overflow:hidden; background:{};", theme().graph_bg)>
                    {move || match parsed.get() {
                        Ok(data) => view! { <GraphView data=data theme=theme() search=search /> }.into_any(),
                        Err(e) => {
                            let color = theme().error_color;
                            view! {
                                <p style=format!("color:{color}; padding:1em; font-family: monospace; white-space:pre-wrap;")>{e}</p>
                            }.into_any()
                        },
                    }}
                </div>
            </div>
            <button
                title="Toggle theme"
                on:click=move |_| set_is_dark.update(|d| *d = !*d)
                style=move || format!(
                    "position:fixed; top:10px; right:10px; z-index:10; width:32px; height:32px; \
                     border-radius:50%; border:1px solid {}; background:{}; color:{}; \
                     font-size:16px; line-height:1; cursor:pointer; display:flex; \
                     align-items:center; justify-content:center;",
                    theme().toolbar_border, theme().toolbar_bg, theme().toolbar_text
                )
            >
                {move || if is_dark.get() { "\u{1F319}" } else { "\u{2600}\u{FE0F}" }}
            </button>
        </div>
    }
}

#[component]
fn GraphView(data: DataNode, theme: Theme, search: ReadSignal<String>) -> impl IntoView {
    let graph = StoredValue::new(build_graph(&data));

    let (collapsed, set_collapsed) = signal(HashSet::<usize>::new());
    let (selected, set_selected) = signal(None::<usize>);
    let (expanded, set_expanded) = signal(HashSet::<(usize, FieldRef)>::new());

    // A search match hidden under a collapsed ancestor would otherwise stay
    // invisible, defeating the point of searching. Expand every ancestor of
    // a match whenever the query (or matches) change.
    Effect::new(move |_| {
        let query = search.get().to_lowercase();
        let matches = graph.with_value(|g| find_matches(g, &query));
        if matches.is_empty() {
            return;
        }
        set_collapsed.update(|set| {
            graph.with_value(|g| {
                for &id in &matches {
                    let mut current = g.nodes[id].parent;
                    while let Some(pid) = current {
                        set.remove(&pid);
                        current = g.nodes[pid].parent;
                    }
                }
            });
        });
    });

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
            render_static_svg(g, &positions, &expanded_set, theme)
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
            <div style=move || format!(
                "position:absolute; top:0; left:0; right:0; display:flex; justify-content:space-between; align-items:center; padding:6px 10px; font-family:monospace; font-size:12px; color:{}; background:{}cc; z-index:1;",
                theme.toolbar_text, theme.toolbar_bg
            )>
                <span style="pointer-events:none;">
                    {move || {
                        let query = search.get().to_lowercase();
                        if !query.is_empty() {
                            let count = graph.with_value(|g| find_matches(g, &query).len());
                            return format!("{count} match(es)");
                        }
                        selected.get()
                            .map(|id| graph.with_value(|g| g.path_to(id)))
                            .unwrap_or_else(|| "Click a node to select it (click = collapse/expand)".to_string())
                    }}
                </span>
                <span>
                    <button style=control_style(theme) on:click=on_export_svg>"Export SVG"</button>
                    <button style=control_style(theme) on:click=on_export_png>"Export PNG"</button>
                </span>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
                <g style=move || format!("transform: translate({}px, {}px) scale({})", tx.get(), ty.get(), scale.get())>
                    {move || {
                        let collapsed_set = collapsed.get();
                        let expanded_set = expanded.get();
                        let sel = selected.get();
                        let query = search.get().to_lowercase();
                        graph.with_value(|g| {
                            let positions = compute_layout(g, &collapsed_set, &expanded_set);
                            let matches = find_matches(g, &query);
                            let edges = render_edges(g, &positions, theme);
                            let nodes = render_nodes(
                                g,
                                &positions,
                                &collapsed_set,
                                &expanded_set,
                                sel,
                                &matches,
                                theme,
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

fn render_edges(graph: &Graph, positions: &HashMap<usize, NodeLayout>, theme: Theme) -> Vec<impl IntoView> {
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
                    view! { <path d=d fill="none" stroke=theme.edge_color stroke-width="1.5" /> }
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
    marker_color: &'static str,
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
                                fill=marker_color
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
                        fill=marker_color
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
    matches: &HashSet<usize>,
    theme: Theme,
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
            let is_match = matches.contains(&id);

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
                theme.title_color,
                theme.title_color,
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
                    theme.text_color,
                    theme.title_color,
                    None,
                    full,
                    (id, FieldRef::Field(i)),
                    expanded,
                    toggle_expand,
                );
                field_views.extend(views);
                y_cursor += count as f64 * LINE_HEIGHT;
            }

            let stroke = if is_selected {
                theme.node_border_selected
            } else if is_match {
                theme.node_border_match
            } else {
                theme.node_border
            };
            let stroke_width = if is_selected || is_match { "2.5" } else { "1.5" };
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
                        fill=theme.node_bg
                        stroke=stroke
                        stroke-width=stroke_width
                    />
                    {title_views}
                    {(!marker.is_empty()).then(|| view! {
                        <text x=marker_x y="16" fill=theme.toolbar_text font-size="11" font-family="monospace">{marker}</text>
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
