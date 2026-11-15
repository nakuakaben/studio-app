# Hyper Media Puller dans Studio App

HMP est la première application de la bibliothèque. Studio App détecte les
releases publiques du dépôt configuré et propose le lien officiel pour
l’installation manuelle et les mises à jour. Les applications de démonstration
sont masquées par défaut.

## Installation personnelle

Ce profil est désactivé par défaut. Il accepte exclusivement les deux artefacts
épinglés de [HMP v0.1.0-beta.1](https://github.com/nakuakaben/hyper-media-puller/releases/tag/v0.1.0-beta.1).
Une nouvelle release reste disponible par le lien officiel jusqu’à ce que ses
artefacts et sa recette soient examinés et épinglés. Les digests GitHub ne
constituent aucune autorisation. Le profil de production reste distinct et inactif.

| Cible | Asset approuvable | SHA-256 |
| --- | --- | --- |
| macOS Apple Silicon | `HMP-V2-0.1.0-beta.1-macos-arm64.dmg` | `19626af397aa54318b2adc6b03287fc224f9790e6b18eb54233a9d3514ab6097` |
| Windows x64 | `HMP-V2-0.1.0-beta.1-windows-x64-setup.exe` | `d35dcb98366c1bd4b0f44ef1e250159175dbee2e388c215094a173bde1a2255e` |

### Parcours standard — sans installation d'outils

Installer le DMG Studio App (macOS Apple Silicon) ou extraire son ZIP (Windows x64),
puis lancer Studio App normalement, depuis le Finder ou l'Explorateur.
**Aucun Rust, Cargo, Git, script, Terminal ou variable d'environnement n'est
nécessaire** pour installer HMP depuis le launcher.

Studio App contient une autorisation limitée aux deux artefacts exacts de
HMP `v0.1.0-beta.1` (URL officielle, nom, taille, SHA-256 et recette).
Un contrôle du fichier téléchargé est toujours effectué. Une clé Ed25519
éphémère est créée uniquement en mémoire pour la politique de consentement
personnelle de la session. **Elle ne certifie pas l'éditeur HMP** et ne
permet pas d'autoriser une autre version ni un package arbitraire.
Aucune clé privée n'est embarquée dans le binaire ou écrite sur le disque.

L'utilisateur doit cocher explicitement son accord dans le dialogue pour
chaque installation. Cette autorisation ne vaut pas signature Apple Developer
ID ni Authenticode. Gatekeeper, SmartScreen et les contrôles de l'OS restent
actifs. En cas de refus, consulter les indications officielles du système.

### Provisionnement avancé (développeurs uniquement)

L'ancienne politique externe reste possible pour les essais contrôlés via
`STUDIO_APP_PERSONAL_INSTALLER_POLICY`. Elle est **facultative**, et n'est
pas nécessaire au parcours utilisateur. Les commandes de génération restent
réservées au développement :

```sh
cargo run --locked --bin studio-catalog -- personal-keygen /chemin/prive/hmp.key
cargo run --locked --bin studio-catalog -- personal-hmp /chemin/prive/hmp.key EXPIRATION_UNIX /chemin/prive/policy.json
```

Ne jamais publier la clé privée. Une variable dédiée à une politique externe
s'applique uniquement au processus explicitement lancé par l'opérateur.

### Installer et vérifier

Ouvrir **Installer HMP**, vérifier la version, la cible et le fichier épinglé,
cocher l’accord et cliquer **Télécharger**. Studio App vérifie
l’approbation avant le téléchargement HTTPS et de nouveau avant la pose ou
l’exécution. Le transfert vérifie la taille et le SHA-256. Il n’accepte qu’un
saut vers le fichier précis du CDN officiel GitHub. L’annulation est possible
avant la copie finale ou avant l’ouverture du NSIS.

- **macOS ARM64.** Le DMG est monté en lecture seule. Studio App contrôle le
  bundle `HMP.app`, son identité et sa signature de code, conserve ou ajoute
  le marquage de quarantaine et pose le bundle sans remplacement sous
  `~/Applications`. La signature ad hoc est admise uniquement dans ce profil.
  Une édition interne reconnue dans un autre dossier peut coexister. Tout
  bundle déjà présent à la destination personnelle bloque l’installation.
  La redétection annonce la version déclarée par le bundle. Pour cet asset,
  `0.1.0` laisse la révision bêta indéterminée. **Ouvrir** délègue à
  LaunchServices. En cas de refus, consulter la décision de macOS et, si
  l’option est autorisée par sa politique, suivre la procédure de consentement
  officielle. [Ouverture d’apps sur macOS](https://support.apple.com/fr-fr/102445)
- **Windows x64.** Après le transfert et le contrôle PE, **Ouvrir NSIS** exige
  un second accord. Le fichier conserve sa marque Internet. Le service natif
  des pièces jointes Windows lance le NSIS interactif, sans arguments
  silencieux ni demande d’élévation forcée. Choisir un dossier vide dans NSIS.
  WebView2 Evergreen reste un prérequis indiqué par le guide de la release.
  Studio App attend l’installateur et exige une entrée HKCU HMP avec l’éditeur,
  le chemin, l’exécutable et la version approuvée. Une installation concurrente
  hors de Studio App ou un ancien dossier non enregistré choisi dans NSIS
  relèvent du contrôle du poste et de l’installateur. [Service d’exécution des pièces jointes Windows](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iattachmentexecute-execute)

L’approbation locale n’est pas une notarisation Apple ni une signature
Authenticode. La release officielle reste le repli en cas de refus ou
d’absence de politique. Si une installation est déjà détectée, **Ouvrir**
reste disponible. La mise à jour et la désinstallation intégrées sont hors
de ce parcours.

## Validation

- Les tests hors ligne couvrent les signatures locales, l’expiration, les
  mutations d’URL, de taille et d’empreinte, le refus des autres architectures,
  les redirections et les fixtures d’installation isolées.
- Un essai macOS ARM64 volontaire consulte GitHub, télécharge et vérifie le
  vrai DMG avec une clé éphémère, pose le bundle dans une racine temporaire,
  puis contrôle la redétection et la quarantaine. Il n’ouvre pas HMP.

```sh
cargo test --locked --lib approved_public_hmp_dmg_installs_and_detects_without_touching_applications -- --ignored
```

- Essai interactif Windows restant à qualifier sur une VM x64 propre avec
  consentement explicite. Vérifier l’avertissement du système, le comportement
  avec ou sans WebView2, le résultat NSIS et la clé HKCU après installation.
  Les fixtures ZIP de la CI exercent le moteur natif sans exécuter le NSIS HMP.
