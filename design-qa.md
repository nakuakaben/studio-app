# Contrôle visuel natif — étape 3.1

## Sources et cadrage

Cible : les captures Studio App fournies pour cette étape (grilles claire/sombre,
liste et réglages), avec les ajustements de survol explicitement décrits dans la
demande. Elles prolongent les proportions ArtCraft déjà validées. Monogrammes,
polices et aperçus restent ceux de Studio App : aucun remplacement de marque.

Source reproductible : docs/screenshots/polish/before-dark.png et before-light.png,
capturées sur le binaire natif de dev après PR #4, avant modification.
Implémentation : after-dark.png et after-light.png du même dossier.
Toutes quatre : 3328 × 1834 pixels, densité 2×, fenêtre demandée 1664 × 940 points
(contenu GPU 1664 × 917 après décoration native). Même catalogue/scénarios,
sauf progression temporelle simulée d'Onde.

Les sources et implémentations ont été réunies et examinées dans les mêmes
images, à une largeur proportionnelle identique de 1000 pixels par vue :
[sombre](docs/screenshots/polish/compare-dark.png),
[clair](docs/screenshots/polish/compare-light.png).
La [comparaison ciblée](docs/screenshots/polish/compare-card.png) juxtapose les
recadrages natifs de carte à la même échelle pour lire logo, texte, badge et actions.

Autres états ouverts et examinés :
- after-five-hover.png / after-five-light-hover.png : 3328 × 1834, huit
  démonstrations sans scénario, cinq colonnes et Relief survolé.
- after-list.png / after-light-list.png : 3328 × 1844, liste, hauteur utile
  légèrement différente avec la décoration macOS.
- after-detail.png / after-settings.png : 3328 × 1834, fiche/réglages.
- hover-native.mp4 : vrai framebuffer 1800 × 1400 (900 × 700 points),
  encodé en 900 × 700/30 fps, 3,33 s, selon les timestamps réels.
  Les [six instants](docs/screenshots/polish/hover-sequence.png) réunissent
  repos, progression du liseré/couleur et retour au repos à la même échelle.

## Historique des corrections et nouvelle comparaison

1. **[P1, corrigé] Texte déplacé au survol.** Le logo réservait 54 puis 56 points
   au layout. Emplacement désormais constant et dessin seul à 104 %, centré.
   Le film final montre titre/catégorie immobiles ; les tests suivent aussi les
   positions exactes des textes/actions sur vingt frames, dans les deux thèmes
   et vues. Aucun zoom de carte ni changement de géométrie.
2. **[P2, corrigé] Trop de couleurs au repos.** Vignettes disponibles vivement
   colorées et fonds installés trop marqués dans la source. Après : gris neutres
   et aplats réduits. Le film et les rangées de cinq montrent une seule vignette
   colorée au survol et une teinte de carton presque imperceptible.
3. **[P2, corrigé] Versions jaunes/cyan peu lisibles en clair.** Petits accents
   ajustés et versions non installées neutres au repos. La comparaison claire
   montre les petites métadonnées installées assombries. Contraste 4,5:1 testé
   contre le fond neutre.
4. La première preuve vidéo visait le dessus des cartes car la barre se replie
   à 900 points. Après observation du framebuffer, point de survol corrigé et
   capture refaite. Le film final montre effectivement le mouvement natif.

Après corrections : nouvelles captures et comparaisons. Aucun P0/P1/P2 restant
dans le périmètre demandé.

## Cinq surfaces de fidélité

- **Polices/typographie** : Lato/Lato Bold et monospace conservés, mêmes graisses,
  retours, ellipses et alignements sur les recadrages lisibles. Le texte n'est
  pas agrandi avec le logo.
- **Espacement/rythme** : cartes complètes de 306 points, marges, bandeau de
  version et actions conservés. Cinq colonnes maintenues. Liste conservée ;
  son bas défile comme auparavant sans cacher les contrôles persistants.
- **Couleurs/tokens** : repos neutre, teinte installée réduite, liseré 220 ms et
  accents lisibles dans les deux thèmes. Boutons monochromes au même emplacement.
- **Images/qualité** : monogrammes originaux conservés, aucun asset ArtCraft
  inventé. Logos PNG/JPEG conservés en couleur dans la fiche et en variante
  grise en bibliothèque ; aucune substitution ni déformation. Zoom de 4 %
  limité au logo, sans débord sur le texte.
- **Texte produit/contenu** : noms, versions, descriptions, états et menus
  conservés. Simulations explicites. Documentation clarifiant notre gamme,
  l'absence d'ajout automatique de logiciels tiers et l'exemple OBS facultatif.

## Interactions, limites et verdict

Sur l'application native macOS avec souris/clavier : grille vers liste,
réglages, thème clair, Échap, ouverture de Folio depuis la liste, retour.
Les captures GPU du harness vérifient les autres états sans façade HTML.
Tests existants : recherche, tri, navigation, actions, préférences, états natifs,
rendu étroit/HiDPI. Nouveaux tests : mouvement réduit, progression/rétraction,
neutralité, contrastes, géométrie immobile et absence d'ajout hors catalogue.

Pas de capture native Windows/Linux ici : CI pour compilation, Clippy et tests.
La préférence système best-effort de mouvement est lue au démarrage ; aucun
réglage d'accessibilité du poste n'a été modifié pour la QA.
Voir les limites de [l'étape 3.1](docs/ETAPE_3_1.md).

Checklist terminée : cinq colonnes, deux thèmes, liste, fiches/réglages,
liseré gauche→droite, logo seul animé, retour doux, détection/lancement inchangés.
Aucun P3 nécessitant une modification supplémentaire retenu dans ce périmètre.

final result: passed
