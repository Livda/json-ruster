use json_ruster::graph::{build_graph, Graph};
use json_ruster::layout::{layout as compute_layout, NodeLayout};
use json_ruster::model::DataNode;
use json_ruster::parsers;
use leptos::prelude::*;
use std::collections::HashMap;

const DEFAULT_JSON: &str = r#"{
  "name": "json-ruster",
  "version": "0.1.0",
  "tags": ["json", "rust", "wasm"],
  "author": {
    "name": "Aurelien",
    "active": true
  }
}"#;

#[component]
fn App() -> impl IntoView {
    let (input, set_input) = signal(DEFAULT_JSON.to_string());

    let parsed = Memo::new(move |_| parsers::json::parse_json(&input.get()));

    view! {
        <div style="display:flex; height:100vh; width:100vw; font-family: sans-serif;">
            <textarea
                style="width:40%; height:100%; box-sizing:border-box; font-family: monospace; font-size:13px; padding:1em;"
                prop:value=move || input.get()
                on:input=move |ev| set_input.set(event_target_value(&ev))
            />
            <div style="width:60%; height:100%; overflow:auto; background:#0f1117;">
                {move || match parsed.get() {
                    Ok(data) => view! { <GraphView data=data /> }.into_any(),
                    Err(e) => view! {
                        <p style="color:#ff6b6b; padding:1em; font-family: monospace;">{e}</p>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
fn GraphView(data: DataNode) -> impl IntoView {
    let graph = build_graph(&data);
    let positions = compute_layout(&graph);

    let max_x = positions.values().map(|p| p.x + p.width).fold(0.0_f64, f64::max);
    let max_y = positions.values().map(|p| p.y + p.height).fold(0.0_f64, f64::max);

    let edges = render_edges(&graph, &positions);
    let nodes = render_nodes(&graph, &positions);

    view! {
        <svg
            width=format!("{}", max_x + 40.0)
            height=format!("{}", max_y + 40.0)
            style="min-width:100%; min-height:100%;"
        >
            <g transform="translate(20, 20)">
                {edges}
                {nodes}
            </g>
        </svg>
    }
}

fn render_edges(graph: &Graph, positions: &HashMap<usize, NodeLayout>) -> Vec<impl IntoView> {
    graph
        .nodes
        .iter()
        .flat_map(|node| {
            let from = positions[&node.id];
            node.children.iter().map(move |&child_id| {
                let to = positions[&child_id];
                let x1 = from.x + from.width / 2.0;
                let y1 = from.y + from.height;
                let x2 = to.x + to.width / 2.0;
                let y2 = to.y;
                let mid_y = (y1 + y2) / 2.0;
                let d = format!("M {x1} {y1} C {x1} {mid_y}, {x2} {mid_y}, {x2} {y2}");
                view! { <path d=d fill="none" stroke="#4a5568" stroke-width="1.5" /> }
            })
        })
        .collect::<Vec<_>>()
}

fn render_nodes(graph: &Graph, positions: &HashMap<usize, NodeLayout>) -> Vec<impl IntoView> {
    graph
        .nodes
        .iter()
        .map(|node| {
            let pos = positions[&node.id];
            let title = node.title.clone();

            let field_lines: Vec<_> = node
                .fields
                .iter()
                .enumerate()
                .map(|(i, (k, v))| {
                    let text = if k.is_empty() {
                        v.clone()
                    } else {
                        format!("{k}: {v}")
                    };
                    let y = 30.0 + i as f64 * 20.0;
                    view! {
                        <text x="10" y=y fill="#e2e8f0" font-size="12" font-family="monospace">
                            {text}
                        </text>
                    }
                })
                .collect();

            view! {
                <g transform=format!("translate({}, {})", pos.x, pos.y)>
                    <rect
                        width=pos.width
                        height=pos.height
                        rx="6"
                        fill="#1a202c"
                        stroke="#4a5568"
                        stroke-width="1.5"
                    />
                    <text x="10" y="16" fill="#63b3ed" font-size="12" font-family="monospace" font-weight="bold">
                        {title}
                    </text>
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
