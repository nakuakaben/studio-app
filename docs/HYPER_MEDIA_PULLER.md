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

### Générer sa politique locale

Depuis le dépôt source, sur macOS par exemple, créer un dossier privé en dehors
du dépôt. Choisir un fichier de sortie neuf et vérifier les empreintes des
artefacts avant de signer. La durée maximale de génération est de 30 jours.

```sh
umask 077
mkdir -p "$HOME/.studio-hmp-personal"
studio_hmp_dir="$HOME/.studio-hmp-personal"
studio_hmp_expires=$(($(date +%s) + 7 * 86400))
cargo run --locked --bin studio-catalog -- personal-keygen "$studio_hmp_dir/hmp.key"
cargo run --locked --bin studio-catalog -- personal-hmp "$studio_hmp_dir/hmp.key" "$studio_hmp_expires" "$studio_hmp_dir/policy.json"
STUDIO_APP_PERSONAL_INSTALLER_POLICY="$studio_hmp_dir/policy.json" cargo run --locked --bin studio-app
```

Pour lancer l’application packagée depuis le même shell, appeler son exécutable
`Studio App.app/Contents/MacOS/studio-app` par son chemin absolu avec la même
variable. L’ouverture habituelle depuis le Finder continue d’utiliser le profil
public.

Sous Windows, l’équivalent PowerShell depuis le dépôt est le suivant.

```pwsh
$studioHmpDir = Join-Path $env:USERPROFILE '.studio-hmp-personal'
New-Item -ItemType Directory -Path $studioHmpDir -Force | Out-Null
$studioHmpKey = Join-Path $studioHmpDir 'hmp.key'
$studioHmpPolicy = Join-Path $studioHmpDir 'policy.json'
$studioHmpExpires = [DateTimeOffset]::UtcNow.AddDays(7).ToUnixTimeSeconds()
cargo run --locked --bin studio-catalog -- personal-keygen $studioHmpKey
cargo run --locked --bin studio-catalog -- personal-hmp $studioHmpKey $studioHmpExpires $studioHmpPolicy
$env:STUDIO_APP_PERSONAL_INSTALLER_POLICY = $studioHmpPolicy
cargo run --locked --bin studio-app
```

Une fois la politique créée, il est aussi possible de lancer `studio-app.exe`
extrait du ZIP depuis ce PowerShell. Conserver la clé privée hors du dépôt et
hors du dossier distribué. Le JSON contient la clé **publique** et les
approbations signées. La variable exige un chemin absolu et doit être définie
pour le seul processus d’essai. Retirer la variable ou déplacer la politique
empêche les nouveaux transferts et les nouvelles poses. Les commandes refusent
d’écraser un fichier auparavant créé. Pour renouveler la politique, utiliser un
nouveau chemin de sortie et le sélectionner explicitement.

### Installer et vérifier

Ouvrir **Installer HMP**, vérifier la version, la cible, le fichier et la clé
publique locale, cocher l’accord et cliquer **Télécharger**. Studio App vérifie
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
