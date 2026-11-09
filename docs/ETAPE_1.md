# Étape 1 — interface native

## Analyse préalable

Base : `dev`, commit `5350cebc5ce20224a135333c409027da531ad9ef`.
`AGENTS.md` et `docs/CAHIER_DES_CHARGES.md` lus avant modification.
La base contenait une fenêtre vide et uniquement `eframe = "0.33"`.
Cette famille est conservée, sans suivre la migration 0.37 du projet de référence.
Le lockfile fixe la résolution. Eframe utilise Glow et les polices embarquées ;
aucun runtime web, aucune dépendance directe supplémentaire.

Architecture publique examinée le 9 novembre 2026 :
[ArtCraft Launcher](https://github.com/storytold/craft-launcher), notamment
`crates/core/src/lib.rs`, `crates/ui-egui/src/lib.rs`, `crates/native/src/lib.rs`
et le manifeste racine. Les mécanismes utiles sont la séparation des données
pures et de la présentation, la représentation explicite des états et le futur
traitement des opérations lentes hors du thread graphique. Le catalogue, les
installateurs et les mises à jour de cette référence ne sont pas réutilisés.

Les trois captures fournies ont guidé les proportions : cartes rectangulaires
jointives, bandeau de version, identité colorée, badge d’état et action en bas ;
grande fiche superposée avec bandeau teinté, galerie et sections numérotées.
Les références ne sont pas intégrées au dépôt. Monogrammes et aperçus conceptuels
sont dessinés par notre propre code. Les applications sont entièrement fictives.

## Organisation

- `src/main.rs` : fenêtre native et dimensions minimales.
- `src/lib.rs` : frontière de bibliothèque, interdiction du code unsafe local.
- `src/domain.rs` : données applicatives, fonctionnalités, états typés.
- `src/catalog.rs` : huit applications fictives embarquées.
- `src/demo.rs` : transitions en mémoire, sans accès au réseau ni au système.
- `src/ui/mod.rs` : bibliothèque, recherche, tri, fiches et navigation.
- `src/ui/theme.rs` : couleurs, typographie, cadres.
- `src/ui/gallery.rs` : galerie vectorielle originale à trois vues.
- `scripts/cargo.sh` : commandes Cargo avec chaîne standard ou locale isolée.

Le futur moteur sera séparé du simulateur : catalogue distant et validation,
versions, détection, téléchargements, installation/désinstallation et mise à
jour du launcher devront rester des responsabilités distinctes. Aucun faux
adaptateur système n’est présenté comme implémenté dans cette étape.

## Parcours de vérification manuelle

1. Lancer `cargo run --locked` ; vérifier les deux groupes de cartes.
2. Rechercher par nom ou catégorie ; essayer une recherche sans résultat et
   son bouton d’effacement. Changer le tri.
3. Cliquer sur le fond d’une carte ou son titre : la fiche s’ouvre.
4. Parcourir les applications par les boutons précédent/suivant ou les flèches
   du clavier. Cliquer sur les miniatures de la galerie, puis ses flèches.
5. Fermer la fiche par la croix, Échap ou le fond assombri.
6. Tester « Simuler la mise à jour » sur Sequence, la pause/reprise et
   l’annulation sur Prisme, et la reprise après erreur sur Lueur.
7. Attendre la fin d’un parcours fictif : la version simulée est actualisée.
8. Tester l’ouverture simulée : un message confirme qu’aucun programme n’a été
   lancé. Le même message est visible dans la fiche.
9. Réduire la fenêtre : une colonne de cartes, fiche empilée et défilement.

Les états reviennent au catalogue de départ à chaque lancement. Onde illustre
l’installation fictive au démarrage, puis passe à l’état installé. Les versions,
tailles, fonctionnalités et nouveautés sont inventées pour le prototype. Les
liens officiels sont désactivés avec explication, aucune destination fictive.

## Vérifications automatisées

`cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked` et `cargo build --locked`.
Les tests couvrent pause, annulation, reprise et fin d’une mise à jour ; reprise
après erreur ; recherche et navigation ; rendu egui à 440, 800 et 1440 points ;
Échap ; clics réels injectés dans egui pour les boutons et la fiche.
La CI exécute compilation, Clippy et tests sur macOS, Windows et Linux.

## Limites et suite

Cette livraison couvre uniquement l’étape 1. Aucune détection réelle, aucun
téléchargement, installateur, lancement, désinstallation, cache, catalogue
distant, mise à jour du launcher, packaging de distribution, signature ou
release. Les étapes 2 à 4 attendent la validation de cette interface.
