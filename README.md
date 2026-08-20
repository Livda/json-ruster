# json-ruster

> Interactive JSON/YAML/XML/CSV/TOML tree viewer — a Rust/WASM take on JSON Crack, no backend.

A Rust clone of [JSON Crack](https://github.com/AykutSarac/jsoncrack.com): interactive tree visualization of JSON/YAML/XML/CSV/TOML, no backend, compiled entirely to WebAssembly.

## Stack

- [Leptos](https://leptos.dev/) (CSR) for the reactive UI.
- [Trunk](https://trunkrs.dev/) for the WASM build/bundling.
- Graph rendered as native SVG (no canvas, no JS graph library).

## Running locally

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve
```

Then open `http://127.0.0.1:8080` (or whichever port is shown).

## Running with Docker

```bash
docker build -t json-ruster .
docker run --rm -p 8080:80 json-ruster
```

Or with Docker Compose:

```bash
docker compose up --build
```

Then open `http://127.0.0.1:8080`. The image is a multi-stage build: a `rust:slim` stage compiles the app with Trunk (`trunk build --release`), and the runtime stage is `nginx:alpine` serving the static `dist/` output — no Rust toolchain in the final image.

## Tests

Pure modules (parsers, layout) are tested outside the browser:

```bash
cargo test --lib
```

## CI

`.github/workflows/ci.yml` runs on GitHub Actions: `fmt`/`clippy` (lint), `cargo test --lib` (test), `trunk build --release` (build, uploaded as an artifact), then `docker build`/`push` to GHCR (`ghcr.io/<owner>/<repo>`) on pushes to `main` (uses the repo's built-in `GITHUB_TOKEN`, no extra setup needed).

## Architecture

- `src/model.rs` — unified internal data representation (`DataNode`), the pivot between all formats.
- `src/parsers/` — one module per input format (JSON, YAML, XML, CSV, TOML) converting to `DataNode`, plus `Format` for dispatch and samples.
- `src/graph.rs` — builds a tree of displayable nodes (`GraphNode`) from a `DataNode`, tracking parents and computing paths (`path_to`).
- `src/layout.rs` — node positioning (simplified Reingold–Tilford-style algorithm), accounting for collapsed nodes and inline-expanded lines.
- `src/convert.rs` — serializes a `DataNode` back to any supported format's text (JSON/YAML/XML/CSV/TOML), inferring numbers/booleans/null from scalar text.
- `src/main.rs` — Leptos components (UI, editor, SVG rendering, pan/zoom, collapse/expand, selection, conversion, SVG/PNG export, search, theme).

## Interactions

- **Pan**: click-and-drag on the graph.
- **Zoom**: mouse wheel.
- **Collapse/expand**: click a node with children (`+N`/`-` indicator); the click also selects the node and shows its path at the top of the panel.
- **Long values**: click the `[...]` marker to expand a truncated line in place (`[-]` to collapse it back).
- **Convert**: pick a target format in "Convert to" and click "Convert" to rewrite the editor's content in that format.
- **Export**: "Export SVG"/"Export PNG" (top-right of the graph panel) download the full graph — independent of the current pan/zoom — as a standalone file.
- **Search**: type in the "Search" box to highlight matching nodes (title or any field key/value, case-insensitive) with a gold border; collapsed ancestors of a match are automatically expanded so it stays reachable.
- **Theme**: the moon/sun icon (top-right) toggles the whole UI, including exported SVG/PNG files.

## Roadmap

See [CHANGELOG.md](./CHANGELOG.md) for detailed progress.

1. **Foundation** — JSON parser, tree layout, static SVG rendering. ✅
2. **Interactivity** — pan/zoom, collapse/expand nodes, selection. ✅
3. **Multi-format** — YAML, XML, CSV, TOML + synced editor. ✅
4. **Conversion & export** — format conversion, SVG export, PNG export. ✅
5. **Stretch** — light/dark theme ✅, search ✅, syntax-highlighting editor (not planned: would need a JS dependency like CodeMirror, out of scope for a pure-Rust/WASM app).
6. **Docker** — multi-stage `Dockerfile` (Trunk build + nginx runtime) for a production image, plus `compose.yaml`. ✅
7. **CI** — GitHub Actions pipeline: lint (fmt/clippy), test, build (Trunk), build & push a production Docker image to GHCR. ✅
