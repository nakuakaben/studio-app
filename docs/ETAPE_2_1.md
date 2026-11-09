# Étape 2.1 — bibliothèque et réglages natifs

Branche `feat/artcraft-ui-refinement`, issue de `dev` après la PR #2.
Aucun changement du schéma applicatif, du catalogue livré ou du moteur de cache.
Aucune installation, exécution de logiciel ou mise à jour réelle.

## Bibliothèque

Les cartes jointives mesurent 306 points de haut, avec 1 à 5 colonnes selon
la largeur disponible (minimum visé de 270 points, cinq colonnes dès 1 350
points de contenu). Le bandeau version/source, le statut, la description et
les actions du bas restent présents. Les états locaux affichés dans la démo
portent explicitement « SIMULÉ » ; aucune version locale n'est inventée.
Les packages absents sont indiqués comme tels, sans taille fictive.

Les applications disposant d'une version locale ou simulée sont regroupées
en premier et teintées. Les autres restent neutres ; le survol ajoute une
teinte, un liseré et un léger agrandissement du logo, sans lancer d'opération.
OUVRIR simule une ouverture, y compris lorsqu'une mise à jour est disponible.
METTRE À JOUR/INSTALLER et l'action groupée utilisent le simulateur existant.
Pause, reprise, annulation, erreur et nouvel essai sont conservés.
Le menu … ouvre la fiche ou réinitialise la simulation ; désinstaller est désactivé.

La grille et la liste partagent recherche, tri, état et actions. Les noms longs
et descriptions peuvent être ellipsés avec leur texte complet en infobulle.
Les boutons restent entiers. La barre d'outils utilise une seule ligne à grande
largeur et trois lignes à moins de 1 050 points de contenu. Les fiches et la
galerie, y compris les images chargées au runtime, sont conservées.
La police libre Lato améliore la lisibilité ; sa licence est fournie.

## Réglages fonctionnels

- Apparence : comme le système (avec repli sombre), sombre et clair, appliqués
  immédiatement à la bibliothèque et aux fiches/réglages.
- Langue : automatique et français. Le français est la seule traduction ;
  automatique utilise donc le français en repli. Aucun anglais incomplet.
- Présentation grille/liste et tri, mémorisés après modification.
- Liens communautaires site/Discord configurables en HTTPS. Ils sont vides par
  défaut ; seul le GitHub réel du projet est fourni. Les liens invalides ne
  deviennent jamais cliquables et ne sont jamais enregistrés.
- Terminé enregistre les préférences ; Échap ou un clic sur le fond ferme aussi
  la modale. Une saisie de lien invalide empêche Terminé ; Échap l'abandonne.
- Quitter Studio App demande la fermeture native du programme.

Le fichier `ui-preferences.json` est distinct du catalogue et du cache, dans le
répertoire de configuration utilisateur résolu par `directories::ProjectDirs`
(`app`, `Studio App`, `Studio App`). Pour un test isolé, définir
`STUDIO_APP_PREFERENCES_DIR` vers un dossier local de confiance.

Schéma des préférences, version 1 :

```json
{
  "schema_version": 1,
  "view": "grid",
  "appearance": "system",
  "language": "auto",
  "sort": "name",
  "community_website": null,
  "community_discord": null
}
```

Valeurs : `grid|list`, `system|dark|light`, `auto|french`, `name|category`.
La lecture est limitée à 16 Kio ; champs inconnus, JSON incomplet, version
incompatible et URLs invalides sont rejetés. L'application utilise alors les
valeurs par défaut et affiche un avertissement. L'écriture valide d'abord les
données, synchronise un fichier temporaire voisin puis le remplace atomiquement.
Une erreur d'écriture est signalée, sans modifier le catalogue.

## Réglages désactivés

La fréquence de vérification, les installations/mises à jour automatiques,
les notifications, les versions bêta, le choix du dossier et les boutons
Parcourir/Par défaut, la zone de notification, le bureau courant et le démarrage
de session sont désactivés avec « Disponible dans une prochaine version. »
Le chemin affiché est prévu pour une future étape et aucun dossier d'installation
n'est créé. La version du launcher est informative, issue de Cargo ; aucun
sélecteur ne prétend télécharger une autre version.

## Validation et captures

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --all-targets --all-features
```

29 tests passent sur macOS : 14 tests de domaine/préférences/interface et les
15 tests de catalogue existants. Deux tests de confinement par symlinks sont
spécifiques à Unix. La CI exécute ces commandes sur macOS, Windows et Ubuntu.
Les tests d'interaction injectent de vrais événements pointeur/clavier egui :
actions séparées du clic carte, recherche/tri, navigation, grille/liste,
survol sans effet de bord, réglages, thème clair, Échap et persistance.
La géométrie est vérifiée à 1× et 2×, dont une rangée de cinq cartes et la barre
à 440 points. Le contrôle natif macOS a confirmé ces parcours avec la souris.

Captures du vrai rendu GPU, sans HTML ni reconstruction :

- [Avant](screenshots/before-grid.png) / [Après](screenshots/after-grid.png).
- [Cinq colonnes](screenshots/after-five-columns.png).
- [Réglages](screenshots/after-settings.png).
- [Liste](screenshots/after-list.png), [clair](screenshots/after-light.png).
- [Étroit](screenshots/after-narrow.png), [réglages étroits](screenshots/after-narrow-settings.png).
- [Fiche](screenshots/after-detail.png), [survol](screenshots/after-hover.png).

`examples/capture.rs` ouvre une vraie fenêtre eframe/glow, attend le chargement
puis enregistre `egui::Event::Screenshot`. La fonctionnalité Cargo `visual-qa`
permet de sélectionner l'état capturé sans ajouter de commande au launcher.

```sh
python3 scripts/prepare-visual-qa.py
cargo build --example capture --features visual-qa --locked
STUDIO_APP_CACHE_DIR=target/qa-cache \
STUDIO_APP_PREFERENCES_DIR=target/qa-prefs \
STUDIO_QA_STATE=settings \
./target/debug/examples/capture target/settings.png
```

`STUDIO_QA_STATE` : `settings`, `list`, `light`, `detail`, `hover` ou vide (grille).
`STUDIO_QA_WIDTH`/`STUDIO_QA_HEIGHT` : points logiques, 1664×940 par défaut.
Pour la rangée de cinq : `STUDIO_APP_CONFIG=target/visual-qa/config.json` après
le script. Depuis l'étape 3.1, il utilise seulement les huit démonstrations
existantes, avec les scénarios désactivés pour montrer cinq cartes disponibles
sur la même rangée. Aucun OBS ni nouveau logiciel n'est ajouté au catalogue.
Les captures macOS sont en densité native 2× ; la hauteur du contenu varie de
quelques points avec la décoration de fenêtre et le placement système.

Le rapport de comparaison est dans [design-qa.md](../design-qa.md).
La vérification visuelle native Windows/Linux reste à effectuer sur ces postes ;
la CI y couvre compilation et tests headless, pas une capture du bureau.
