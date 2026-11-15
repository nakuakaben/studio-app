# Hyper Media Puller dans Studio App

HMP est le premier logiciel réel référencé dans la configuration par défaut.
La [bêta publique v0.1.0-beta.1](https://github.com/nakuakaben/hyper-media-puller/releases/tag/v0.1.0-beta.1)
fournit un DMG pour macOS Apple Silicon et un installateur NSIS pour Windows x64.
Le catalogue de démonstration reste disponible dans la même bibliothèque.

Au démarrage et à l’actualisation du catalogue, Studio App interroge l’API
publique du seul dépôt configuré dans `catalog/hyper-media-puller.json`. La
dernière version SemVer complète est proposée dès que GitHub la publie : les
brouillons, releases sans date, assets manquants, noms ou sources inattendus
et digests SHA-256 invalides sont ignorés. Un cache conserve la dernière
réponse valide en cas de panne réseau. Une application inscrite directement
dans le catalogue de distribution garde la priorité sur cette fiche locale.

Sur une plateforme compatible, **INSTALLATEUR ↗** ouvre la page de la
version exacte sur GitHub. Utiliser le guide et le manifeste SHA-256 joints
à cette version. Après une installation manuelle, actualiser la bibliothèque
pour détecter HMP. Une version locale comparée comme plus ancienne affiche
**MISE À JOUR ↗** ; quitter HMP après la fin des transferts et suivre le
guide du nouvel installateur. Studio App ne ferme ni ne remplace HMP.

La détection vise l’édition publique : bundle
`fr.tktprod.hyperdeck-media-puller.v2.preview` dans `HMP.app` sur macOS,
identité NSIS `HMP` / `tktprod` et exécutable `hmp-v2-preview.exe`
sur Windows. L’édition interne a un autre identifiant de bundle et ne doit
pas être reconnue comme une installation publique. La détection Windows est
dérivée de la configuration et du modèle NSIS Tauri ; le test de ce raccord
sur une installation Windows réelle reste à faire.

Le DMG public testé annonce `0.1.0` dans le bundle, même pour
`0.1.0-beta.1`. Si la version locale ne contient pas de suffixe et
correspond à une série bêta publiée connue, Studio App indique **BÊTA NON
IDENTIFIÉE** et propose **VERSIONS ↗** sur macOS. Une prochaine bêta devrait
exposer sa version de release complète dans une métadonnée de bundle fiable
pour distinguer ses révisions lors de la détection.

Cette intégration ne crée aucun package approuvé ni exécutable par Studio App.
Le digest est un contrôle d’intégrité et ne remplace pas la signature Ed25519
de distribution. Le DMG est signé ad hoc et le NSIS n’est pas signé
Authenticode. L’installation directe nécessitera des binaires et une politique
de distribution validés, puis des essais de mise à jour avec préservation des
données et vérification après remplacement. Le chemin actuel conserve les
protections du navigateur et du système.
