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

## Tests

Pure modules (parsers, layout) are tested outside the browser:

```bash
cargo test --lib
```

## Architecture

- `src/model.rs` — unified internal data representation (`DataNode`), the pivot between all formats.
- `src/parsers/` — one module per input format (JSON, YAML, XML, CSV, TOML) converting to `DataNode`, plus `Format` for dispatch and samples.
- `src/graph.rs` — builds a tree of displayable nodes (`GraphNode`) from a `DataNode`, tracking parents and computing paths (`path_to`).
- `src/layout.rs` — node positioning (simplified Reingold–Tilford-style algorithm), accounting for collapsed nodes and inline-expanded lines.
- `src/main.rs` — Leptos components (UI, editor, SVG rendering, pan/zoom, collapse/expand, selection).

## Interactions

- **Pan**: click-and-drag on the graph.
- **Zoom**: mouse wheel.
- **Collapse/expand**: click a node with children (`+N`/`-` indicator); the click also selects the node and shows its path at the top of the panel.
- **Long values**: click the `[...]` marker to expand a truncated line in place (`[-]` to collapse it back).

## Roadmap

See [CHANGELOG.md](./CHANGELOG.md) for detailed progress.

1. **Foundation** — JSON parser, tree layout, static SVG rendering. ✅
2. **Interactivity** — pan/zoom, collapse/expand nodes, selection. ✅
3. **Multi-format** — YAML, XML, CSV, TOML + synced editor. ✅
4. **Conversion & export** — format conversion, SVG/PNG export.
5. **Stretch** — themes, search, syntax-highlighting editor.
