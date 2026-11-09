# Contrôle visuel natif — étape 2.1

## Portée et sources

Contrôle du vrai programme Rust/egui/glow sur macOS, densité native 2×.
Branche de départ : `dev` après fusion de la PR #2 (`3ac770c`).
La capture `docs/screenshots/before-grid.png` précède les modifications.
Les captures `after-*.png` proviennent de la fenêtre native, via la commande
Screenshot d'egui et son événement GPU ; aucune image de façade ne les remplace.

Références fournies et examinées :

- ArtCraft : `codex-clipboard-9c86f6ef-25f2-4818-bce9-f436c71addbe.png`
  (bibliothèque), `codex-clipboard-a46af0b3-3729-45a7-8fff-ca552ea6bb85.png`
  (réglages).
- Studio App : `visuel 01.png` et `visuel 02.png`.
- Les trois captures ArtCraft initiales pour la continuité des fiches.

Les comparaisons côte à côte ont été examinées avec source et capture réelle
sur la même image, ramenées proportionnellement à une largeur de 830 pixels
(sans déformation), avec un recadrage dédié aux bandeaux, bordures et actions.
Les composites locaux sont dans `target/visual-qa/` : `artcraft-grid.png`,
`artcraft-settings.png`, `studio-grid.png`, `studio-settings.png`,
`card-crops.png`. Les images de marque tierces ne sont pas copiées dans Git.

Le contrôle Product Design a été adapté au rendu natif : aucune conversion web,
aucun redesign de marque, aucune génération d'assets remplaçant les monogrammes
existants. Les exigences explicites Rust/egui et identité propre priment.

## Écarts corrigés lors des itérations

1. Anciennes cartes de 360 points et calcul à 320 points : remplacés par des
   cartes jointives de 306 points et une grille de 1 à 5 colonnes, minimum de
   contenu 270 points par colonne. Les actions ont un emplacement réservé.
2. Premier rendu des nouveaux cartons : le corps héritait du layout horizontal
   du parent et débordait. Layout vertical explicite et identifiants propres
   aux sous-zones ; ajout d'un test de clic nom/action distincts.
3. Première barre étroite : le sélecteur dépassait et élargissait les cartes.
   Lignes explicites sous 1 050 points ; test des limites à 440 points.
4. Une hauteur d'interaction de la barre se propageait au contenu des cartes :
   style isolé dans un scope. L'éditeur/date reste au-dessus des boutons.
5. Thème clair : panneaux, contours et textes secondaires des fiches/réglages
   adaptés ; les aperçus conceptuels conservent leur propre rendu de logiciel.
6. Réglages trop hauts : espacement vertical réduit, contenu défilant borné et
   footer fixe ; tous les groupes tiennent sur deux colonnes à grande largeur.
7. Densité des vignettes : logos 54 points (56 au survol) et titres Lato Bold,
   sans changement de monogrammes ni de couleurs du catalogue.
8. États ambiguës : marquage SIMULÉ dans les badges concernés et le footer,
   absence de tailles inventées lorsqu'aucun package n'est approuvé.

## Résultat de la comparaison

- Proportions : bandeau fin, cartes complètes et denses, contours jointifs,
  actions alignées, cinq emplacements sur la largeur de référence. Le rang
  complet de contrôle contient cinq applications réelles du catalogue d'exemple
  ou fictives explicitement indiquées, sans en créer de nouvelles pour l'image.
- Structure : identité/signature/engrenage en haut, actualisation à gauche et
  contrôles alignés à droite ; installées/simulées en premier, disponibles après.
- Couleurs : arrière-plan sombre neutre, accent par application, survol subtil
  avec liseré supérieur ; boutons principaux monochromes.
- Réglages : modal assombrissant la bibliothèque, deux colonnes larges, sections
  bordées, footer Quitter/Terminé ; empilement et défilement sur fenêtre étroite.
- Typographie : noms lisibles et graisse dédiée ; petites métadonnées monospace.
  Ellipses réservées aux noms/descriptions longs avec infobulle, jamais aux actions.
- Interaction native : engrenage, thème clair puis sombre, Échap, grille/liste,
  saisie de recherche, ouverture d'une fiche et fermeture vérifiés à la souris.
- Tests headless : géométrie à 1×/2×, cinq colonnes, petite fenêtre, clics,
  navigation, actions, survol sans effet de bord et préférences persistantes.

Différences intentionnelles : les données ne reprennent pas les versions,
installations ni tailles fictives des maquettes ; les fonctions système non
implémentées sont désactivées. Aucun faux Discord, réseau social, logo animalier
ou illustration ArtCraft. Les anciens aperçus conceptuels sont conservés.
Le launcher reste une application native avec décoration de fenêtre système.

## Limites et verdict

Contrôle visuel macOS validé sur les captures conservées dans
`docs/screenshots/`. Le contenu hors zone visible se consulte par défilement ;
les noms/descriptions peuvent être ellipsés avec infobulle. Les contrôles
d'installation réelle restent volontairement désactivés. La CI macOS/Windows/
Ubuntu couvre code, tests et compilation ; aucune capture native Windows/Linux
n'a été réalisée sur ce poste.

Le résultat respecte les proportions et interactions demandées tout en conservant
l'identité et les données Studio App. Aucun écart substantiel de débordement ou
d'action tronquée ne demeure dans les formats contrôlés (440, 800, 1440, 1664
points et densités 1×/2×).
