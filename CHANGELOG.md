# Changelog

Format based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/) v2.0.0.

## [Unreleased]

## [0.11.8] - Separate "Format" from "Convert"

### Changed
- Switching the "Format" selector no longer replaces the editor content with a sample -- it only changes which parser reads the current text. That side effect is now its own "Load sample" button, so "Format" can't be mistaken for a second way to convert the document (the "Convert to" + "Convert" pair is the only control that transforms content).

## [0.11.7] - Fix zoom/rotate pushing the graph out of view

### Fixed
- Mouse-wheel zoom scaled around the graph's local origin instead of the cursor, so on a large graph a few wheel ticks could push everything off-screen; it now keeps the point under the cursor fixed (`zoom_anchored_pan`).
- Rotating 90° or using Expand/Collapse all reshapes the whole layout (unlike toggling one node, there's no single clicked node to keep in place), so a stale pan/zoom could end up framing empty space -- these now reset the view.

## [0.11.6] - Expand/collapse all, value type coloring

### Added
- "Expand all"/"Collapse all" buttons in the graph toolbar, resetting every node's collapse state at once.
- Field values are colored by inferred type (string/number/boolean/null, via `convert::infer_scalar`) instead of a single default text color, in both the interactive view and SVG/PNG export.

## [0.11.5] - Rotate the graph 90°

### Added
- A "↻" button on the graph panel toggles between the default top-to-bottom tree layout and a left-to-right one (`layout::Orientation`), like JSON Crack's rotate control. Applies to the interactive view and to SVG/PNG export alike.

## [0.11.4] - CI/coverage badges

### Added
- README badges for CI status (GitHub's native workflow badge) and line coverage (a `shields.io` endpoint badge reading `.github/badges/coverage.json`, regenerated and committed back to `main` by the `test` job on every push there).

## [0.11.3] - Fullscreen mode

### Added
- A "⛶" button on both the editor and the graph panel toggles fullscreen (Fullscreen API) for that panel independently.

### Fixed
- `tests/ui.rs`'s DOM suite runs all tests in one shared browser page, so `localStorage` (and the URL hash) written by one test leaked into the next test's initial state; `mount_app()` now clears both before each mount so every test starts from the same default document regardless of run order.

## [0.11.2] - Surface coverage on GitHub

### Added
- CI's `test` job writes the `cargo-llvm-cov` per-file summary table to the workflow run's Summary tab (`$GITHUB_STEP_SUMMARY`), so coverage is visible directly in GitHub's UI without downloading the `lcov.info` artifact.

### Notes
- Kept GitHub-native on purpose (no Codecov/Coveralls account, no extra permissions): the user picked the Summary-tab option over a PR-comment bot, which would need `pull-requests: write`.

## [0.11.1] - Share link: compression + size limit

### Changed
- Share links now DEFLATE-compress the document (`miniz_oxide`) and base64-encode it (`base64`, URL-safe, no padding) instead of a plain percent-encoded copy, shrinking typical shared URLs considerably.
- Sharing a document whose compressed payload exceeds 8 KB is refused with an inline error instead of producing a link that browsers/chat apps may truncate or choke on.

### Notes
- The 8 KB cap is on the compressed+base64 payload, not the raw document, since that's what actually lands in the URL.

## [0.11.0] - Batch 3/3: UX, plus coverage

### Added
- Theme defaults to the OS's `prefers-color-scheme` on first visit instead of always dark.
- The current document (format + text) and theme are persisted to `localStorage` and restored on reload.
- "Share" button: encodes the format and text into the URL fragment (`#format=...&data=...`) and copies the link; opening a shared link reconstructs the same document client-side, no server involved. It takes priority over anything saved locally.
- "Copy" button: copies the editor's current content to the clipboard.
- `cargo-llvm-cov` (+ `llvm-tools-preview`) pre-installed in the `Dockerfile`'s `builder` stage, so `docker compose --profile test run --rm test cargo llvm-cov --lib ...` produces a terminal summary or an HTML/lcov report for the native test suite. Wired into CI's `test` job too (`lcov.info` uploaded as an artifact).
- `cargo-home` named volume (`$CARGO_HOME`) for the `dev`/`test` services, seeded from the image on first use: installing a new dev tool at runtime (e.g. `cargo install` inside the container) now persists across container recreations without a `Dockerfile` change/rebuild.

### Fixed
- The `chromedriver` service was being rebuilt reflexively even though its stage has no dependency on the app's source (just chromium/chromedriver from apt) -- it never needed it. The `test` service now bind-mounts the source like `dev` too, so ordinary code changes no longer require `--build` at all, only changes to `Cargo.toml`/the `Dockerfile` itself do.

### Notes
- Coverage is native-only (the pure-logic `cargo test --lib` suite); the browser-driven `tests/ui.rs` suite isn't instrumented for coverage or wired into CI yet, since neither is currently worth the added complexity for four DOM-interaction tests.

## [0.10.0] - Batch 2/3: Robustness

### Added
- Debounced search: the box updates immediately, but `find_matches` (which rescans every node) only runs ~200ms after the user stops typing.
- `tests/ui.rs`: wasm-bindgen-test DOM integration suite (mounts `App` in a real browser, dispatches real input/click events) covering the format select, JSON parse error vs. graph rendering, and the theme toggle. Moved all UI code from `src/main.rs` into `src/ui.rs` (part of the library) so these tests can import and mount it; `main.rs` is now just the `mount_to_body` call.
- Pure-logic unit tests for `find_matches` and `control_style` in `src/ui.rs`, run via plain `cargo test --lib` (no browser).
- Containerized dev/test workflow, all on the compose bridge network (no host Rust toolchain, no `--network host`): a `dev` service (`trunk serve`, bind-mounted source, live reload) and a `test` service (runs `cargo test --target wasm32-unknown-unknown --test ui` against a `chromedriver` service on the same network), both reusing the `Dockerfile`'s `builder` stage. `chromedriver` is a new stage in the same `Dockerfile` (chromium + chromedriver), not a separate file.

### Fixed
- `Dockerfile` wasn't copying `Trunk.toml` into the build context, so the production image silently used a different (trunk-default) `wasm-opt` version than the one pinned for reproducibility. Verified before/after: the pinned version now downloads inside the image.

### Notes
- Getting the containerized WASM test suite working surfaced a few real gotchas, in case they bite again: `wasm-bindgen-test-runner` needs `WASM_BINDGEN_TEST_ADDRESS` as a literal `ip:port` (not a hostname, and not port `0` -- that literal `0` leaks into the URL the browser navigates to instead of resolving to the OS-assigned port); newer chromedriver rejects requests whose `Host` doesn't look local unless `--allowed-origins` is set; and under rootless Docker, containers live in a separate network namespace from the host shell, so a plain host process and a container can't reach each other via loopback tricks -- both sides of a browser-driven test need to be containers on the same compose network.

### Added
- `wasm-opt -Oz` runs on release builds (`data-wasm-opt="z"` on the `<link data-trunk rel="rust">` tag in `index.html`, tool version pinned via `Trunk.toml`). Measured: shipped `.wasm` shrinks from ~1.1 MB (release, no wasm-opt) to ~0.7 MB.

## [0.8.0] - Project audit follow-up

### Added
- `LICENSE` (Apache-2.0), plus `license`/`description`/`repository` metadata in `Cargo.toml`.
- `rust-toolchain.toml` pinning the Rust toolchain to `1.93.1` (with `wasm32-unknown-unknown`, `rustfmt`, `clippy`), so local dev, Docker and CI all build with the same, reproducible compiler instead of a floating `stable`/`1-slim-bookworm`.

### Fixed
- **Real crash, not just theoretical**: `roxmltree::Document::parse` has no recursion-depth limit and genuinely stack-overflows the whole process on deeply nested XML (confirmed by testing: JSON/YAML/TOML all returned a clean parse error at depth ~50k thanks to their own built-in recursion limits; XML aborted the process). `parsers::xml::parse_xml` now runs a cheap tag-depth pre-scan (`check_xml_depth`, limit 256) before handing the input to roxmltree, rejecting pathologically deep documents with a normal error instead of crashing the tab.
- `convert::to_xml` used `DataNode` keys directly as XML tag/attribute names without validating them; a key with spaces, quotes or a leading digit produced invalid XML. Added `sanitize_xml_name` to replace invalid characters instead.
- Docker base images (`rust:1-slim-bookworm`, `nginx:alpine`) and the CI Rust toolchain (`dtolnay/rust-toolchain@stable`) were floating tags; pinned to `rust:1.93.1-slim-bookworm`, `nginx:1.27-alpine` and `dtolnay/rust-toolchain@1.93.1`.

### Notes
- Audited the project end-to-end for gaps (see conversation); accessibility (keyboard navigation for the graph) was flagged but intentionally left out of this pass.

## [0.7.0] - Milestone 7 — GitHub Actions CI

### Added
- `.github/workflows/ci.yml` with jobs `fmt`/`clippy` (lint), `test` (`cargo test --lib`), `build-wasm` (`trunk build --release`, uploaded as an artifact), and `docker` (`docker build` to validate the `Dockerfile`, not pushed anywhere).
- Cargo build cache via `Swatinem/rust-cache` shared across the clippy/test/build-wasm jobs.

### Fixed
- Applied `cargo fmt` across the codebase and fixed the two `clippy::too_many_arguments` lints on `truncatable_lines`/`render_nodes` (via a targeted `#[allow]`, not worth a bigger refactor right now) so the new lint/test CI jobs start clean instead of red on day one.

### Notes
- A GitLab CI pipeline was set up first and then replaced with this GitHub Actions one at the user's request.
- The `docker` job only builds the image locally in the runner to catch a broken `Dockerfile`; it does not push to any registry (no `docker/login-action`, no credentials needed).
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
