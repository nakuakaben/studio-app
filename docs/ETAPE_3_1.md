# Étape 3.1 — bibliothèque sobre et survol

Branche `feat/library-hover-polish`, depuis `dev` après fusion de la PR #4
(`e308fcc`). Aucun changement du catalogue JSON, des schémas, du cache, des
identifiants natifs ou des workers existants. Aucun installateur ni téléchargement
de package. Les simulations restent explicitement fictives.

## Périmètre produit

Studio App distribue notre propre gamme de logiciels professionnels. La
bibliothèque vient exclusivement des entrées du catalogue approuvé. La détection
vérifie ces références et n'ajoute jamais une application découverte sur le poste.
OBS est uniquement un exemple technique facultatif dans sa configuration isolée ;
il n'est pas inclus dans le catalogue par défaut. Les huit produits actuels sont
des démonstrations fictives. Les futurs produits réels devront fournir leurs
identifiants et versions vérifiés ; aucune valeur n'est inventée ici.

Cette règle figure aussi dans `AGENTS.md`, le cahier des charges, le README et
le guide du catalogue. Le script de QA utilise désormais les huit démonstrations
existantes sans scénarios, pour montrer cinq cartes disponibles, sans y ajouter OBS.

## Traitement visuel

- Non installées : fond neutre, logo/monogramme en niveaux de gris, version neutre.
  Au survol, retour progressif à la couleur, avec seulement 1,8 % de teinte du fond.
- Installées : identité colorée conservée sur le logo, fond limité à 3,5 % de
  mélange avec la couleur d'identité. Le survol ajoute 1,8 % au maximum.
- Liseré : progression de gauche à droite sur 220 ms, puis rétraction douce.
- Logo : agrandissement de dessin de 4 %, autour du même centre. Emplacement
  alloué de 54 points en grille et 42 en liste **constant**. Aucun zoom du carton,
  texte, bouton ou badge. Le débord visuel reste dans les marges entourant le logo.
- Grille et liste utilisent les mêmes paramètres centralisés dans
  `src/ui/theme/hover.rs`. Les en-têtes de fiches et réglages gardent leurs couleurs.
- Les petits textes d'identité sont ajustés vers le clair/sombre pour atteindre
  un contraste de 4,5:1 contre le fond neutre, notamment pour le jaune en clair.

`egui::Context::animate_bool_with_time` conserve un état identifié par ID stable
d'application et par vue ; la position dans la liste ne sert pas d'identité.
Une interpolation smoothstep est appliquée au progrès linéaire : ni rebond ni
dépassement. egui demande les repeints nécessaires, sans boucle d'attente UI.
Les variantes grises des logos PNG/JPEG sont préparées une fois à leur réception,
puis conservées comme textures et mélangées au dessin ; pas de redécodage à chaque
survol. Elles sont libérées avec les images lors du changement de catalogue.
Leur mémoire GPU supplémentaire est bornée par le budget existant des images
décodées (au plus 64 Mio supplémentaires si toutes les ressources sont des logos).

## Réduction des animations

Lecture au démarrage dans un petit traitement de préférences visuelles séparé :

- macOS : requête fixe, en lecture seule, `/usr/bin/defaults read
  com.apple.universalaccess reduceMotion`, sans shell ni entrée de catalogue.
  Le helper est arrêté/récolté après deux secondes si nécessaire.
- Windows : lecture seule de `HKCU\Control Panel\Desktop\WindowMetrics\MinAnimate` ;
  la valeur `0` désactive les transitions du launcher. Aucun paramètre système
  n'est écrit. Il s'agit d'une prise en compte de cette préférence disponible,
  pas d'une intégration exhaustive de toutes les API d'accessibilité Windows.
- `STUDIO_APP_REDUCE_MOTION=1` force le mode réduit sur tous les OS, notamment
  lorsqu'une préférence n'est pas exposée par le backend.

Pendant la lecture initiale, aucun mouvement. En mode réduit, le liseré et les
couleurs changent directement et le logo ne grossit pas. Une préférence absente
ou illisible laisse les transitions normales ; Linux n'a pas de détecteur
système à cette étape. La préférence système est relue au redémarrage, sans
abonnement aux changements en cours de session. Les tests vérifient le mode réduit
sans modifier les réglages d'accessibilité du poste.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --all-targets --all-features
```

49 tests locaux macOS réussis : 34 bibliothèque + 15 catalogue. Les nouveaux
contrôles vérifient les états intermédiaires du liseré, sa progression/rétraction
sans dépassement, l'indépendance des IDs, la taille/position du logo, les positions
strictement constantes des textes/actions dans les deux vues et thèmes, les fonds
neutres, les accents lisibles et la réduction du mouvement. Le chargement de la
texture grise et l'impossibilité d'ajouter une entrée depuis un résultat natif
non référencé sont aussi couverts. Les tests natifs et catalogue existants passent.

Contrôle avec souris/clavier sur l'application native macOS : passage grille/liste,
ouverture des réglages, changement vers le thème clair, fermeture par Échap,
ouverture de la fiche Folio depuis la liste, retour par Échap. Aucun logiciel
tiers n'a été ajouté. Les captures GPU confirment le maintien de cinq colonnes,
des fiches et des réglages. Windows/Linux sont vérifiés par la CI, sans prétendre
à une capture graphique sur ces postes. La détection et le lancement natifs ne
sont pas modifiés ; leurs contrôles Windows/macOS de l'étape 3 sont conservés.

## Captures natives avant/après

Rendu réel eframe/Glow sur macOS, aucune reconstruction HTML. Fenêtre de 1664 ×
940 points demandée ; images en densité 2×. La décoration et le placement macOS
peuvent faire varier légèrement la hauteur utile. Le prototype affiche les mêmes
scénarios ; la progression simulée d'Onde varie selon l'instant de capture.

- Sombre : [avant](screenshots/polish/before-dark.png), [après](screenshots/polish/after-dark.png), [comparaison](screenshots/polish/compare-dark.png).
- Clair : [avant](screenshots/polish/before-light.png), [après](screenshots/polish/after-light.png), [comparaison](screenshots/polish/compare-light.png).
- [Cinq colonnes et survol sombre](screenshots/polish/after-five-hover.png), [clair](screenshots/polish/after-five-light-hover.png).
- Liste [sombre](screenshots/polish/after-list.png) / [claire](screenshots/polish/after-light-list.png).
- [Fiche](screenshots/polish/after-detail.png), [réglages](screenshots/polish/after-settings.png).
- [Vidéo native du survol, 3,33 s](screenshots/polish/hover-native.mp4), [six instants du mouvement](screenshots/polish/hover-sequence.png).

La vidéo vient des événements Screenshot du vrai framebuffer GPU, à environ
30 échantillons/s. Le harness injecte seulement les entrées/sorties du pointeur ;
aucun liseré ou zoom n'est dessiné par un outil vidéo. Les timestamps réels sont
conservés dans une liste concat ; FFmpeg encode en H.264/30 fps, sans accélération.
Les frames intermédiaires restent dans `target`, seul le MP4 compact est versionné.

Reproduction de la grille cinq colonnes :

```sh
python3 scripts/prepare-visual-qa.py
cargo build --locked --example capture --features visual-qa
STUDIO_APP_CONFIG="$PWD/target/visual-qa/config.json" \
STUDIO_APP_CACHE_DIR=target/qa-cache STUDIO_APP_PREFERENCES_DIR=target/qa-prefs \
STUDIO_QA_STATE=hover ./target/debug/examples/capture target/five-hover.png
```

Ajouter `light` à l'état pour le clair ; `list`, `detail` et `settings` sélectionnent
les autres vues. Pour le film : même configuration, `STUDIO_QA_STATE=dark`,
`STUDIO_QA_WIDTH=900 STUDIO_QA_HEIGHT=700 STUDIO_QA_VIDEO=target/hover-frames`.
Attendre la fermeture du harness et l'écriture de `frames.txt`, puis :

```sh
ffmpeg -f concat -safe 0 -i target/hover-frames/frames.txt \
  -vf 'fps=30,scale=900:-2' -c:v libx264 -crf 19 -pix_fmt yuv420p \
  -movflags +faststart target/hover-native.mp4
```

Le mode vidéo est un outil de développement gourmand en mémoire (environ 1 Gio
pour 100 frames 1800 × 1400), jamais activé dans le launcher. La QA visuelle et
ses limites sont consignées dans [design-qa.md](../design-qa.md).
