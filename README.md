# Studio App

Launcher de la suite de logiciels professionnels Studio App.

## Objectif

Application de bureau native développée en Rust. Expérience visuelle et fonctionnelle inspirée d'ArtCraft Launcher : cartes d'applications, fiches détaillées, installation, lancement et mises à jour.

## Étape 1 : interface de démonstration

Fenêtre native Rust/egui, thème sombre, huit applications fictives en cartes
complètes, fiches détaillées avec galerie conceptuelle, recherche par nom ou
catégorie, tri, navigation et six états visuels. Échap ferme la fiche ; les
flèches gauche/droite parcourent les applications visibles.

**Toutes les opérations sont des simulations en mémoire**, explicitement
signalées. Aucun logiciel n’est installé, téléchargé, lancé ou détecté. Les
données fictives sont réinitialisées à chaque lancement. Les étapes 2 à 4
attendent la validation de cette interface.

## Lancer

Installer Rust stable depuis [rustup.rs](https://rustup.rs), puis depuis ce dépôt :

```sh
cargo run --locked
```

Sur le poste de développement actuel, sans Rust global, la chaîne est isolée
dans le dossier parent `.toolchain`. Le raccourci suivant la détecte :

```sh
./scripts/cargo.sh
```

Pour lancer directement le binaire compilé sur macOS : `./target/debug/studio-app`.
Sur Windows : `target\debug\studio-app.exe`. Compilation : `cargo build --locked`.

## Vérifier

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
```

Le raccourci accepte les mêmes arguments, par exemple
`./scripts/cargo.sh test --locked`. Sur Linux, prévoir les bibliothèques de
développement X11/Wayland/OpenGL indiquées dans le workflow CI. Le lancement
graphique demande une session de bureau. Les tests egui fonctionnent sans écran.

Le détail de l’architecture, les références examinées, les scénarios de
vérification et les limites figurent dans [le bilan de l’étape 1](docs/ETAPE_1.md).
Le [cahier des charges](docs/CAHIER_DES_CHARGES.md) décrit la V1 complète.
Les changements se font sur des branches issues de `dev`, avec PR et CI ;
aucune publication sur `main` ni release dans cette étape.

Chaque logiciel reste indépendant, installable et utilisable sans le launcher.

Référence : https://github.com/storytold/craft-launcher — inspiration UX/UI, pas de réutilisation automatique de la marque ou des assets.
