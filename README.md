# Studio App

Launcher desktop natif Rust/egui pour une suite de logiciels professionnels.
Cartes applicatives complètes, fiches détaillées et thème sombre, inspirés de
[l’ergonomie ArtCraft](https://github.com/storytold/craft-launcher), avec code et
identité propres. Aucun Electron ni WebView.

## Périmètre et état : étape 3.1

Studio App est le launcher de **notre propre gamme de logiciels professionnels**.
La bibliothèque affiche seulement les applications publiées au catalogue approuvé.
La détection native vérifie ces références ; elle n'inventorie pas l'ordinateur
et n'ajoute jamais de logiciels tiers automatiquement. Les futurs produits seront
référencés avec leurs véritables identifiants et versions, sans valeurs inventées.

Le catalogue est désormais chargé **au runtime** depuis un fichier JSON ou une
URL HTTPS configurée. Validation stricte, cache persistant, repli hors ligne,
images PNG/JPEG et actualisation manuelle alimentent les cartes existantes.
Recherche, tri, navigation et Échap sont conservés. La bibliothèque propose
une grille compacte de cinq colonnes et une liste, avec des réglages natifs
persistants (apparence, langue, présentation et liens communautaires).
Voir le [guide de l’étape 2.1](docs/ETAPE_2_1.md) et ses captures avant/après.

**La détection et l’ouverture natives sont actives sur macOS et Windows pour les
applications munies de règles explicites dans le catalogue.** Le bouton OUVRIR
revérifie l’installation avant de lancer le logiciel. Les applications fictives
conservent leur simulateur. Le [moteur de l’étape 4](docs/ETAPE_4.md) permet désormais une installation réelle
du logiciel de test isolé (ZIP/DMG macOS, ZIP portable Windows), avec HTTPS,
approbation Ed25519, SHA-256 et détection finale. Le déploiement de production,
PKG/MSI/EXE, les mises à jour et désinstallations restent désactivés.
Voir [détection, limites et validation de l’étape 3](docs/ETAPE_3.md).
La [finition graphique 3.1](docs/ETAPE_3_1.md) apporte des logos désaturés au repos,
des teintes discrètes et un survol animé sans déplacement du contenu, en clair/sombre.

Le [processus de distribution 4.1](docs/DISTRIBUTION.md) prépare les nouvelles
versions depuis les dépôts GitHub explicitement enregistrés : proposition de PR,
empreintes et approbation Ed25519 hors ligne avant publication du catalogue.
Le registre de distribution livré est vide et les workflows d’écriture sont
désactivés par défaut ; aucune vraie application n’est publiée.

[Hyper Media Puller](docs/HYPER_MEDIA_PULLER.md) est le premier logiciel réel
du catalogue par défaut. Studio App propose la release compatible et ouvre
l’installateur officiel via sa page GitHub ; l’installation reste manuelle.

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
de recompiler. OBS n'est pas un produit de notre gamme et n'est pas inclus par défaut.
Pour le seul contrôle technique facultatif de détection/ouverture, choisir son catalogue isolé :

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
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --all-targets --all-features
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
Linux. Les PR visent `dev` ; aucune fusion ni release automatique.

## Bibliothèque utilisateur

Par défaut, seules les applications réelles publiées au catalogue sont visibles. Les huit fiches fictives sont conservées pour la QA mais masquées ; **Réglages → Bibliothèque → Afficher les applications de démonstration** permet de les réactiver. La préférence est conservée dans `ui-preferences.json` ; les anciens fichiers de préférences restent compatibles et choisissent le mode réel.

L'emblème de Studio App est défini dans `assets/brand/studio-app.svg` et repris dans l'en-tête. Le logo est également utilisé pour l'icône de fenêtre native. Le DMG macOS embarque un fichier ICNS généré automatiquement. Sous Windows, l'icône de fenêtre est disponible ; l'icône embarquée dans les ressources PE et la signature finale restent à qualifier avant publication.
