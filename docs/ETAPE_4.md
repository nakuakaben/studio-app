# Étape 4 — moteur d’installation native

Cette étape livre un premier parcours **réel, isolé et reproductible** : INSTALLER
sur la carte → HTTPS → approbation et intégrité → installation → détection → OUVRIR.
Le catalogue de démonstration et son simulateur restent inchangés. Aucun logiciel
personnel n’est ajouté au catalogue ni remplacé.

## Périmètre activé

| Plateforme | Format réellement exercé | Destination et mécanisme |
| --- | --- | --- |
| macOS | ZIP contenant un `.app` simple | Copie sans remplacement sous `~/Applications/StudioAppInstallerTest-<nonce>/` |
| macOS | DMG contenant ce même `.app` | `hdiutil attach -readonly -nobrowse -noautoopen`, copie, démontage du point privé |
| Windows | ZIP portable de structure approuvée | Dossier temporaire `StudioAppInstallerTest-<nonce>`, inscription HKCU dédiée, jamais HKLM |
| Linux | Aucun installateur | Compilation, tests de sécurité, catalogue et UI ; installation désactivée |

**Seul le profil du logiciel de test local est activé**, avec la feature
`installer-fixtures`. Le binaire normal refuse ce profil. Le déploiement de
production reste explicitement refusé même avec une approbation Ed25519 valide :
aucune publication Developer ID/notarisée ou Authenticode de notre propre gamme
n’a été fournie pour un test réel. La signature ad-hoc du logiciel de test local
ne valide pas un scénario de distribution publique.

PKG, MSI, EXE, applications avec liens symboliques/frameworks complexes,
installations privilégiées, UAC, mises à jour et désinstallations restent
**désactivés**. Aucun argument silencieux n’est deviné. Aucun `sudo`, shell,
script du catalogue, suppression de quarantaine ou modification de Gatekeeper
n’est utilisé. Le dossier d’installation des réglages reste une option future ;
il ne dirige pas ces tests isolés.

## Architecture

`src/installer/` est indépendant de l’interface :

- `model` : sélection OS/CPU dans la seule publication courante, phases, annulation,
  erreurs permanentes/recommençables, politique et plan autorisé opaque.
- `approval` : validation du manifeste et des règles natives, vérification Ed25519,
  expiration, correspondance exacte des métadonnées et confinement du profil local.
- `download` : transport HTTPS sans proxy/redirection, octets réels, débit moyen,
  reprise contrôlée, taille et SHA-256, fichier temporaire privé.
- `archive` : validation de la structure, limites de ressources, extraction privée.
- `platform` : préparation, signature macOS, montage DMG, copie finale sans écrasement,
  inscription portable Windows, détection finale et retour arrière de notre seule copie.
- `worker` : tout accès réseau/disque et tout outil système hors du thread egui.
- `fixture` et les deux binaires `tools/` : génération et nettoyage du test, exclus
  des builds sans `installer-fixtures`.

Une seule installation à la fois. Les cartes, liste, fiches, thèmes, proportions
et animations de survol sont conservés. Les phases réelles ne portent jamais le
suffixe SIMULÉ. La barre utilise les octets reçus, le débit est accessible au
survol. Annulation pendant transfert/vérification/préparation ; la copie finale
est indivisible du point de vue de l’UI et ne propose plus d’annulation.
Les compteurs de version locale ne sont renseignés que par le détecteur natif.
Un changement du catalogue pendant une opération demande son annulation et empêche
l’application d’un résultat à de nouvelles métadonnées.

## Approbation et authenticité

Le schéma du catalogue reste **v2**, sans migration ni champ de commande. Une URL,
un hash ou un asset découvert par GitHub ne constitue jamais une autorisation.
Il faut une politique **locale de confiance**, distincte du catalogue distant :
`STUDIO_APP_INSTALLER_POLICY`, sinon `installer-policy.json` dans le dossier de
configuration utilisateur de Studio App. Limite : 2 Mio. Ne jamais télécharger
les clés de confiance à côté d’un catalogue non authentifié.

Politique JSON stricte, `schema_version: 1` :

```json
{
  "schema_version": 1,
  "keys": [{ "id": "identifiant-local", "public_key": "32 octets hexadécimaux" }],
  "approvals": [{
    "key_id": "identifiant-local",
    "body": {
      "schema_version": 1,
      "application": "objet AppMetadata complet, pas une chaîne",
      "package_id": "identifiant exact dans release.packages",
      "recipe": { "kind": "mac_bundle", "bundle_name": "Nom.app", "team_id": null },
      "expires_unix": 1800000000
    },
    "signature": "64 octets hexadécimaux"
  }],
  "deployment": { "kind": "production" }
}
```

Cet extrait explique les champs ; ses chaînes descriptives ne sont pas un fichier
valide. Le générateur produit des exemples réellement valides dans son dossier
source temporaire, avec la variante `local_fixture` (racine, nonce, certificat TLS
relatif et port). Aucune clé privée ou approbation exploitable n’est livrée au dépôt.

La signature Ed25519 porte sur le préfixe binaire
`Studio App approved installation v1\0`, suivi du JSON compact **typé** produit par
`approval::signing_message(&ApprovalBody)` : ordre des champs Rust, valeurs par défaut
et `null` sérialisés par serde, sans espaces. Ce n’est pas une signature des octets
bruts du fichier JSON ni une canonicalisation JSON universelle. Les producteurs
doivent utiliser cette fonction. L’approbation lie notamment identifiant, version,
éditeur, règles de détection, plateforme, URL, taille, SHA-256 et recette.
La correspondance exacte est revérifiée avant tout téléchargement. Une entrée
modifiée, une clé absente/ambiguë, un schéma inconnu, une signature invalide ou une
approbation expirée bloque l’opération. L’expiration est revérifiée avant la copie.

SHA-256 prouve l’intégrité des octets associés à ces métadonnées signées ; il n’est
pas traité comme une preuve de provenance autonome. La clé locale est l’ancre de
confiance, à provisionner par l’opérateur. Les clés de la fixture sont éphémères,
créées uniquement pour ce scénario local et ne servent pas à la production.

## Réseau, archives et erreurs

HTTPS obligatoire ; TLS et noms d’hôtes toujours vérifiés. Le profil local accepte
uniquement `packages.example.com/<identifiant-unique>.zip` ou `.dmg`, résolu vers
un serveur loopback et son certificat éphémère explicitement approuvé par ce seul
client. Aucun certificat système n’est ajouté et aucun mode TLS non sûr n’est activé.
Les branches de transport production prévoient la résolution d’IP publiques figée,
mais sont inaccessibles tant que le déploiement production est désactivé.

Limites : package 2 Gio, extraction 4 Gio, fichier 1 Gio, 10 000 entrées, profondeur
24, index ZIP 4 Mio vérifié avant sa lecture par la bibliothèque. ZIP64, multipartie,
SFX, fichiers spéciaux, liens symboliques, traversées, noms de périphériques Windows,
chemins absolus et doublons insensibles à la casse sont refusés. Le staging est privé.
Un DMG n’autorise que la copie du bundle explicitement approuvé, sans auto-ouverture.
Un démontage refusé conserve le point privé et donne son chemin pour inspection ;
aucune suppression récursive d’un montage potentiellement actif n’est tentée.

Connexion : 4 s ; opérations de lecture : 10 s ; durée globale : 30 min.
Au plus trois requêtes. Une réponse interrompue ne reprend que si `Accept-Ranges:
bytes`, ETag fort identique, réponse 206, `Content-Range` exact et taille cohérente.
Une réponse 200 à une demande de reprise n’est jamais ajoutée au fichier partiel.
La reprise se fait dans la même opération, sans persistance intersessions.
L’annulation peut attendre la fin d’une lecture bloquée (au plus son timeout).
Les temporaires sont supprimés par RAII après erreur/annulation/fin normale.
Un arrêt brutal du processus/OS peut laisser des temporaires, à gérer via le
nettoyage du répertoire temporaire du système.

La destination est réservée par création exclusive : une installation détectée,
un dossier existant ou une clé Windows existante empêche tout remplacement.
Un marqueur `.studio-app-installing` bloque la détection pendant la copie ; un
arrêt brutal laisse donc une installation partielle non ouvrable par le launcher.
Seul le nouveau dossier créé par cette opération est retiré après échec géré.
La clé HKCU de test est retirée seulement si son marqueur de propriétaire correspond.
Un échec de nettoyage est signalé ; aucun succès n’est annoncé sur le seul retour
zéro d’un outil. La détection doit confirmer chemin canonique, identité et version.

Sur macOS, le bundle local est contrôlé par `codesign --verify --deep --strict` et
sa signature doit être ad-hoc dans ce profil de développement. Sur Windows, la
structure PE et l’identité du registre sont contrôlées. La version Windows est
celle de la publication approuvée inscrite au registre ; la sonde confirme aussi
0.0.1 à son lancement. Aucun contrôle Authenticode positif n’est revendiqué.
Les contrôles Developer ID, notarisation et Authenticode nécessaires à la future
distribution devront être implémentés et exercés avant d’activer production.

## Test manuel et démonstration

Construire sur le système hôte (macOS ou Windows) :

```sh
cargo build --locked --features installer-fixtures --bins
cargo run --locked --features installer-fixtures --bin studio-installer-fixture -- target/debug/studio-installer-probe zip
```

Sous Windows, utiliser `target/debug/studio-installer-probe.exe`. Sur macOS,
remplacer `zip` par `dmg` pour le second scénario. Le serveur affiche quatre
variables d’environnement et la destination exacte. Dans **un deuxième terminal**,
recopier ces valeurs pour cette seule session, puis lancer :

```sh
cargo run --locked --features installer-fixtures --bin studio-app
```

Sous PowerShell, affecter les valeurs par `$env:STUDIO_APP_CONFIG=...`, etc.
Les préférences/cache du test ont leurs propres dossiers ; ne pas changer les
variables de la session habituelle. Le serveur reste ouvert pendant le test.

1. Vérifier que la carte Studio Installer Test est disponible, version 0.0.1.
2. Cliquer INSTALLER : observer les octets, le pourcentage, puis les phases natives.
3. Attendre la confirmation de détection et le bouton OUVRIR.
4. Cliquer OUVRIR. La sonde minimale sans fenêtre se lance puis termine ; le fichier
   `probe-launched.txt` du dossier nonce contient `Studio App Installer Test 0.0.1 launched`.
5. Fermer le launcher de test. Appuyer sur Entrée dans le terminal serveur : suppression
   de ce seul dossier marqué, de sa clé HKCU éventuelle et des sources temporaires.

Après interruption brutale, conserver le chemin de la politique affiché puis :

```sh
cargo run --locked --features installer-fixtures --bin studio-installer-fixture -- --cleanup /chemin/exact/installer-policy.json
```

Cette récupération exige la signature de la fixture, ses identifiants réservés,
son nonce et son marqueur. Les fichiers étrangers au test à la racine font refuser
le nettoyage. Elle ne remplace pas un désinstalleur de produit. Les sources
restées dans `studio-installer-source-*` peuvent ensuite être retirées du dossier
temporaire après inspection ; aucune clé privée durable n’y est enregistrée.

Démonstration réelle réalisée sur macOS : clic INSTALLER sur la carte, réception
HTTPS affichée à 39 %, confirmation native de la version 0.0.1, clic OUVRIR et
vérification du témoin `Studio App Installer Test 0.0.1 launched`, puis nettoyage.
Quatre captures sont préparées localement ; leur publication dans cette PR reste
soumise à l’autorisation explicite du propriétaire. Aucun résultat de simulation
n’est utilisé comme preuve.

## Validation automatisée

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --all-targets --all-features
```

Les tests n’utilisent aucun serveur public ni logiciel personnel. Les tests natifs
utilisent le serveur HTTPS loopback, le binaire sonde construit pour l’hôte et des
identités/dossiers aléatoires. Ils installent, détectent, lancent, contrôlent le témoin,
refusent la réinstallation et nettoient. macOS exerce ZIP et DMG ; Windows exerce
le portable ZIP et HKCU. Linux ne prétend pas exercer une installation native.

Les tests unitaires couvrent sélection, architecture/format incompatibles, version
courante seule, signature/expiration/métadonnées, réseau, interruption, reprise,
taille/hash, annulation, corruption, index ZIP borné, traversées/liens/doublons,
marqueur d’installation partielle et suppression des temporaires après échec.
Les intégrations couvrent aussi refus de remplacement, source non approuvée,
échec TLS et permissions (macOS). Des tests egui rendent les octets réels dans la
grille, la liste et la fiche en clair/sombre sans état simulé. Les tests existants
de recherche, navigation, survol et simulateur sont conservés.

## Références système

Les signatures macOS et la notarisation sont deux contrôles distincts :
[Code Signing Guide d’Apple](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/Procedures/Procedures.html),
[Notarizing macOS software](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
La production attend un scénario réel signé et notarisé ; une signature ad-hoc locale
n’en démontre pas la conformité. Les futurs MSI devront utiliser les mécanismes
[Windows Installer documentés](https://learn.microsoft.com/en-us/windows/win32/msi/standard-installer-command-line-options),
avec validation des signatures et des scénarios UAC avant activation.
