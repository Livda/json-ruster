# Changelog

Format based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/) v2.0.0.

## [Unreleased]

## [0.7.0] - Milestone 7 — GitHub Actions CI

### Added
- `.github/workflows/ci.yml` with jobs `fmt`/`clippy` (lint), `test` (`cargo test --lib`), `build-wasm` (`trunk build --release`, uploaded as an artifact), and `docker` (build and push the production image to GHCR at `ghcr.io/<repo>`, tagged with the commit SHA and `latest`, on pushes to `main`).
- Cargo build cache via `Swatinem/rust-cache` shared across the clippy/test/build-wasm jobs.

### Fixed
- Applied `cargo fmt` across the codebase and fixed the two `clippy::too_many_arguments` lints on `truncatable_lines`/`render_nodes` (via a targeted `#[allow]`, not worth a bigger refactor right now) so the new lint/test CI jobs start clean instead of red on day one.

### Notes
- A GitLab CI pipeline was set up first and then replaced with this GitHub Actions one at the user's request.
- The `docker` job's push step could not be exercised here (no GitHub Actions runner in this environment); `docker build`, `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test --lib` were all run and verified locally. The push relies on the repo's built-in `GITHUB_TOKEN` and the GitHub Container Registry, both available by default with no extra secrets to configure.
## [0.6.0] - Milestone 6 — Docker

### Added
- Multi-stage `Dockerfile`: a `rust:slim-bookworm` stage builds the app with Trunk (`trunk build --release`), and the runtime stage is `nginx:alpine` serving the static output on port 80 — no Rust toolchain in the final image.
- `.dockerignore` excluding `target`, `dist` and `.git` from the build context.
- `compose.yaml`: `docker compose up --build` builds the image and exposes it on `http://127.0.0.1:8080`.

### Fixed
- Editor textarea kept the browser's default white background/black text regardless of theme; it now follows `Theme.node_bg`/`text_color`.
- `select`/`input`/`button` controls (format, convert-to, search, export buttons) also kept a white background in dark mode; added a shared `control_style(theme)` helper applied to all of them.
- The theme toggle button, first added as a `position:fixed` icon at the viewport's top-right corner, overlapped the graph panel's "Export SVG"/"Export PNG" buttons (also anchored top-right). Moved it into the top toolbar row instead, positioned relative to that row rather than the viewport.

## [0.5.0] - Milestone 5 — Theme & search

### Added
- Light/dark theme toggle: a `Theme` struct of colors threaded through the toolbar, editor panel, node/edge rendering and SVG/PNG export, so exported files match whichever theme was active.
- Search box: highlights nodes whose title or any field key/value contains the query (case-insensitive) with a gold border, and shows a match count in the graph panel's info bar. Collapsed ancestors of a match are automatically expanded (`find_matches` + an `Effect` watching the query) so matches stay reachable.
- "Export SVG"/"Export PNG" buttons (top-right of the graph panel): render the graph as a standalone SVG document from the data itself -- independent of the live view's pan/zoom or panel size -- then download it directly, or draw it into an offscreen canvas and download a PNG (`render_static_svg` in `main.rs`).

### Fixed
- Panic ("already been disposed") when a `window_event_listener` callback (pan drag) kept running after its `GraphView` was unmounted, e.g. right after a format change/conversion. `window_event_listener` does not auto-cleanup; the handle is now removed via `on_cleanup`.
- Clicking a non-root node reset pan/zoom to the origin, jumping the view away from wherever the user was looking. Replaced with a precise fix: the toggle now shifts the pan by exactly the clicked node's layout-position delta, so it stays under the cursor regardless of how the rest of the layout reshuffles (this also fixes the original "root goes off-screen on a large document" report).
- Stray blue underline rendered as a lone `_` before the `[...]`/`[-]` markers (the leading space in the marker text inherited `text-decoration:underline`). Dropped the underline.

### Notes
- No syntax-highlighting editor: a real one (e.g. CodeMirror) would need a JS dependency, which doesn't fit a pure-Rust/WASM app. The plain textarea stays.

## [0.4.0] - Milestone 4 — Conversion & export

### Added
- `convert::convert`: serializes a `DataNode` back to JSON, YAML, XML, CSV or TOML text, inferring numbers/booleans/null from scalar text instead of quoting everything as a string.
- "Convert to" selector + button in the toolbar: rewrites the editor's content (and switches the active format) to the chosen target format.
- "Export SVG" button: downloads the currently rendered graph as a standalone `.svg` file.

### Notes
- TOML has no null type, so `DataNode::Null` becomes an empty string when converting to TOML.
- CSV export requires a top-level array of objects; nested object/array values inside a row are dropped (flattening them is out of scope for now).

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
