# Changelog

Format basé sur [Keep a Changelog](https://keepachangelog.com/en/2.0.0/) v2.0.0.

## [Unreleased]

## [0.2.0] - Jalon 2 — Interactivité

### Ajouté
- Pan (glisser) et zoom (molette) sur le graphe via transform CSS, écouteurs `pointerdown`/`pointermove`/`pointerup`/`wheel`.
- Pli/dépli d'un sous-arbre au clic sur un nœud, avec indicateur (`+N` replié / `-` déplié) ; le layout ne calcule plus que les nœuds visibles.
- Sélection de nœud (contour surligné) avec affichage du chemin (ex. `root.tags[0]`) dans un bandeau au-dessus du graphe.
- `GraphNode::parent` et `Graph::path_to` pour reconstruire le chemin d'un nœud jusqu'à la racine.
- Tests unitaires pour `path_to` et pour le layout avec nœuds repliés.

## [0.1.0] - Jalon 1 — Socle

### Ajouté
- Scaffold du projet Leptos (CSR) + Trunk, ciblant `wasm32-unknown-unknown`.
- `model::DataNode` : représentation interne unifiée (Object/Array/Scalar/Null).
- Parseur JSON (`serde_json` → `DataNode`).
- `graph::build_graph` : construction d'un arbre de nœuds affichables à partir d'un `DataNode`.
- `layout::layout` : positionnement des nœuds via un algorithme de type Reingold–Tilford simplifié (pas de chevauchement entre sous-arbres).
- Rendu SVG des nœuds/arêtes dans un composant Leptos, avec éditeur (textarea) synchronisé en temps réel.
- Tests unitaires pour les parseurs et le layout, exécutables via `cargo test --lib`.
