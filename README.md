# json-ruster

Un clone de [JSON Crack](https://github.com/AykutSarac/jsoncrack.com) en Rust : visualisation interactive de JSON/YAML/XML/CSV/TOML sous forme d'arbre, sans backend, entièrement compilé en WebAssembly.

## Stack

- [Leptos](https://leptos.dev/) (CSR) pour l'UI réactive.
- [Trunk](https://trunkrs.dev/) pour le build/bundling WASM.
- Rendu du graphe en SVG natif (pas de canvas, pas de lib JS de graphe).

## Lancer en local

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve
```

Puis ouvrir `http://127.0.0.1:8080` (ou le port indiqué).

## Tests

Les modules purs (parseurs, layout) sont testés hors navigateur :

```bash
cargo test --lib
```

## Architecture

- `src/model.rs` — représentation interne unifiée de la donnée (`DataNode`), pivot entre tous les formats.
- `src/parsers/` — un module par format d'entrée, convertissant vers `DataNode`.
- `src/graph.rs` — construction d'un arbre de nœuds affichables (`GraphNode`) à partir d'un `DataNode`.
- `src/layout.rs` — positionnement des nœuds (algorithme de type Reingold–Tilford simplifié).
- `src/main.rs` — composants Leptos (UI, éditeur, rendu SVG).

## Roadmap

Voir [CHANGELOG.md](./CHANGELOG.md) pour l'avancement détaillé.

1. **Socle** — parser JSON, layout d'arbre, rendu SVG statique. ✅
2. **Interactivité** — pan/zoom, pli/dépli des nœuds, sélection.
3. **Multi-formats** — YAML, XML, CSV, TOML + éditeur synchronisé.
4. **Conversion & export** — conversion entre formats, export SVG/PNG.
5. **Stretch** — thèmes, recherche, éditeur avec coloration syntaxique.
