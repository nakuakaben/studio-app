# Studio App

Launcher desktop natif Rust/egui pour une suite de logiciels professionnels.
Cartes applicatives complètes, fiches détaillées et thème sombre, inspirés de
[l’ergonomie ArtCraft](https://github.com/storytold/craft-launcher), avec code et
identité propres. Aucun Electron ni WebView.

## État : étape 2

Le catalogue est désormais chargé **au runtime** depuis un fichier JSON ou une
URL HTTPS configurée. Validation stricte, cache persistant, repli hors ligne,
images PNG/JPEG et actualisation manuelle alimentent les cartes existantes.
Recherche, tri, navigation et Échap sont conservés.

**Les opérations d’installation et d’ouverture restent des simulations en
mémoire.** Le catalogue et GitHub ne déclarent aucune installation locale.
Aucun package n’est téléchargé, installé ou exécuté.

## Lancer

Installer Rust stable depuis [rustup.rs](https://rustup.rs), puis depuis le dépôt :

```sh
cargo run --locked
```

Sur le poste de développement actuel, la chaîne Rust isolée du dossier parent
`.toolchain` est détectée par :

```sh
./scripts/cargo.sh
```

Le catalogue JSON de démonstration est fourni dans `catalog/catalog.json`.
Modifier ce fichier puis cliquer sur **Actualiser le catalogue** : aucun besoin
de recompiler. Pour essayer le catalogue réel OBS Studio, sans installateur :

```sh
STUDIO_APP_CONFIG="$PWD/catalog/obs-config.json" ./scripts/cargo.sh
```

Pour l’exemple d’image locale, choisir `catalog/assets-config.json` de la même
façon. Le [guide du catalogue](docs/CATALOGUE.md) décrit la configuration HTTPS,
le schéma, le cache, la sécurité et l’ajout d’une application.

Binaire compilé : `target/debug/studio-app` sur macOS/Linux,
`target\debug\studio-app.exe` sur Windows. Aucun packaging de distribution dans
cette étape. Chaque logiciel cible restera indépendant du launcher.

## Vérifier

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
cargo run --locked -- --check-catalog catalog/obs-config.json
cargo run --locked -- --discover obsproject/obs-studio
```

Le raccourci accepte les mêmes arguments : `./scripts/cargo.sh test --locked`.
La commande `--discover` effectue uniquement une lecture des métadonnées GitHub,
sans ajouter ni installer de package. Les tests automatisés sont hors ligne.
Sous Linux, installer les bibliothèques X11/Wayland/OpenGL indiquées dans le
workflow CI. Seul le lancement graphique nécessite une session de bureau.

Documentation : [cahier des charges](docs/CAHIER_DES_CHARGES.md),
[bilan étape 1](docs/ETAPE_1.md), [bilan étape 2](docs/ETAPE_2.md),
[manifeste et ajout d’applications](docs/CATALOGUE.md).
Développement sur des branches issues de `dev`, PR vers `dev`, CI macOS/Windows/
Linux. L’étape 3, les fusions et les releases nécessitent des instructions séparées.
