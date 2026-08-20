# Changelog

Format basé sur [Keep a Changelog](https://keepachangelog.com/en/2.0.0/) v2.0.0.

## [Unreleased]

### Corrigé
- Chevauchement vertical des boîtes quand un nœud a plus de champs que la hauteur de ligne fixe ne le permettait : la hauteur de chaque rangée est maintenant calculée dynamiquement (hauteur du plus grand nœud de la rangée précédente), au lieu d'une constante `LEVEL_HEIGHT`.
- Débordement du texte hors des boîtes pour les valeurs très longues (ex. paragraphes de texte) : les lignes "clé: valeur" et le titre sont tronqués à 48 caractères (`layout::truncate_display` / `field_text`).
- L'infobulle native (`<title>` SVG) ne s'affichait pas au survol (conflit avec la balise HTML `<title>` dans Leptos) : remplacée par un marqueur `[...]` cliquable qui ouvre un panneau affichant le texte complet.

## [0.3.0] - Jalon 3 — Multi-formats

### Ajouté
- Parseurs YAML (`serde_yaml`), XML (`roxmltree`), CSV (`csv`) et TOML (`toml`) vers `DataNode`, avec tests unitaires pour chacun.
- `parsers::Format` : énumération des formats supportés, avec exemple (`sample()`) et dispatch de parsing (`parsers::parse`).
- Sélecteur de format dans l'UI ; changer de format recharge l'éditeur avec un exemple représentatif.
- Affichage des erreurs de parsing (JSON/YAML/XML/CSV/TOML) directement dans le panneau du graphe.

### Notes
- Pas de debounce sur la saisie : le re-parsing à chaque frappe est négligeable pour la taille de documents visée, donc omis pour rester simple (cf. plan initial qui l'envisageait).

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
