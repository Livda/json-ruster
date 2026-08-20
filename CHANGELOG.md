# Changelog

Format basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).

## [Unreleased]

## [0.1.0] - Jalon 1 — Socle

### Ajouté
- Scaffold du projet Leptos (CSR) + Trunk, ciblant `wasm32-unknown-unknown`.
- `model::DataNode` : représentation interne unifiée (Object/Array/Scalar/Null).
- Parseur JSON (`serde_json` → `DataNode`).
- `graph::build_graph` : construction d'un arbre de nœuds affichables à partir d'un `DataNode`.
- `layout::layout` : positionnement des nœuds via un algorithme de type Reingold–Tilford simplifié (pas de chevauchement entre sous-arbres).
- Rendu SVG des nœuds/arêtes dans un composant Leptos, avec éditeur (textarea) synchronisé en temps réel.
- Tests unitaires pour les parseurs et le layout, exécutables via `cargo test --lib`.
