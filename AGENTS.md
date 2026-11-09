# Consignes pour Codex et agents de développement

## But
Créer **Studio App**, un launcher Rust natif, inspiré très fidèlement de l'ergonomie ArtCraft. Lire `docs/CAHIER_DES_CHARGES.md` avant toute modification.

## Règles
- Studio App distribue notre propre gamme professionnelle : afficher uniquement les applications explicitement publiées au catalogue approuvé. Aucun inventaire universel ni ajout automatique de logiciels tiers. La détection native s'applique seulement aux identifiants référencés. OBS demeure un exemple technique facultatif, hors du catalogue par défaut. Ne pas inventer les identifiants ou versions de futurs logiciels.
- Ne jamais réduire les cartes applicatives à une grille d'icônes.
- Conserver des cartes complètes : logo, nom, résumé, version, état, action.
- Le clic sur la carte ouvre une fiche détaillée intégrée, avec un en-tête coloré propre à chaque application.
- Design sombre, minimaliste, sans fonctionnalités annexes.
- Chaque application distribuée demeure indépendante du launcher.
- Pas de copie des logos, noms, illustrations ou assets protégés d'ArtCraft.
- Construire du Rust propre et testable ; séparer clairement UI, catalogue, système local et mises à jour.
- Ne pas inventer d'URLs de versions ni lancer des commandes reçues d'un catalogue distant sans validation de confiance.
- Ne pas introduire de télémétrie, comptes, marketplace ou paiement dans la V1.
- Ne pas écrire dans `main` directement. Travailler sur `dev` ou branches de fonctionnalités et utiliser PR + CI pour intégrer.
- Ne jamais faire passer un bouton démonstratif pour une installation opérationnelle.

## Définition de terminé
Fonctionnalité testée, erreurs traitées, documentation mise à jour et CI verte.
