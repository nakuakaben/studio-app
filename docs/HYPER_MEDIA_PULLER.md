# Hyper Media Puller dans Studio App

HMP est le premier logiciel réel référencé dans la configuration par défaut.
La [bêta publique v0.1.0-beta.1](https://github.com/nakuakaben/hyper-media-puller/releases/tag/v0.1.0-beta.1)
fournit un DMG pour macOS Apple Silicon et un installateur NSIS pour Windows x64.
Le catalogue de démonstration est masqué par défaut.

Au démarrage et à l’actualisation du catalogue, Studio App interroge l’API
publique du seul dépôt configuré dans `catalog/hyper-media-puller.json`. La
dernière version SemVer complète est proposée dès que GitHub la publie : les
brouillons, releases sans date, assets manquants, noms ou sources inattendus
et digests SHA-256 invalides sont ignorés. Un cache conserve la dernière
réponse valide en cas de panne réseau. Une application inscrite directement
dans le catalogue de distribution garde la priorité sur cette fiche locale.

**INSTALLER** ouvre la fenêtre **Installer HMP**. Elle sélectionne l’asset
correspondant au système et à l’architecture et affiche son nom, sa taille
et la destination usuelle. Windows utilise l’architecture native du système
pour refuser ARM64 même si le launcher x64 s’exécute par émulation. Aucun
asset n’est proposé sur Mac Intel, Windows ARM64 ou Linux.

Pour la bêta actuelle, le téléchargement intégré est bloqué. Le bouton
**Ouvrir la release officielle** ouvre la page de la version exacte sur
GitHub. Utiliser le guide et le manifeste SHA-256 joints à cette version.
Après une installation manuelle, actualiser la bibliothèque
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

## Approbation et validation

Les métadonnées publiques sont distinctes des packages approuvés et ne
déclenchent aucun téléchargement dans Studio App. Lorsqu’un package pris
en charge figure dans un catalogue explicite, la fenêtre contrôle la
politique locale en arrière-plan. Le moteur revérifie la signature Ed25519,
les métadonnées exactes, l’expiration et la destination confirmée avant le
transfert. Le profil isolé `installer-fixtures` permet de tester les octets
réellement reçus, l’annulation avant la pose, la reprise par une nouvelle
tentative, la détection et **OUVRIR**. Il utilise une application de test
signée par une clé éphémère, un serveur HTTPS loopback et une destination
réservée. Voir [le protocole de test](ETAPE_4.md).

| Cible | Mécanisme et prérequis pour la production |
| --- | --- |
| macOS Apple Silicon | DMG monté en lecture seule dans un point privé, bundle `HMP.app` vérifié avant copie sans remplacement sous `~/Applications`. L’éditeur doit fournir un bundle signé Developer ID avec un Team ID réel, une notarisation valide et une métadonnée signée qui identifie la révision complète. Qualifier le contrôle système de cette identité et de Gatekeeper sur le package final. La bêta actuelle est ad hoc et déclare seulement `0.1.0`. |
| Windows x64 | Le NSIS public est interactif, par utilisateur, avec destination par défaut dans `%LOCALAPPDATA%` et choix dans l’installateur. WebView2 Evergreen est requis. L’éditeur doit fournir l’installateur et les binaires signés Authenticode. Qualifier la vérification Windows de la chaîne de confiance et du signataire attendu avant tout lancement, puis la détection par clé HKCU, identité, version complète et chemin réel. Aucun switch silencieux ou paramètre de destination n’est supposé. Le NSIS et les binaires actuels sont sans Authenticode. |

Les deux cibles nécessitent une approbation Ed25519 de production qui lie
la version, l’asset exact, l’URL, le SHA-256 et la recette validée. Les
empreintes GitHub attestent l’intégrité des octets publiés. Aucun certificat
de test ni aucune clé privée ne doit être déployé en production. Le moteur
refuse encore le profil de production et les exécutables NSIS, y compris
avant transfert. L’interface ne propose aucun sélecteur libre tant qu’une
recette approuvée ne peut pas appliquer ce choix.

Les essais Windows du moteur portent sur un ZIP portable de test et sa
clé HKCU dédiée. L’installation et la redétection du véritable NSIS HMP
restent à qualifier sur un Windows x64 isolé. La mise à jour intégrée reste
hors du parcours validé.
