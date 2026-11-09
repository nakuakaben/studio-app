# Catalogue dynamique — schémas 1 et 2

Le launcher lit un manifeste JSON **au runtime**. Modifier ce fichier puis cliquer
sur « Actualiser le catalogue » suffit pour ajouter, modifier ou retirer une
application. Ni recompilation, ni serveur applicatif spécifique ne sont nécessaires.

## Configuration locale de confiance

La configuration est choisie dans cet ordre :

1. Fichier explicitement désigné par `STUDIO_APP_CONFIG`.
2. `catalog-config.json` dans le dossier de configuration utilisateur fourni par
   `directories::ProjectDirs("app", "StudioApp", "StudioApp")`.
3. `catalog/config.json` dans le dossier courant.
4. Le même fichier trouvé dans les six parents du binaire, pour les lancements
   du prototype depuis le checkout. Sinon, le chemin utilisateur est retenu et
   l’absence du fichier est signalée.

Le chargement et l’actualisation relisent cette configuration. Elle est locale
et ne peut pas être remplacée par un champ du catalogue distant.

Exemple local, fourni dans `catalog/config.json` :

```json
{
  "source": {"kind": "local", "path": "catalog.json"},
  "allowed_asset_hosts": [],
  "demo_scenarios": true
}
```

`path` est relatif au dossier de la configuration. Les traversées, chemins
absolus, préfixes de lecteur, séquences encodées et séparateurs Windows sont
refusés. Le fichier résolu doit rester dans ce dossier, même via un lien
symbolique. Les champs inconnus sont refusés, y compris dans `source`.

Exemple HTTPS à adapter à un hôte que vous contrôlez (ces domaines sont des
exemples, pas un service existant) :

```json
{
  "source": {"kind": "https", "url": "https://catalog.example.com/studio.json"},
  "allowed_asset_hosts": ["images.example.com"],
  "demo_scenarios": false
}
```

Utiliser une URL HTTPS directe et publique. Pas de redirection, de port autre
que 443, d’identifiants dans l’URL, ni de fragment. La résolution DNS est vérifiée
et épinglée au client ; les réseaux privés, loopback, link-local et plusieurs
plages réservées sont refusés. Les proxies d’environnement sont désactivés.
Il n’y a pas encore d’authentification de catalogue privé ni de CA personnalisée.

`allowed_asset_hosts` autorise seulement les images, pas une installation.
Il accepte au plus 16 noms d’hôtes DNS exacts, sans sous-domaines implicites.
Un catalogue distant ne peut jamais charger une image depuis le disque.
`demo_scenarios` peuple uniquement les versions **simulées** des applications
fictives connues du prototype ; il ne déclare aucune installation réelle.

## Ajouter une application

1. Copier un objet de `catalog/obs-catalog.json` ou de `catalog/catalog.json`
   dans le tableau `applications` du manifeste choisi.
2. Attribuer un `id` stable et unique : minuscules ASCII, chiffres, tirets,
   1 à 64 caractères, premier caractère alphanumérique. Le conserver lors des
   changements de nom ou de version.
3. Renseigner les métadonnées, la version SemVer disponible, la date et les
   plateformes. Mettre `fictional: false` pour un vrai logiciel.
4. Ajouter les images et les liens dont vous disposez des droits nécessaires.
5. Valider la configuration, puis actualiser la bibliothèque :

```sh
cargo run --locked -- --check-catalog catalog/obs-config.json
```

Sur le poste de développement sans Cargo global :

```sh
./scripts/cargo.sh run --locked -- --check-catalog catalog/obs-config.json
STUDIO_APP_CONFIG="$PWD/catalog/obs-config.json" ./scripts/cargo.sh
```

Dans PowerShell :

```powershell
$env:STUDIO_APP_CONFIG = (Resolve-Path .\catalog\obs-config.json).Path
cargo run --locked
```

Le catalogue OBS est un exemple réel, renseigné d’après la publication officielle
[33.0.1 du 29 octobre 2026](https://github.com/obsproject/obs-studio/releases/tag/33.0.1),
consultée le 9 novembre 2026. Sa version est un instantané explicitement inscrit
au catalogue, pas une promesse de version perpétuellement à jour. Il n’a aucun
package approuvé et ne télécharge pas le logiciel. Les huit autres applications
restent des démonstrations explicitement fictives, désormais décrites en JSON.

## Structure du manifeste

`catalog/schema-v1.json` et `catalog/schema-v2.json` fournissent les JSON Schemas
2020-12 structurels. La validation
Rust est obligatoire : elle ajoute les contraintes d’unicité et de cohérence,
la politique d’URL et de ressources et les plafonds en octets. Les chaînes sont
bornées en UTF-8 côté Rust ; le JSON Schema décrit les limites en caractères.
Les entiers `1` et `2` sont acceptés ; les autres versions sont refusées.
Le schéma 2 ajoute les règles facultatives de détection native, documentées dans
[ETAPE_3.md](ETAPE_3.md). Les anciens manifestes 1 restent lisibles sans migration.
Un ancien launcher des étapes 1/2 refuse le schéma 2 ; actualiser le launcher
avant de publier un manifeste 2, ou conserver un endpoint 1 pour ces clients. Les champs inconnus,
les doublons de propriétés JSON et les types incorrects sont refusés.

Racine : `schema_version`, `name`, `applications` (tableau pouvant être vide).
Chaque application contient :

| Champ | Rôle |
| --- | --- |
| `id`, `name`, `publisher`, `category` | Identité et classement |
| `short_description`, `description` | Résumé des cartes et présentation longue des fiches |
| `color` | Trois entiers RGB de 0 à 255 |
| `monogram` | Repli graphique de 1 à 3 caractères, au plus 8 octets UTF-8 |
| `fictional` | Faux par défaut ; signale une application fictive |
| `logo` | Facultatif/null ; objet `{source, alt}` |
| `screenshots` | Tableau de `{image: {source, alt}, caption}` |
| `features` | Tableau de `{title, description}` |
| `release` | Version disponible explicitement inscrite au catalogue |
| `history` | Anciennes versions uniques, de précédence SemVer inférieure |
| `links` | `website`, `documentation`, `github` facultatifs/null, HTTPS publics |
| `platforms` | OS et architectures déclarés compatibles |
| `github` | Facultatif : `{owner, repository}` pour la découverte de publications |
| `detection` | Schéma 2 : identités macOS/Windows explicites, jamais une commande |

`release` et chaque entrée d’historique contiennent `version` (SemVer sans
préfixe `v`), `published` (date valide `YYYY-MM-DD`), `notes` et `packages`.
Les tableaux facultatifs sont vides par défaut. Les textes sont affichés comme
texte, sans HTML ou interprétation de commandes.

Un package est une déclaration de métadonnées approuvées :

```json
{
  "id": "mon-outil-macos-aarch64",
  "os": "macos",
  "architecture": "aarch64",
  "format": "dmg",
  "url": "https://downloads.example.com/mon-outil-1.0.0.dmg",
  "size_bytes": 12000000,
  "sha256": "REMPLACER_PAR_L_EMPREINTE_HEXADECIMALE_REELLE_DE_64_CARACTERES",
  "github_asset_id": null
}
```

Cet extrait explicatif est volontairement **invalide tant que l’empreinte
réelle n’est pas renseignée**. Ne pas fabriquer de checksum. Le fichier de test
`tests/fixtures/valid.json` utilise une empreinte de données de test et un domaine
réservé d’exemple : ce n’est pas un package distribuable.

OS : `macos`, `windows`, `linux`. Architectures : `aarch64`, `x86_64`.
Formats : `dmg`, `pkg`, `msi`, `exe`, `app_image`, `deb`, `rpm`, `zip`, `tar_gz`.
Les couples OS/architecture doivent figurer dans `platforms` ; les formats natifs
sont vérifiés contre l’OS. Aucune commande, argument de processus ou destination
d’installation n’est acceptée. La présence d’une URL et d’une empreinte ne prouve
pas l’authenticité et ne provoque aucune action système.

## Logos et captures

Seuls PNG et JPEG sont décodés. Les SVG, HTML, `data:`, `file:` et autres formats
ne sont pas chargés. Une ressource HTTPS doit appartenir à un hôte explicitement
autorisé. Dans un catalogue **local**, `source: "capture.png"` désigne uniquement
`assets/capture.png`, relativement au dossier de la configuration. Le dossier
`assets` lui-même ne peut pas être un lien symbolique ; les chemins résolus
restent confinés à ce dossier.

`catalog/assets-config.json` fournit un exemple complet avec une image locale.
Le petit PNG fourni est une mire de test originale, pas une capture de logiciel.
Les vraies applications sans image affichent un monogramme et un emplacement
vide ; les aperçus conceptuels de l’étape 1 restent réservés aux applications
fictives sans capture. Une ressource défaillante n’empêche pas de parcourir le
catalogue ; la fiche signale son indisponibilité.

## Cache et erreurs

Par défaut : dossier de cache utilisateur `ProjectDirs::cache_dir()/catalog-v1`.
`STUDIO_APP_CACHE_DIR` permet de choisir un dossier de cache explicitement local,
notamment pour les tests. Chaque configuration/source/politique est isolée par
une clé SHA-256 : jamais de réutilisation du cache d’une autre source.

Le manifeste est entièrement lu, analysé et validé avant remplacement atomique
via fichier temporaire sur le même volume. Un manifeste réseau/local invalide
ne remplace jamais le dernier cache valide. Chaque lecture du cache est à
nouveau validée. Erreur réseau, HTTP, fichier absent ou manifeste invalide :
repli sur le cache de cette source, avec indication visible. Sans cache valide,
le catalogue est indisponible. Une actualisation échouée conserve aussi la vue
déjà en mémoire. Un échec d’écriture du cache est signalé mais laisse utilisable
un catalogue valablement chargé.

Les images valides sont également persistées et redécodées à la lecture.
Une réponse image corrompue ne remplace pas une image valide. Les images sans
référence sont retirées du cache courant après une nouvelle sauvegarde réussie.
Ce nettoyage est de bonne foi : des permissions insuffisantes peuvent l’empêcher.
Les caches d’autres configurations ne sont pas purgés automatiquement.

| Limite | Valeur |
| --- | --- |
| Configuration | 16 KiB |
| Manifeste / réponse GitHub | 2 MiB, y compris sans Content-Length |
| Applications / ressources distinctes | 128 / 64 |
| Captures / fonctionnalités / historique par application | 6 / 16 / 32 |
| Packages / notes par version | 16 / 64 |
| Un package déclaré | 1 octet à 100 GiB, sans téléchargement |
| Une image compressée | 4 MiB |
| Dimensions / pixels | 4096 par côté / 8 mégapixels |
| Allocation du décodeur | 32 MiB |
| Images RGBA cumulées par chargement | 64 MiB |
| Connexion / requête HTTPS | 4 s / 10 s |
| Phase de ressources | Après 30 s écoulées, lecture du cache uniquement |

Une requête d’image déjà démarrée peut finir après ce seuil de 30 s. La résolution
DNS initiale utilise le résolveur système ; ses délais dépendent de l’OS. En repli
hors ligne, les ressources sont lues directement dans le cache sans réessayer le
réseau pour chaque image. Tous les traitements réseau, fichiers et décodages sont
sur un worker ; egui reçoit le catalogue puis les textures progressivement.

## GitHub : découverte distincte de l’approbation

```sh
cargo run --locked -- --discover obsproject/obs-studio
```

L’adaptateur lit au plus 20 publications publiques via l’API GitHub. Les
`DiscoveredRelease` / `DiscoveredAsset` contiennent les tags, dates, notes et
identifiants des assets. Ce sont des types distincts des `Release` / `Package`
approuvés. Ils ne sont pas fusionnés dans le catalogue ni dans le cache de
catalogue. Aucune URL d’asset découverte n’est téléchargée.

Une publication doit être examinée puis inscrite explicitement au manifeste
avec ses identifiants, plateformes et empreintes avant une éventuelle prise en
charge future. Les limites de débit GitHub et erreurs HTTP sont remontées.
La découverte n’est pas automatiquement lancée à chaque ouverture de fiche.

## Séparation des états

`Application.metadata` contient uniquement les métadonnées du manifeste.
`Application.local` contient uniquement les installations et versions obtenues
par le détecteur natif ; les chemins ne sont jamais chargés depuis un état
installé prétendu par le manifeste. `simulation.version` et `operation` appartiennent
au simulateur en mémoire. Ni le parsing, ni GitHub, ni la fin d’une simulation
ne peuvent déclarer un logiciel réellement installé. Les versions du catalogue
ne sont jamais réécrites par le simulateur.

À l’actualisation, les états en mémoire sont rapprochés par identifiant stable,
pas par position du tableau. La sélection suit un changement d’ordre et se ferme
si l’application disparaît. Les nouvelles entrées apparaissent immédiatement.

Le lancement natif exige une installation effectivement détectée puis revérifiée
au clic. Voir [ETAPE_3.md](ETAPE_3.md) pour les identités et limites. Aucun
téléchargement de package, installation, désinstallation ou auto-update n’est
implémenté.
