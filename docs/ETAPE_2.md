# Étape 2 — catalogue dynamique

Précision produit de l'étape 3.1 : OBS cité ci-dessous est uniquement un exemple
technique facultatif. Studio App distribue notre gamme et n'ajoute jamais
automatiquement les logiciels tiers du poste. Voir le cahier des charges à jour.

Branche : `feat/dynamic-catalog`, issue de `dev` au commit
`702ef4ce9371525e30674d09f604876897bb20a8` (fusion de l’étape 1).
Les consignes `AGENTS.md`, le cahier des charges et le bilan de l’étape 1 ont été
relus avant modification. L’interface existante et ses proportions sont conservées.

## Modifications

- Catalogue codé en Rust remplacé par des fichiers JSON lus au runtime.
- Métadonnées détenues en `String`, distinctes de l’état local et des opérations.
- Schéma 1 documenté avec validation stricte et limites de ressources.
- Source locale ou HTTPS configurable, cache atomique et repli hors ligne.
- Chargement hors thread graphique, actualisation manuelle, indication de source,
  cache, indisponibilité et erreurs ; rapprochement des applications par ID.
- Logos et captures PNG/JPEG, chemins confinés, hôtes explicitement autorisés,
  cache d’images et repli graphique. Présentation longue, éditeur, liens,
  plateformes, version disponible et historique alimentés par le manifeste.
- Adaptateur GitHub Releases en lecture seule, types de découverte distincts des
  packages approuvés. Commandes de vérification et de découverte facultatives.
- Exemples : huit logiciels fictifs, OBS Studio réel sans package installable,
  mire d’image locale, fixtures valides et invalides.

Le code du catalogue est dans `src/catalog/{model,validation,security,loader,
cache,assets,github,worker}.rs` et `src/catalog/mod.rs` ; l’ancien `src/catalog.rs`
a été retiré. `src/ui/catalog_state.rs` branche le worker sur les composants de
l’étape 1. `src/domain.rs` et `src/demo.rs` séparent désormais les versions du
catalogue, les états locaux et le simulateur.

Dépendances directes ajoutées : serde/serde_json (modèle JSON), semver (versions),
url/reqwest avec rustls (HTTPS), image PNG/JPEG (décodage borné), sha2 (clés de
cache), tempfile (remplacement atomique), directories (dossiers multiplateformes),
time (dates réelles). Eframe 0.33 et Glow sont conservés. Le lockfile est mis à jour.

## Vérifications

- `cargo fmt --all -- --check`.
- `cargo clippy --locked --all-targets -- -D warnings`.
- `cargo test --locked` : 22 tests locaux sur macOS (7 UI/simulation et 15 catalogue).
  Deux tests de liens symboliques sont spécifiques à Unix ; Windows exécute les
  autres tests. Aucun test automatisé ne dépend d’Internet.
- `cargo build --locked` : binaire natif macOS compilé.
- Lecture CLI des trois configurations d’exemple.
- Chargement HTTPS réel d’un manifeste vide de test via un endpoint public de
  décodage Base64, avec résultat `Remote` et écriture du cache ; aucune donnée
  privée transmise. Les scénarios de repli hors ligne restent déterministes.
- Lecture HTTPS réelle de l’API GitHub OBS : 20 publications découvertes, aucun
  asset téléchargé ; cette vérification manuelle est indépendante de la CI.
- Contrôle visuel de la fenêtre native : bibliothèque dynamique et grande fiche
  conservées. Les tests de rendu incluent des images chargées au runtime et des
  dimensions de 440, 800 et 1440 points.
- CI existante : formatage, Clippy, tests et compilation sur macOS, Windows et
  Linux ; le workflow ne crée aucune release.

Les tests couvrent ajout/suppression, évolution de l’ordre, sélection d’une fiche
supprimée, erreurs réseau/HTTP simulées, cache sur disque, isolation des sources,
cache corrompu, erreur d’écriture, manifeste invalide, schéma incompatible,
empreintes et plateformes, URLs privées et schémas interdits, traversées et
symlinks, limites d’images, images corrompues, cache d’images, worker asynchrone
et absence de promotion automatique des publications GitHub.

## Utiliser et étendre

Voir [CATALOGUE.md](CATALOGUE.md) pour les champs, exemples, commandes, chemins,
limites et procédure d’ajout sans compilation. Le README contient les commandes
de lancement. Les simulations restent explicitement indiquées ; la détection,
l’installation et la mise à jour réelles ne sont pas implémentées.

## Limites assumées

- Schéma 1 uniquement ; aucun mécanisme de migration automatique d’un futur schéma.
- HTTPS public, sans redirection ni proxy ; sources privées et authentification
  non prises en charge. La configuration locale reste le point d’approbation.
- Cache validé, mais pas de signature cryptographique du manifeste, d’expiration
  imposée hors ligne ni de purge globale des autres configurations.
- PNG/JPEG uniquement, chargement progressif et borné ; pas de SVG ni d’animation.
- Découverte GitHub accessible via API Rust/CLI, sans synchronisation automatique
  des versions approuvées ni interface d’approbation de packages.
- Aucune détection réelle, et aucune persistance d’installations simulées.
- Aucun package n’est téléchargé ou exécuté. Aucune étape 3, fusion ou release.
