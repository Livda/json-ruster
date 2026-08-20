use crate::convert;
use crate::graph::{build_graph, Graph};
use crate::layout::{
    field_full_text, layout as compute_layout, truncate_display, wrap_text, FieldRef, NodeLayout,
    Orientation, LESS_MARKER, LINE_HEIGHT, MORE_MARKER,
};
use crate::model::DataNode;
use crate::parsers::{self, Format};
use base64::Engine as _;
use leptos::ev::{MouseEvent, PointerEvent, WheelEvent};
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub page_bg: &'static str,
    pub toolbar_bg: &'static str,
    pub toolbar_border: &'static str,
    pub toolbar_text: &'static str,
    pub graph_bg: &'static str,
    pub node_bg: &'static str,
    pub node_border: &'static str,
    pub node_border_selected: &'static str,
    pub node_border_match: &'static str,
    pub edge_color: &'static str,
    pub title_color: &'static str,
    pub text_color: &'static str,
    pub error_color: &'static str,
}

impl Theme {
    pub fn dark() -> Self {
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

    pub fn light() -> Self {
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
pub fn control_style(theme: Theme) -> String {
    format!(
        "background:{}; color:{}; border:1px solid {}; border-radius:4px; padding:2px 6px; font-size:13px;",
        theme.node_bg, theme.text_color, theme.toolbar_border
    )
}

/// Nodes whose title or a field's key/value contains `query`
/// (case-insensitive). Searched across the whole graph regardless of
/// collapse state, since a match hidden under a collapsed ancestor should
/// still surface (and that ancestor gets auto-expanded, see `GraphView`).
pub fn find_matches(graph: &Graph, query: &str) -> HashSet<usize> {
    if query.is_empty() {
        return HashSet::new();
    }
    graph
        .nodes
        .iter()
        .filter(|n| {
            n.title.to_lowercase().contains(query)
                || n.fields.iter().any(|(k, v)| {
                    k.to_lowercase().contains(query) || v.to_lowercase().contains(query)
                })
        })
        .map(|n| n.id)
        .collect()
}

const STORAGE_FORMAT_KEY: &str = "json-ruster:format";
const STORAGE_INPUT_KEY: &str = "json-ruster:input";
const STORAGE_THEME_KEY: &str = "json-ruster:theme";

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Falls back to dark if the OS preference can't be read at all (e.g. very
/// old browsers), matching the app's original always-dark default.
fn prefers_dark_theme() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
        .map(|mql| mql.matches())
        .unwrap_or(true)
}

fn initial_theme() -> bool {
    if let Some(Some(value)) = local_storage().map(|s| s.get_item(STORAGE_THEME_KEY).ok().flatten())
    {
        return value == "dark";
    }
    prefers_dark_theme()
}

/// Cap on the *compressed, base64* share payload, not the raw document --
/// that's what actually ends up in the URL. The fragment is never sent to
/// a server, but chat apps, browsers and clipboard managers can still
/// choke on or silently truncate extremely long URLs.
const MAX_SHARE_DATA_LEN: usize = 8_000;

fn compress_for_url(input: &str) -> String {
    let compressed = miniz_oxide::deflate::compress_to_vec(input.as_bytes(), 8);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(compressed)
}

fn decompress_from_url(data: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(data)
        .ok()?;
    let decompressed = miniz_oxide::inflate::decompress_to_vec(&bytes).ok()?;
    String::from_utf8(decompressed).ok()
}

/// A shared link encodes the format and DEFLATE+base64-compressed text in
/// the URL fragment (`#format=Json&data=...`), so opening it reconstructs
/// the same document without any server round-trip.
fn parse_share_hash() -> Option<(Format, String)> {
    let hash = web_sys::window()?.location().hash().ok()?;
    let hash = hash.strip_prefix('#')?;
    let mut format = None;
    let mut data = None;
    for pair in hash.split('&') {
        let (k, v) = pair.split_once('=')?;
        match k {
            "format" => format = Format::from_label(v),
            "data" => data = decompress_from_url(v),
            _ => {}
        }
    }
    Some((format?, data?))
}

/// Builds a shareable URL for `input` against `base` (the page URL without
/// its fragment), or an error message if the compressed payload is still
/// too large to share as a link. Kept separate from `share_url` so the
/// size-limit logic is testable without touching `web_sys::window()`,
/// which panics outside a real browser/wasm environment.
fn share_url_with_base(format: Format, input: &str, base: &str) -> Result<String, String> {
    let encoded = compress_for_url(input);
    if encoded.len() > MAX_SHARE_DATA_LEN {
        return Err(format!(
            "Document too large to share as a link ({} KB compressed, limit {} KB)",
            encoded.len().div_ceil(1024),
            MAX_SHARE_DATA_LEN / 1024
        ));
    }
    Ok(format!("{base}#format={}&data={encoded}", format.label()))
}

fn share_url(format: Format, input: &str) -> Result<String, String> {
    let href = web_sys::window()
        .and_then(|w| w.location().href().ok())
        .unwrap_or_default();
    let base = href.split('#').next().unwrap_or(&href).to_string();
    share_url_with_base(format, input, &base)
}

/// Initial (format, text) for the editor: a share link in the URL wins,
/// then whatever was last saved locally, then the default JSON sample.
fn initial_document() -> (Format, String) {
    if let Some(shared) = parse_share_hash() {
        return shared;
    }
    let storage = local_storage();
    let format = storage
        .as_ref()
        .and_then(|s| s.get_item(STORAGE_FORMAT_KEY).ok().flatten())
        .and_then(|s| Format::from_label(&s));
    let input = storage.and_then(|s| s.get_item(STORAGE_INPUT_KEY).ok().flatten());
    match (format, input) {
        (Some(format), Some(input)) => (format, input),
        _ => (Format::Json, Format::Json.sample().to_string()),
    }
}

fn copy_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
}

/// Toggles OS-level fullscreen (the Fullscreen API, not just an in-page
/// "maximize") for `el`. Checking `document.fullscreen_element()` rather
/// than tracking our own state means this stays correct even if the user
/// exits fullscreen some other way (Esc, browser UI).
fn toggle_fullscreen(el: web_sys::Element) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if document.fullscreen_element().is_some() {
        document.exit_fullscreen();
    } else {
        let _ = el.request_fullscreen();
    }
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
        let Ok(canvas) = document.create_element("canvas").and_then(|c| {
            c.dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(Into::into)
        }) else {
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
        if ctx
            .draw_image_with_html_image_element(&img_for_draw, 0.0, 0.0)
            .is_ok()
        {
            if let Ok(png_url) = canvas.to_data_url_with_type("image/png") {
                trigger_download("graph.png", &png_url);
            }
        }
    });
    img.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
}

fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
    orientation: Orientation,
) -> (String, f64, f64) {
    let width = positions
        .values()
        .map(|p| p.x + p.width)
        .fold(0.0_f64, f64::max)
        + 40.0;
    let height = positions
        .values()
        .map(|p| p.y + p.height)
        .fold(0.0_f64, f64::max)
        + 40.0;

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"{width}\" height=\"{height}\" fill=\"{}\" />\n\
         <g transform=\"translate(20, 20)\">\n",
        theme.graph_bg
    );

    for (&id, from) in positions {
        for &child_id in &graph.nodes[id].children {
            if let Some(to) = positions.get(&child_id) {
                let [x1, y1, x2, y2, mid] = edge_anchors(*from, *to, orientation);
                let d = match orientation {
                    Orientation::Vertical => {
                        format!("M {x1} {y1} C {x1} {mid}, {x2} {mid}, {x2} {y2}")
                    }
                    Orientation::Horizontal => {
                        format!("M {x1} {y1} C {mid} {y1}, {mid} {y2}, {x2} {y2}")
                    }
                };
                svg.push_str(&format!(
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\" />\n",
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
pub fn App() -> impl IntoView {
    let (initial_format, initial_input) = initial_document();
    let (format, set_format) = signal(initial_format);
    let (input, set_input) = signal(initial_input);
    let (convert_target, set_convert_target) = signal(Format::Yaml);
    let (convert_error, set_convert_error) = signal(None::<String>);
    let (is_dark, set_is_dark) = signal(initial_theme());
    let (copied, set_copied) = signal(false);
    let (share_copied, set_share_copied) = signal(false);
    let (share_error, set_share_error) = signal(None::<String>);
    let editor_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // Remember the current document and theme locally so a reload picks up
    // where the user left off.
    Effect::new(move |_| {
        if let Some(storage) = local_storage() {
            let _ = storage.set_item(STORAGE_FORMAT_KEY, format.get().label());
            let _ = storage.set_item(STORAGE_INPUT_KEY, &input.get());
        }
    });
    Effect::new(move |_| {
        if let Some(storage) = local_storage() {
            let value = if is_dark.get() { "dark" } else { "light" };
            let _ = storage.set_item(STORAGE_THEME_KEY, value);
        }
    });
    // `search_input` mirrors the box immediately for a responsive typing
    // feel; `search` (fed into GraphView) only updates ~200ms after the
    // user stops typing, since find_matches rescans every node on every
    // change and a big document would otherwise re-scan on each keystroke.
    let (search_input, set_search_input) = signal(String::new());
    let (search, set_search) = signal(String::new());
    let search_debounce: StoredValue<Option<TimeoutHandle>> = StoredValue::new(None);

    let theme = move || {
        if is_dark.get() {
            Theme::dark()
        } else {
            Theme::light()
        }
    };

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
            Err(_) => {
                set_convert_error.set(Some("Fix the parsing error before converting".to_string()))
            }
        }
    };

    let on_copy_click = move |_| {
        copy_to_clipboard(&input.get_untracked());
        set_copied.set(true);
        set_timeout(
            move || set_copied.set(false),
            std::time::Duration::from_millis(1200),
        );
    };

    let on_share_click = move |_| {
        let current_format = format.get_untracked();
        let current_input = input.get_untracked();
        match share_url(current_format, &current_input) {
            Ok(url) => {
                if let Some(location) = web_sys::window().map(|w| w.location()) {
                    if let Some(hash) = url.split_once('#').map(|(_, h)| h) {
                        let _ = location.set_hash(hash);
                    }
                }
                copy_to_clipboard(&url);
                set_share_error.set(None);
                set_share_copied.set(true);
                set_timeout(
                    move || set_share_copied.set(false),
                    std::time::Duration::from_millis(1200),
                );
            }
            Err(e) => {
                set_share_copied.set(false);
                set_share_error.set(Some(e));
            }
        }
    };

    view! {
        <div style=move || format!(
            "display:flex; flex-direction:column; height:100vh; width:100vw; font-family: sans-serif; background:{};",
            theme().page_bg
        )>
            <div style=move || format!(
                "position:relative; display:flex; align-items:center; flex-wrap:wrap; gap:6px; padding:6px 44px 6px 10px; background:{}; border-bottom:1px solid {};",
                theme().toolbar_bg, theme().toolbar_border
            )>
                <button
                    title="Toggle theme"
                    on:click=move |_| set_is_dark.update(|d| *d = !*d)
                    style=move || format!(
                        "position:absolute; top:50%; right:8px; transform:translateY(-50%); \
                         width:28px; height:28px; border-radius:50%; border:1px solid {}; \
                         background:{}; color:{}; font-size:14px; line-height:1; cursor:pointer; \
                         display:flex; align-items:center; justify-content:center;",
                        theme().toolbar_border, theme().node_bg, theme().toolbar_text
                    )
                >
                    {move || if is_dark.get() { "\u{1F319}" } else { "\u{2600}\u{FE0F}" }}
                </button>
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

                <button style=move || control_style(theme()) on:click=on_copy_click>
                    {move || if copied.get() { "Copied!" } else { "Copy" }}
                </button>
                <button style=move || control_style(theme()) on:click=on_share_click title="Copy a shareable link to this document">
                    {move || if share_copied.get() { "Link copied!" } else { "Share" }}
                </button>

                <span style=move || format!("color:{}; margin:0 4px;", theme().toolbar_border)>"|"</span>

                <label style=move || format!("color:{}; font-size:13px;", theme().toolbar_text)>"Search"</label>
                <input
                    type="text"
                    placeholder="key or value..."
                    style=move || control_style(theme())
                    prop:value=move || search_input.get()
                    on:input=move |ev| {
                        let value = event_target_value(&ev);
                        set_search_input.set(value.clone());
                        if let Some(handle) = search_debounce.get_value() {
                            handle.clear();
                        }
                        let handle = set_timeout_with_handle(
                            move || set_search.set(value),
                            std::time::Duration::from_millis(200),
                        )
                        .ok();
                        search_debounce.set_value(handle);
                    }
                />

                {move || convert_error.get().map(|e| {
                    let color = theme().error_color;
                    view! {
                        <span style=format!("color:{color}; font-size:12px;")>{e}</span>
                    }
                })}
                {move || share_error.get().map(|e| {
                    let color = theme().error_color;
                    view! {
                        <span style=format!("color:{color}; font-size:12px;")>{e}</span>
                    }
                })}
            </div>
            <div style="display:flex; flex:1; min-height:0;">
                <div node_ref=editor_ref style="position:relative; width:40%; height:100%;">
                    <textarea
                        style=move || format!(
                            "width:100%; height:100%; box-sizing:border-box; font-family: monospace; font-size:13px; padding:1em; resize:none; border:1px solid {}; background:{}; color:{};",
                            theme().toolbar_border, theme().node_bg, theme().text_color
                        )
                        prop:value=move || input.get()
                        on:input=move |ev| {
                            set_input.set(event_target_value(&ev));
                            set_convert_error.set(None);
                        }
                    />
                    <button
                        title="Toggle fullscreen"
                        on:click=move |_| {
                            if let Some(el) = editor_ref.get() {
                                toggle_fullscreen(el.into());
                            }
                        }
                        style=move || format!(
                            "position:absolute; top:8px; right:8px; border:1px solid {}; \
                             background:{}cc; color:{}; border-radius:4px; padding:2px 6px; \
                             font-size:12px; cursor:pointer;",
                            theme().toolbar_border, theme().node_bg, theme().text_color
                        )
                    >
                        "⛶"
                    </button>
                </div>
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
        </div>
    }
}

#[component]
fn GraphView(data: DataNode, theme: Theme, search: ReadSignal<String>) -> impl IntoView {
    let graph = StoredValue::new(build_graph(&data));
    let root_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    let (collapsed, set_collapsed) = signal(HashSet::<usize>::new());
    let (selected, set_selected) = signal(None::<usize>);
    let (expanded, set_expanded) = signal(HashSet::<(usize, FieldRef)>::new());
    let (orientation, set_orientation) = signal(Orientation::Vertical);

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
    let pointermove_handle =
        window_event_listener(leptos::ev::pointermove, move |ev: PointerEvent| {
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
        let orientation_value = orientation.get_untracked();
        let before = collapsed.get_untracked();
        let pos_before =
            graph.with_value(|g| compute_layout(g, &before, &expanded_set, orientation_value));

        set_collapsed.update(|set| {
            if !set.remove(&id) {
                set.insert(id);
            }
        });

        let after = collapsed.get_untracked();
        let pos_after =
            graph.with_value(|g| compute_layout(g, &after, &expanded_set, orientation_value));

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
        let orientation_value = orientation.get_untracked();
        graph.with_value(|g| {
            let positions = compute_layout(g, &collapsed_set, &expanded_set, orientation_value);
            render_static_svg(g, &positions, &expanded_set, theme, orientation_value)
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
            node_ref=root_ref
            style=move || format!(
                "width:100%; height:100%; position:relative; cursor:{}; background:{};",
                if is_dragging.get() { "grabbing" } else { "grab" },
                theme.graph_bg
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
                    <button
                        title="Toggle fullscreen"
                        style=control_style(theme)
                        on:click=move |_| {
                            if let Some(el) = root_ref.get() {
                                toggle_fullscreen(el.into());
                            }
                        }
                    >
                        "⛶"
                    </button>
                    <button
                        title="Rotate 90°"
                        style=control_style(theme)
                        on:click=move |_| set_orientation.update(|o| *o = o.toggle())
                    >
                        "↻"
                    </button>
                    <button style=control_style(theme) on:click=on_export_svg>"Export SVG"</button>
                    <button style=control_style(theme) on:click=on_export_png>"Export PNG"</button>
                </span>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
                <g style=move || format!("transform: translate({}px, {}px) scale({})", tx.get(), ty.get(), scale.get())>
                    {move || {
                        let collapsed_set = collapsed.get();
                        let expanded_set = expanded.get();
                        let orientation_value = orientation.get();
                        let sel = selected.get();
                        let query = search.get().to_lowercase();
                        graph.with_value(|g| {
                            let positions =
                                compute_layout(g, &collapsed_set, &expanded_set, orientation_value);
                            let matches = find_matches(g, &query);
                            let edges = render_edges(g, &positions, theme, orientation_value);
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

/// Anchor points for the edge connecting `from` to `to`: the midpoint of
/// `from`'s trailing edge (bottom in `Vertical`, right in `Horizontal`) and
/// `to`'s leading edge (top / left), plus the control-point coordinate the
/// bezier curve bulges towards along the growth axis.
fn edge_anchors(from: NodeLayout, to: NodeLayout, orientation: Orientation) -> [f64; 5] {
    match orientation {
        Orientation::Vertical => {
            let x1 = from.x + from.width / 2.0;
            let y1 = from.y + from.height;
            let x2 = to.x + to.width / 2.0;
            let y2 = to.y;
            [x1, y1, x2, y2, (y1 + y2) / 2.0]
        }
        Orientation::Horizontal => {
            let x1 = from.x + from.width;
            let y1 = from.y + from.height / 2.0;
            let x2 = to.x;
            let y2 = to.y + to.height / 2.0;
            [x1, y1, x2, y2, (x1 + x2) / 2.0]
        }
    }
}

fn render_edges(
    graph: &Graph,
    positions: &HashMap<usize, NodeLayout>,
    theme: Theme,
    orientation: Orientation,
) -> Vec<impl IntoView> {
    positions
        .keys()
        .flat_map(|&id| {
            let from = positions[&id];
            graph.nodes[id]
                .children
                .iter()
                .filter_map(move |&child_id| {
                    positions.get(&child_id).map(|&to| {
                        let [x1, y1, x2, y2, mid] = edge_anchors(from, to, orientation);
                        let d = match orientation {
                            Orientation::Vertical => {
                                format!("M {x1} {y1} C {x1} {mid}, {x2} {mid}, {x2} {y2}")
                            }
                            Orientation::Horizontal => {
                                format!("M {x1} {y1} C {mid} {y1}, {mid} {y2}, {x2} {y2}")
                            }
                        };
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
#[allow(clippy::too_many_arguments)]
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

#[allow(clippy::too_many_arguments)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataNode;

    #[test]
    fn find_matches_is_case_insensitive_on_keys_and_values() {
        let data = DataNode::Object(vec![
            ("Name".into(), DataNode::Scalar("Alice".into())),
            ("role".into(), DataNode::Scalar("Admin".into())),
        ]);
        let graph = build_graph(&data);

        let by_key = find_matches(&graph, "name");
        let by_value = find_matches(&graph, "admin");
        assert_eq!(by_key, HashSet::from([graph.root]));
        assert_eq!(by_value, HashSet::from([graph.root]));
    }

    #[test]
    fn find_matches_is_empty_for_an_empty_query() {
        let data = DataNode::Object(vec![("a".into(), DataNode::Scalar("b".into()))]);
        let graph = build_graph(&data);
        assert!(find_matches(&graph, "").is_empty());
    }

    #[test]
    fn find_matches_returns_nothing_for_no_match() {
        let data = DataNode::Object(vec![("a".into(), DataNode::Scalar("b".into()))]);
        let graph = build_graph(&data);
        assert!(find_matches(&graph, "nope").is_empty());
    }

    #[test]
    fn control_style_uses_the_theme_colors() {
        let theme = Theme::dark();
        let style = control_style(theme);
        assert!(style.contains(theme.node_bg));
        assert!(style.contains(theme.text_color));
        assert!(style.contains(theme.toolbar_border));
    }

    #[test]
    fn dark_and_light_themes_use_different_colors() {
        assert_ne!(Theme::dark(), Theme::light());
    }

    #[test]
    fn compress_for_url_round_trips() {
        let original = r#"{"a": 1, "b": ["x", "y", "z"], "c": {"nested": true}}"#;
        let encoded = compress_for_url(original);
        assert_eq!(decompress_from_url(&encoded), Some(original.to_string()));
    }

    #[test]
    fn compress_for_url_shrinks_repetitive_text() {
        let original = "a".repeat(10_000);
        let encoded = compress_for_url(&original);
        assert!(
            encoded.len() < original.len() / 10,
            "expected heavy compression for repetitive text, got {} bytes from {}",
            encoded.len(),
            original.len()
        );
    }

    #[test]
    fn share_url_succeeds_for_a_small_document() {
        let url = share_url_with_base(Format::Json, r#"{"a": 1}"#, "https://example/").unwrap();
        assert!(url.starts_with("https://example/#"));
        assert!(url.contains("format=JSON"));
        assert!(url.contains("data="));
    }

    #[test]
    fn share_url_rejects_documents_too_large_to_share() {
        // A short cycle would compress away to almost nothing; use a cheap
        // PRNG so the text has enough entropy to stay well past
        // MAX_SHARE_DATA_LEN even after DEFLATE + base64.
        let mut state: u32 = 12345;
        let big: String = (0..200_000)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (b'a' + (state >> 16) as u8 % 26) as char
            })
            .collect();
        let err = share_url_with_base(Format::Json, &big, "https://example/").unwrap_err();
        assert!(err.contains("too large"));
    }
}
