# Étape 3 — détection et lancement natifs

Périmètre confirmé à l'[étape 3.1](ETAPE_3_1.md) : le moteur décrit ici ne
détecte que les références du catalogue choisi. Aucun ajout automatique de logiciel
tiers ; OBS est un contrôle technique facultatif, hors du catalogue par défaut.

Branche `feat/native-app-detection`, créée depuis `origin/dev` au commit
`08ce7b6` après fusion de la PR #3. L'interface de l'étape 2.1 est conservée :
cartes, proportions, grille/liste, tri, recherche, navigation, réglages et fiches.
Seuls les états, actions et informations locales sont branchés au module natif.

Aucune installation, désinstallation, mise à jour automatique ou récupération de
package. Le chargement habituel des métadonnées et images du catalogue demeure.
Les simulations sont réservées aux applications explicitement fictives.

## Modèle et compatibilité du catalogue

Le schéma 1 reste accepté pour les catalogues existants. Le schéma 2 ajoute un
champ facultatif `detection` sur les applications non fictives. Sans règle pour
l'OS courant, aucune installation n'est déclarée et aucun lancement réel n'est
possible. Aucune association par nom affiché, par GitHub ou par simple nom de fichier.

Extrait de l'exemple OBS livré (`catalog/obs-catalog.json`, schéma 2) :

```json
"detection": {
  "macos": {
    "bundle_id": "com.obsproject.obs-studio",
    "bundle_paths": ["OBS.app"]
  },
  "windows": {
    "uninstall_key": "OBS Studio",
    "publisher": "OBS Project",
    "executable": "bin/64bit/obs64.exe"
  }
}
```

Les règles doivent être examinées avec le catalogue approuvé. Les identités sont
uniques par OS dans un catalogue. Une application fictive ne peut pas déclarer
ces règles. Les commandes, arguments, URLs d'exécution, chemins absolus et
traversées dans les règles sont interdits. Sous Windows, les interpréteurs et
installateurs usuels (`cmd`, PowerShell, `msiexec`, etc.) sont refusés.

`catalog/schema-v2.json` décrit la structure ; la validation Rust ajoute les
contraintes sémantiques et de confinement. `tests/fixtures/native-v2.json` et
`native-invalid.json` illustrent acceptation et rejet. Le schéma 1 n'accepte pas
de règle de détection non nulle. Pour migrer : passer `schema_version` à `2`,
conserver les IDs/métadonnées et ajouter seulement les règles vérifiées.
Un ancien launcher strict des étapes 1/2 rejette le schéma 2 : mettre à jour le
launcher d'abord ou conserver un endpoint de schéma 1 pour ces clients.
Les schémas futurs inconnus restent rejetés. Aucun état installé ne provient du JSON.

## macOS

La détection vérifie uniquement les chemins relatifs explicitement déclarés,
dans cet ordre de racines : `~/Applications`, `/Applications`,
`/System/Applications`. Au plus quatre chemins par règle, soit douze candidats ;
aucune recherche récursive, Spotlight, commande shell ou exécution du logiciel.

Pour chaque candidat existant :

1. Résolution canonique du bundle et confinement sous la racine ; extension `.app`.
2. Lecture bornée à 256 Kio de `Contents/Info.plist`, XML ou binaire via `plist`.
   Le fichier résolu doit rester dans le bundle.
3. Égalité exacte de `CFBundleIdentifier`, et `CFBundlePackageType = APPL`.
4. `CFBundleExecutable` doit être un nom simple, sans séparateur ni commande.
   Le fichier canonique doit exister sous `Contents/MacOS` et être exécutable.
5. Version lue dans `CFBundleShortVersionString`, sinon `CFBundleVersion`.
   Une version absente n'empêche pas une détection valide.

Au clic OUVRIR, une nouvelle détection revérifie le même chemin canonique et le
même exécutable. Le lancement utilise `/usr/bin/open`, avec les deux arguments
littéraux `-a` et le chemin canonique du bundle, via LaunchServices. Aucun shell,
`--args`, URL, recherche par nom ou repli sur une autre installation. Le retour
du helper est contrôlé, avec un délai de dix secondes ; erreur et refus sont
signalés. Une réussite signifie que la demande a été transmise à l'OS.

Sources : [clés Core Foundation](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html)
et manuel macOS `man open` (lancement par chemin d'application). L'identifiant OBS
est également vérifié dans l'Info.plist de l'installation locale de contrôle.

## Windows

Lecture directe du registre avec `winreg`, sans PowerShell, WMI, `reg.exe`,
`Win32_Product` ou lancement d'un désinstalleur. Seule la clé exacte
`Software\Microsoft\Windows\CurrentVersion\Uninstall\<uninstall_key>` est ouverte,
en lecture `KEY_QUERY_VALUE`, dans cet ordre : HKCU 64 bits, HKCU 32 bits,
HKLM 64 bits, HKLM 32 bits. Les vues WOW64 sont explicites ; aucune énumération
large ni écriture du registre dans le code produit.

La règle vérifie l'identifiant de clé et l'égalité exacte de `Publisher`.
`InstallLocation` fournit le dossier, `DisplayVersion` la version facultative.
Seuls des `REG_SZ` bornés après lecture sont acceptés ; aucune expansion
`%VARIABLE%`. `DisplayName`, `DisplayIcon`, `UninstallString` et commandes de
maintenance ne sont jamais utilisés comme autorité ou comme commande.

`InstallLocation` doit être absolu sur un lecteur local. UNC, chemins de
périphérique et chemins relatifs au lecteur sont refusés, y compris après
canonicalisation. L'exécutable relatif approuvé est résolu sous ce dossier ;
les jonctions/symlinks qui en sortent sont refusés. Le fichier doit être un `.exe`
avec en-têtes MZ/PE cohérents, indicateur exécutable et sans indicateur DLL.
La recherche d'en-tête PE est bornée au premier Mio ; aucun fichier entier n'est
chargé pour cette vérification.

Au clic, la clé et les fichiers sont relus et le candidat exact est revérifié.
`std::process::Command` démarre directement cet `.exe` via le mécanisme Windows
natif, sans arguments ni shell, avec le répertoire de l'exécutable comme dossier
courant. Le processus est détaché du traitement UI ; une erreur CreateProcess
est signalée, sans simulation. La réussite ne garantit pas que le logiciel
n'échouera pas ensuite à son propre démarrage.

Sources : [registre de désinstallation Windows Installer](https://learn.microsoft.com/en-us/windows/win32/msi/uninstall-registry-key)
et [vues alternatives du registre](https://learn.microsoft.com/en-us/windows/win32/winprog64/accessing-an-alternate-registry-view).
La règle OBS vise les installations classiques exposant la clé, l'éditeur et
le chemin indiqués. Ces identifiants viennent du
[script officiel NSIS OBS](https://github.com/obsproject/obs-studio/blob/26.1.2/UI/installer/mp-installer.nsi) ;
les distributions ou installateurs ne conservant pas ces valeurs nécessitent
une règle spécifique vérifiée. L'exemple ne prétend pas couvrir OBS portable/MSIX.

## État, versions et installations multiples

`Application.metadata` reste le catalogue approuvé. `Application.local` reçoit
les chemins, versions et diagnostics produits par la détection ; `simulation`
reste séparée et non persistée. Une installation existe indépendamment de la
connaissance de sa version : OUVRIR reste disponible avec « version inconnue ».
Une simulation ne peut pas fournir une installation réelle ni contourner cette
vérification. Une application réelle ne peut plus démarrer un transfert simulé.

Les candidats sont dédupliqués par chemin canonique, dans l'ordre déterministe
ci-dessus (et l'ordre `bundle_paths` sur macOS). Le premier est retenu pour
OUVRIR ; les autres chemins figurent dans la fiche existante. Pas de sélection
« plus récente » devinée ni de repli silencieux si le candidat retenu disparaît.
Un clic sur Actualiser recalcule le premier candidat encore valide.

Comparaison SemVer, puis comparaison numérique de versions à un à quatre
composants pour les versions Windows (`1.2.3.0` équivaut à `1.2.3`). Les versions
non comparables restent affichées sans promesse « à jour ». Une version locale
plus récente que le catalogue n'est pas présentée comme mise à jour à appliquer.
Le catalogue disponible n'est jamais réécrit. Les boutons d'installation/mise à
jour et l'action groupée ne peuvent agir sur une application réelle à cette étape.

## Thread graphique et erreurs

`src/native/{model,macos,windows,worker,mod}.rs` ne dépend pas de l'UI. Le worker
reçoit des identifiants de catalogue et règles immuables. Le démarrage et chaque
chargement/actualisation du catalogue, y compris depuis le cache hors ligne,
planifient une détection. Une seule détection est active ; un nouveau catalogue
pendant une détection est mis en attente. Les générations, IDs et règles sont
vérifiés avant d'adopter les résultats ; les anciens résultats ne peuvent pas
réinstaller une application supprimée ou changer son identité.

Aucun scan ni accès au disque dans la boucle de rendu. Le lancement et sa
revérification utilisent aussi un worker, avec une seule demande active. Les
résultats arrivent par canal ; un worker arrêté sans résultat est signalé.
Les chemins manquants, permissions, plists/PE invalides et refus sont conservés
dans les diagnostics de la fiche ; les erreurs d'ouverture utilisent le bandeau
existant. La détection est relancée après une demande d'ouverture.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --all-targets --all-features
cargo run --locked --example detect -- catalog/obs-catalog.json
STUDIO_APP_CONFIG="$PWD/catalog/obs-config.json" cargo run --locked
```

Le diagnostic `examples/detect.rs` est strictement en lecture seule : aucun
argument de lancement et aucune exécution. Les tests sont hors ligne.

44 tests passent localement sur macOS : 29 tests de bibliothèque et 15 tests
catalogue. Présence/absence, identité incorrecte malgré le même nom, version
inconnue/identique/différente, plusieurs installations et déduplication,
permissions, fichiers manquants, symlinks, plists XML/binaires, en-têtes PE,
limites, schémas et règles invalides, résultat périmé, arrêt de worker,
échec d'ouverture et arguments littéraux sont couverts. Les tests UI existants
continuent de vérifier cartes, fiches, navigation, grille/liste, tailles et HiDPI.

La CI Windows comprend un test natif supplémentaire : une clé HKCU réservée au
test et supprimée ensuite, un exécutable PE de fixture volontairement invalide
pour vérifier l'erreur CreateProcess, puis `where.exe` fourni par Windows, sans
argument, pour vérifier un démarrage natif réussi. Les sorties sont redirigées
vers le périphérique nul. La détection ne modifie pas la valeur sentinelle du
registre. Le code produit ne crée aucune de ces clés et n'écrit jamais le registre.

Contrôle natif macOS : OBS existant détecté sous `/Applications/OBS.app`, clic sur
OUVRIR dans la carte, retour natif réussi et processus `com.obsproject.obs-studio`
confirmé en cours d'exécution. Actualisation du catalogue et de la version locale
vérifiée ; le diagnostic final lit OBS 33.0.1. Aucun enregistrement ou streaming
n'a été déclenché. [Capture native](screenshots/native-detection-macos.png).
La configuration du contrôle a été placée dans un dossier temporaire accessible
pour éviter un blocage macOS à l'ouverture de fichiers du dossier Documents ;
aucune permission système n'a été modifiée.

## Limites

- Linux conserve le catalogue et les simulations ; pas de détecteur/lancement natif.
- macOS : applications hors des racines ciblées, renommées sans adaptation de la
  règle, alias Finder et versions déplacées dans Downloads/volumes externes non pris en charge.
- Windows : MSIX/UWP, raccourcis, App Paths, applications portables sans clé
  d'installation, valeurs REG_EXPAND_SZ ou chemins UNC non pris en charge.
- Une clé d'installation sans `InstallLocation` valide n'est pas devinée à partir
  d'une commande de désinstallation, même si son nom ressemble au catalogue.
- Les identifiants/éditeurs lus sont des preuves de correspondance locale, pas
  une authentification cryptographique de l'éditeur. Les signatures de code,
  Team ID/Authenticode, durcissement face à un attaquant local et suppression de
  toute course entre vérification et ouverture restent hors de cette étape.
- Aucun suivi permanent des modifications système ; actualiser après une
  installation manuelle. Une application peut encore échouer après acceptation
  du lancement par l'OS. Les autorisations macOS/Windows restent celles de l'utilisateur.
- Ouverture réelle validée sur macOS et tests natifs de processus/registre en CI
  Windows ; aucun parcours graphique OBS sur un poste Windows n'a été effectué ici.
