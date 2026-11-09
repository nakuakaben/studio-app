# Studio App — cahier des charges V1

Statut : cadrage initial. Référence fonctionnelle et ergonomique : [ArtCraft Launcher](https://github.com/storytold/craft-launcher).

## Vision
Launcher natif minimaliste pour une suite de logiciels professionnels. Inspiration très proche des captures ArtCraft fournies pendant le cadrage : **cartes applicatives complètes**, et non simples icônes. Code et identité propres.

## UX/UI — exigences
- Bibliothèque sous forme de grandes cartes rectangulaires homogènes.
- Chaque carte : logo, couleur propre à l'outil, nom, phrase courte, version, état et action principale.
- Clic sur la carte : ouvre une grande fiche détaillée intégrée au launcher.
- Fiche : en-tête coloré, logo, présentation, captures, fonctionnalités, nouveautés, version et liens facultatifs (GitHub/site/documentation), action d'installation/lancement/mise à jour.
- États : non installé, installé à jour, mise à jour disponible, téléchargement en cours, installation en cours, erreur.
- Navigation sobre, thème sombre, aucun dashboard superflu.
- Reproduire le *format et la hiérarchie* des vues ArtCraft, sans utiliser ses marques, logos ni assets réservés.

## Fonctionnel — V1
- Catalogue distant configurable et mis en cache.
- Ajout d'une application au catalogue sans recompilation du launcher.
- Identification de la plateforme et de l'architecture.
- Téléchargement de packages natifs avec intégrité vérifiée.
- Détection locale si elle est supportée par l'application et sa méthode d'installation.
- Installation, lancement, mise à jour, désinstallation via mécanismes natifs compatibles.
- Mise à jour du launcher par mécanisme séparé.
- Applications utilisables et installables sans launcher.
- Hors ligne : consulter le cache et lancer les applications installées.
- Windows et macOS prioritaires. Linux ensuite.

## Architecture visée
- Rust + egui/eframe (choix de départ, à valider par prototype).
- Séparation UI / domaine (catalogue, versions, états) / plateformes et installation / mises à jour.
- Manifeste structuré et versionné. Sources GitHub Releases et téléchargement HTTPS ; source privée possible ultérieurement.
- Aucune URL de téléchargement ou commande d'installation arbitraire ne doit être exécutée depuis un catalogue non fiable.
- Vérifier provenance, empreinte SHA-256 et signature quand disponible ; ne pas confondre intégrité et authenticité.

## Hors périmètre V1
Marketplace ouverte, comptes, paiement, licences, avis, télémétrie, multi-versions avancées.

## Critères d'acceptation
1. L'utilisateur parcourt les cartes et ouvre une fiche détaillée.
2. Chaque carte montre version, état et action principale correcte.
3. Le catalogue peut accueillir une application sans recompilation.
4. Installation et lancement fonctionnent sur au moins un OS cible.
5. Mise à jour applicative et mise à jour du launcher fonctionnent et gèrent les erreurs.
6. Le mode hors ligne ne bloque pas le lancement des apps présentes.
7. Une application fonctionne en dehors du launcher.

## Référence et limites
ArtCraft sert de référence UX/UI et de comportement. Ne pas cloner sa marque ou ses illustrations.
Ne pas supposer qu'une capacité ArtCraft est validée tant qu'elle n'est pas auditée ou testée.
Les captures de référence devront être ajoutées au dépôt avec l'autorisation et les droits adéquats, ou remplacées par des annotations internes.
