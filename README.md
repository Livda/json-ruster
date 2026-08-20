# json-ruster

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](./LICENSE)
[![CI](https://github.com/Livda/json-ruster/actions/workflows/ci.yml/badge.svg)](https://github.com/Livda/json-ruster/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/Livda/json-ruster/main/.github/badges/coverage.json)](https://github.com/Livda/json-ruster/actions/workflows/ci.yml)

> Interactive JSON/YAML/XML/CSV/TOML tree viewer — a Rust/WASM take on JSON Crack, no backend.

A Rust clone of [JSON Crack](https://github.com/AykutSarac/jsoncrack.com): interactive tree visualization of JSON/YAML/XML/CSV/TOML, no backend, compiled entirely to WebAssembly.

## Stack

- [Leptos](https://leptos.dev/) (CSR) for the reactive UI.
- [Trunk](https://trunkrs.dev/) for the WASM build/bundling.
- Graph rendered as native SVG (no canvas, no JS graph library).
- Release builds run through `wasm-opt -Oz` (`data-wasm-opt="z"` in `index.html`, version pinned in `Trunk.toml`), shrinking the shipped `.wasm` from ~1.1 MB to ~0.7 MB.

## Everything runs in containers

No Rust toolchain is required on the host -- `Dockerfile` (`builder` stage) has `rustup`, `trunk` and `wasm-bindgen-cli` pinned and pre-installed, and every workflow below runs through it via `compose.yaml`. `rust-toolchain.toml`, the Docker base images and CI all pin the exact same Rust version (`1.93.1`) for reproducibility.

**Development** (live-reload):

```bash
docker compose --profile dev up dev
```

Then open `http://127.0.0.1:8080`. The source is bind-mounted, so edits on the host are picked up by `trunk serve`'s watcher without rebuilding the image; `target/` also lives on the host and persists across restarts.

**Production image**:

```bash
docker compose up --build
```

Then open `http://127.0.0.1:8080`. This builds the full multi-stage `Dockerfile`: `builder` compiles the app with Trunk (`trunk build --release`, `wasm-opt`-optimized), and the final `runtime` stage is `nginx:1.27-alpine` serving the static output -- no Rust toolchain in that image. `docker build -t json-ruster . && docker run --rm -p 8080:80 json-ruster` works the same way without Compose.

## Tests

Pure logic (parsers, layout, convert, `find_matches`, `control_style`) is tested natively, no browser needed -- run it through the `test` service (same `builder` image as the browser suite below, just a different command):

```bash
docker compose --profile test run --rm --build test cargo test --lib
```

Coverage for that same suite, via `cargo-llvm-cov` (pre-installed in the `builder` stage):

```bash
docker compose --profile test run --rm test cargo llvm-cov --lib --summary-only   # terminal report
docker compose --profile test run --rm test cargo llvm-cov --lib --html           # target/llvm-cov/html
```

DOM-level integration tests (`tests/ui.rs`, mounting `App` and driving it via real events) need a browser, provided by a standalone `chromedriver` + `chromium` service on the same compose network -- see `compose.yaml` for how the test container finds it and exposes itself back (`CHROMEDRIVER_REMOTE`, `WASM_BINDGEN_TEST_ADDRESS`):

```bash
docker compose --profile test up -d chromedriver
docker compose --profile test run --rm test
```

`dev`/`test` also mount a persistent `cargo-home` volume (`$CARGO_HOME`), seeded from the image on first use: installing another dev tool with e.g. `docker compose --profile dev run dev cargo install <tool>` sticks around across container recreations without touching the `Dockerfile`.

## CI

`.github/workflows/ci.yml` runs on GitHub Actions: `fmt`/`clippy` (lint), `cargo test --lib` plus `cargo-llvm-cov` (test + coverage -- a per-file table is written to the run's Summary tab, and `lcov.info` is uploaded as an artifact), `trunk build --release` (build, uploaded as an artifact), then a `docker build` to validate the `Dockerfile` still builds (not pushed anywhere). The browser-driven `tests/ui.rs` suite isn't wired into CI yet (it only runs locally via the `chromedriver`/`test` services) -- it would need chromedriver + chromium on the runner.

## Architecture

- `src/model.rs` — unified internal data representation (`DataNode`), the pivot between all formats.
- `src/parsers/` — one module per input format (JSON, YAML, XML, CSV, TOML) converting to `DataNode`, plus `Format` for dispatch and samples.
- `src/graph.rs` — builds a tree of displayable nodes (`GraphNode`) from a `DataNode`, tracking parents and computing paths (`path_to`).
- `src/layout.rs` — node positioning (simplified Reingold–Tilford-style algorithm), accounting for collapsed nodes and inline-expanded lines.
- `src/convert.rs` — serializes a `DataNode` back to any supported format's text (JSON/YAML/XML/CSV/TOML), inferring numbers/booleans/null from scalar text.
- `src/ui.rs` — Leptos components (UI, editor, SVG rendering, pan/zoom, collapse/expand, selection, conversion, SVG/PNG export, search, theme); part of the library so `tests/ui.rs` can mount and drive it.
- `src/main.rs` — just calls `mount_to_body(ui::App)`.

## Interactions

- **Pan**: click-and-drag on the graph.
- **Zoom**: mouse wheel.
- **Collapse/expand**: click a node with children (`+N`/`-` indicator); the click also selects the node and shows its path at the top of the panel.
- **Long values**: click the `[...]` marker to expand a truncated line in place (`[-]` to collapse it back).
- **Convert**: pick a target format in "Convert to" and click "Convert" to rewrite the editor's content in that format.
- **Export**: "Export SVG"/"Export PNG" (top-right of the graph panel) download the full graph — independent of the current pan/zoom — as a standalone file.
- **Search**: type in the "Search" box to highlight matching nodes (title or any field key/value, case-insensitive) with a gold border; collapsed ancestors of a match are automatically expanded so it stays reachable.
- **Theme**: the moon/sun icon (top-right) toggles the whole UI, including exported SVG/PNG files. Defaults to the OS's `prefers-color-scheme`, then remembers your last choice.
- **Copy**: copies the editor's current content to the clipboard.
- **Share**: copies a link that reopens with the same document and format, encoded in the URL fragment (nothing is sent to a server) as DEFLATE-compressed, base64 text. Refused with an error instead of producing an unusable link if the compressed payload would exceed 8 KB.
- **Fullscreen**: the "⛶" button on the editor or the graph panel (top-right of each) toggles fullscreen for that panel independently.
- **Rotate**: the "↻" button (top-right of the graph panel) turns the tree 90° between the default top-to-bottom layout and a left-to-right one.

The current document, format and theme are saved to `localStorage`, so reloading the page picks up where you left off (a share link, if present in the URL, takes priority).

## Roadmap

See [CHANGELOG.md](./CHANGELOG.md) for detailed progress.

1. **Foundation** — JSON parser, tree layout, static SVG rendering. ✅
2. **Interactivity** — pan/zoom, collapse/expand nodes, selection. ✅
3. **Multi-format** — YAML, XML, CSV, TOML + synced editor. ✅
4. **Conversion & export** — format conversion, SVG export, PNG export. ✅
5. **Stretch** — light/dark theme ✅, search ✅, syntax-highlighting editor (not planned: would need a JS dependency like CodeMirror, out of scope for a pure-Rust/WASM app).
6. **Docker** — multi-stage `Dockerfile` (Trunk build + nginx runtime) for a production image, plus `compose.yaml`. ✅
7. **CI** — GitHub Actions pipeline: lint (fmt/clippy), test, build (Trunk), validate the Docker image builds. ✅

## License

Apache-2.0, see [LICENSE](./LICENSE).
