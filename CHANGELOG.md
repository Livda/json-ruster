# Changelog

Format based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/) v2.0.0.

## [Unreleased]

### Fixed
- Vertical overlap between boxes when a node had more fields than the fixed row height allowed: each row's height is now computed dynamically (tallest node in the previous row), instead of a `LEVEL_HEIGHT` constant.
- Text overflowing outside boxes for very long values (e.g. paragraphs of text): "key: value" lines and the title are now truncated to 48 characters (`layout::truncate_display` / `field_text`).
- The native tooltip (SVG `<title>`) didn't show on hover (conflicted with Leptos's handling of the HTML `<title>` tag): replaced with a clickable `[...]` marker.
- The `[...]` marker opened the full text in a panel docked at the bottom of the graph instead of in place: clicking it now expands the line inside the box itself (text wrapped over several lines via `layout::wrap_text`, the box grows taller), with a `[-]` marker to collapse it back. Tracked via `FieldRef`/`layout::layout` (new `expanded` parameter).

## [0.3.0] - Milestone 3 — Multi-format

### Added
- YAML (`serde_yaml`), XML (`roxmltree`), CSV (`csv`) and TOML (`toml`) parsers to `DataNode`, each with unit tests.
- `parsers::Format`: enum of supported formats, with a sample (`sample()`) and parsing dispatch (`parsers::parse`).
- Format selector in the UI; switching format reloads the editor with a representative sample.
- Parsing errors (JSON/YAML/XML/CSV/TOML) shown directly in the graph panel.

### Notes
- No debounce on input: re-parsing on every keystroke is negligible for the document sizes targeted here, so it was left out to keep things simple (the initial plan had considered it).

## [0.2.0] - Milestone 2 — Interactivity

### Added
- Pan (drag) and zoom (wheel) on the graph via CSS transform, `pointerdown`/`pointermove`/`pointerup`/`wheel` listeners.
- Collapse/expand a subtree by clicking a node, with an indicator (`+N` collapsed / `-` expanded); the layout now only computes visible nodes.
- Node selection (highlighted outline) with its path shown (e.g. `root.tags[0]`) in a bar above the graph.
- `GraphNode::parent` and `Graph::path_to` to reconstruct a node's path back to the root.
- Unit tests for `path_to` and for the layout with collapsed nodes.

## [0.1.0] - Milestone 1 — Foundation

### Added
- Leptos (CSR) + Trunk project scaffold, targeting `wasm32-unknown-unknown`.
- `model::DataNode`: unified internal representation (Object/Array/Scalar/Null).
- JSON parser (`serde_json` → `DataNode`).
- `graph::build_graph`: builds a tree of displayable nodes from a `DataNode`.
- `layout::layout`: node positioning via a simplified Reingold–Tilford-style algorithm (no overlap between subtrees).
- SVG rendering of nodes/edges in a Leptos component, with a live-synced editor (textarea).
- Unit tests for the parsers and the layout, runnable via `cargo test --lib`.
