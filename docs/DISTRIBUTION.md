# Catalogue de distribution — étape 4.1

Studio App conserve son catalogue dynamique, son cache, sa détection, son moteur
sécurisé et son interface. Ce processus ajoute un **outil éditorial séparé** : les
logiciels gardent chacun leur dépôt GitHub, et leurs releases alimentent une
proposition de catalogue, jamais une installation ni une approbation automatique.

## État livré

- `distribution/registry.json` : registre v1 des seules applications de notre gamme
  explicitement inscrites par les mainteneurs. **Vide** dans cette étape.
- `distribution/registry.example.json` : exemple valide vide, à remplir avec des
  métadonnées réelles. Aucun nom, identifiant ou dépôt inventé n’est présenté comme produit.
- `distribution/catalog.json` : catalogue v2 de distribution, distinct du catalogue
  de démonstration `catalog/catalog.json`, qui reste inchangé et chargé par défaut.
- `distribution/trust.json` : ancre de confiance de production, sans clé configurée.
  Uniquement des **clés publiques** Ed25519, provisionnées par une PR distincte.
- `distribution/approvals.json` : approbations signées des packages courants ; vide.
- `studio-catalog` : synchronisation, demande d’approbation, signature hors ligne et
  contrôle de publication. Ne contient aucun appel d’installation ou d’exécution d’asset.
- Trois workflows : proposition, contrôle de PR et publication des données du catalogue.
  Les deux workflows qui écrivent sont **désactivés par défaut**, sans secrets ni
  variables configurés par cette PR. Aucune vraie application n’est publiée.

Les seules configurations complètes avec noms/versions/identités d’exemple sont
les **fixtures explicitement fictives** de `tests/fixtures/distribution-registry.json`
et les réponses API simulées dans `tests/distribution.rs`. Ne jamais les inscrire
au registre de production ou les publier.

## Enregistrer une application de notre gamme

Procéder d’abord à une PR éditoriale vers `dev`, sans proposition de release dans
la même PR. Renseigner une entrée du registre, revue par un mainteneur :

| Champ | À renseigner avec les données réelles |
| --- | --- |
| `metadata.id` | Identifiant stable, unique, minuscules ASCII/chiffres/tirets, 64 octets maximum ; indépendant du nom et du dépôt |
| `metadata.name`, `publisher` | Nom officiel et éditeur de notre gamme |
| `short_description`, `description`, `category` | Textes des cartes et des fiches |
| `color`, `monogram` | RGB et repli visuel ; aucune modification du dessin des cartes |
| `logo`, `screenshots`, `features`, `links` | Ressources HTTPS publiques autorisées, textes et liens officiels ; `logo: null` et tableaux vides possibles |
| `platforms` | OS et architectures explicitement compatibles, sans déduction du nom d’un asset |
| `detection` | Véritables bundle ID et chemins relatifs macOS, clé Uninstall/éditeur/exécutable Windows ; jamais le seul nom affiché |
| `github.owner`, `repository` | Dépôt indépendant explicitement autorisé ; aucune découverte de nouveaux dépôts |
| `visibility` | `public` ou `private` |
| `allow_private_metadata_in_proposals` | Obligatoirement `true` pour lire un dépôt privé : consentement à inclure ses métadonnées dans une PR du catalogue |
| `channel` | `stable` ou `prerelease` explicitement choisi |
| `tag_prefix` | Préfixe exact des tags, par exemple vide ou celui réellement utilisé ; aucun préfixe deviné |
| `packages` | Un binding par OS/architecture, avec la structure et la recette explicitement approuvées |

La définition éditoriale ne contient **aucune version factice d’initialisation**.
La version, la date et les notes viendront de la release GitHub sélectionnée.
Les champs non listés, les commandes, les tokens et les destinations d’installation
ne sont pas acceptés dans le registre.

Un binding contient :

- `package_id` stable et unique dans l’application ;
- `asset_name`, nom exact ou modèle utilisant seulement `{version}` et `{tag}` ;
- `os`, `architecture`, `format`, jamais inférés du nom ;
- `max_size_bytes`, limite explicite entre 1 octet et 2 Gio ;
- `hash_policy` : `compute` calcule toujours SHA-256 ; `require_github_digest` impose
  aussi la présence du digest GitHub. Lorsqu’un digest GitHub est présent, il doit
  correspondre aux octets téléchargés, dans les deux modes ;
- `recipe`, identique au type de l’étape 4 : `mac_bundle` avec `bundle_name`/`team_id`,
  ou `windows_portable` avec `directory_name`/`authenticode_thumbprint` ;
- `public_url: null`, pour l’URL publique GitHub, ou un modèle HTTPS public explicite
  de miroir, vérifié par un second calcul d’empreinte. Aucun token dans l’URL.

La structure sérialisée de référence est la fixture complète ; ses valeurs sont
uniquement des données de test. L’exemple actif livré reste volontairement vide.
Les contraintes Rust sont la validation normative : schéma v1 strict, au plus
32 applications, 16 bindings par application, une cible OS/CPU non ambiguë.
Les formats préparés correspondent aux recettes existantes ZIP/DMG `.app` et ZIP
portable Windows. PKG/MSI/EXE et l’installation Linux restent désactivés.

Les images d’un catalogue distribué doivent être servies publiquement en HTTPS.
Configurer leurs hôtes autorisés dans la configuration locale du launcher comme
pour le catalogue dynamique. Le processus ne copie pas d’images privées sur un
hébergement public et n’invente aucun logo ni screenshot.

## Synchroniser sans approuver

```sh
cargo build --locked --bin studio-catalog
cargo run --locked --bin studio-catalog -- sync distribution/registry.json distribution/catalog.json target/catalog-proposal.json
cargo run --locked --bin studio-catalog -- validate distribution/registry.json distribution/catalog.json target/catalog-proposal.json
```

Avec le registre livré, la première commande ne fait aucun appel GitHub : elle
produit une proposition vide et aucune application. `sync` ne change ni la baseline,
ni le registre, ni les approbations. Il écrit seulement le fichier de sortie choisi.
Le JSON est borné à 2 Mio et écrit atomiquement après validation complète.

Pour chaque dépôt enregistré, l’outil lit les 20 releases les plus récentes et
sélectionne la version SemVer de plus grande précédence dans le canal configuré.
Les drafts sont ignorées ; le canal stable ignore les prereleases. Des tags
ambigus/incompatibles, une date invalide, une release stable avec un tag prerelease,
un asset manquant ou des IDs dupliqués font échouer la proposition entière.
La fenêtre de 20 releases est volontairement bornée : un dépôt à conventions
incompatibles ou dont la version voulue est hors fenêtre exige un ajustement
éditorial explicite, pas une supposition ou un scan illimité.

Seuls les assets nommés dans les bindings sont téléchargés, **pour les hacher**,
sans extraction, stockage durable ni exécution. La taille réelle doit correspondre
à l’API et à la limite configurée. Le digest GitHub est vérifié quand présent.
Le transfert total est borné à 4 Gio par synchronisation, la lecture API à 2 Mio,
une release à 128 assets. Notes : au plus 64 lignes non vides de 2048 octets.
Les textes restent des données JSON ; aucune note ne devient une commande shell.

Une version déjà connue et inchangée donne zéro modification. Une version plus
récente fait passer la précédente dans l’historique, borné à 32 versions.
Une même version dont les packages, les notes ou la date ont changé est refusée
pour audit manuel. Une release plus ancienne ne provoque jamais un downgrade.
La proposition inclut les empreintes canoniques du registre et de la baseline,
le catalogue candidat, les IDs de releases/assets et les IDs modifiés.

SHA-256 vérifie les octets ; **il n’approuve pas leur provenance**. Le digest fourni
par GitHub ne remplace pas non plus une signature d’approbation.

## Dépôts privés et séparation des accès

Pour une synchronisation locale autorisée, fournir `STUDIO_CATALOG_GITHUB_TOKEN`
uniquement par l’environnement. Ne jamais le passer dans les arguments, le JSON,
une URL, un log ou un fichier versionné. Préférer un jeton d’installation GitHub
App à courte durée, limité à la lecture Contents des seuls dépôts enregistrés.
Un 401/403/404 ou une limitation API produit une erreur sans token ni corps de
réponse. Un dépôt inaccessible ne devient jamais une application vide approuvée.

La visibilité réelle du dépôt est vérifiée via l’API avant de lire ses releases :
un dépôt privé ne peut pas être traité comme public par simple erreur de registre.
L’authentification est envoyée uniquement à `api.github.com`, dont l’URL est
construite à partir des identifiants validés. Pour le téléchargement d’asset,
l’API peut répondre 200 ou 302. Une unique redirection HTTPS est autorisée vers
`release-assets.githubusercontent.com` ou `objects.githubusercontent.com`, avec
un **nouveau client sans Authorization**. Aucun token n’est envoyé à un CDN ou
miroir. Les autres redirections et proxies d’environnement sont refusés ; le DNS
public est validé et épinglé pour chaque connexion.

Les workflows attendent un token valide injecté par le propriétaire. Un token
GitHub App à courte durée exige son renouvellement par votre orchestration ; un
PAT finement limité peut aussi être utilisé avec rotation et expiration gérées.
Cette étape ne configure aucun compte, App ou service de renouvellement de secrets.

Une source privée peut fournir les métadonnées d’un logiciel dont les packages
sont publiés sur un miroir public autorisé : configurer `public_url` pour chaque
binding et contrôler l’égalité des empreintes source/miroir. Sans miroir explicite,
la proposition reste inspectable, mais la publication est refusée. Le launcher
ne reçoit jamais de token GitHub et n’acquiert pas ici de téléchargement privé.
Le consentement aux métadonnées privées est distinct de l’autorisation d’accès :
si la PR cible est publique, les notes et les métadonnées proposées seront visibles
publiquement. Le mainteneur doit l’autoriser en connaissance de cette destination.

## Approbation Ed25519 manuelle

La politique de l’étape 4 est conservée : `ApprovalBody` et le même préfixe de
signature, `Studio App approved installation v1\0`, suivi du JSON compact typé
produit par `installer::approval::signing_message`. Le corps lie toute l’application,
le package exact, la recette et l’expiration. Aucun nouveau schéma du launcher.

Après revue des sources et du package, créer la demande pour chaque package courant :

```text
studio-catalog request <registre> <baseline> <proposition> <id-réel> <package-id-réel> <expiration-unix-future> <demande.json>
studio-catalog sign <demande.json> <clé-privée-Ed25519-PKCS8-hors-dépôt> <id-clé-approuvée> <signature.json>
```

Les chevrons ci-dessus sont des paramètres explicatifs, **pas des identifiants ou
chemins à réutiliser littéralement**. La signature est une action locale explicite
de l’opérateur ; `sync` et les workflows ne l’appellent jamais. Le format de clé
est PKCS8 Ed25519 DER accepté par `ring`, généré/provisionné par votre procédure
hors ligne. Ni génération, ni stockage d’une clé privée de production dans ce dépôt.

Ajouter les objets signés au tableau `approvals` de `distribution/approvals.json`,
avec `schema_version: 1`, `environment: "production"`. Conserver une approbation
exacte pour **chaque package courant** du catalogue, pas seulement le nouveau.
Maximum 128 approbations ; renouveler les signatures expirées avant publication.
Les anciennes signatures ne sont pas réutilisables si les métadonnées changent.
Le fichier d’approbations n’accepte aucune clé de confiance.

```text
studio-catalog verify <registre-de-confiance> <baseline> <proposition> <catalogue-candidat> <approbations> <ancre-publique-de-confiance>
```

Ce contrôle hors ligne refuse les signatures absentes, fausses, expirées ou
ambiguës, les clés non autorisées, une baseline/registre modifiés, un ajout de
source non configurée, un changement d’historique ou une suppression d’application.
Il refuse aussi une tentative sans modification à publier et un catalogue différent
de la proposition. La clé publique doit provenir de l’ancre de confiance revue,
pas de la proposition ou d’un artefact téléchargé.

Les fichiers de confiance et d’approbation ont un environnement explicite.
La vérification de production refuse `environment: "test"`. L’option `--test`
existe seulement pour des validations locales de fixtures ; aucun workflow de
publication ne l’utilise. Les tests génèrent leurs clés éphémères en mémoire,
sans clé privée ou seed statique committé. Les clés de production et de test ne
doivent jamais être réutilisées. Une clé publique nouvelle se provisionne par
une PR séparée, avant toute proposition qui l’utilise.

**Cette validation permet de publier des métadonnées approuvées, pas d’activer
l’installation de production.** Le traitement des redirections d’assets dans cet outil de calcul d’empreinte
ne modifie pas le transport de l’installeur. Le moteur de l’étape 4 continue de refuser le
déploiement de production tant que signatures de code, notarisation/Authenticode
et scénarios natifs réels de notre gamme n’ont pas été validés séparément.

## Workflows et activation future

| Workflow | Fonction | Accès |
| --- | --- | --- |
| `catalog-sync.yml` | Planification quotidienne, déclenchement manuel ou `repository_dispatch: studio_app_release` ; lecture du registre de `dev`, calcul des empreintes, proposition, PR brouillon déterministe | Job de lecture : token source ; job de PR séparé : token cible |
| `catalog-validate.yml` | Contrôle de publication sur PR vers `dev`, hors ligne | Aucun secret ; registre, catalogue de base, clés et binaire de vérification issus de la branche de base de confiance |
| `catalog-publish.yml` | Après fusion manuelle sur `dev` ou déclenchement manuel explicite, revérification puis publication atomique des données | Environnement protégé et token de publication distinct |

Un `repository_dispatch` ne transmet aucune autorisation : son payload est ignoré.
Il réveille la synchronisation, qui relit uniquement les dépôts enregistrés.
Les dépôts logiciels peuvent envoyer ce signal lors de leur événement
`release: published`, avec un accès autorisé au dépôt du catalogue. La planification
quotidienne reste le repli sans webhook à installer dans chaque dépôt.
Aucun déclenchement ni dépôt logiciel réel n’est configuré dans cette étape.

L’automatisation de PR écrit uniquement `distribution/proposal.json` et
`distribution/catalog.json`. Sa branche dépend du hash de la proposition et
n’est jamais réécrite si elle existe : elle peut déjà contenir les signatures
ajoutées par un mainteneur. Elle crée une **PR brouillon**, sans fusion automatique.
Une proposition périmée doit être régénérée et revue ; aucun historique humain
n’est écrasé. Sans modification, aucun jeton d’écriture ni PR n’est nécessaire.

Le contrôle de PR construit et exécute le vérificateur de la **base de confiance**,
pas celui qu’une proposition pourrait modifier. Une nouvelle application et une
nouvelle clé doivent donc être enregistrées dans une PR éditoriale préalable.
L’amorçage de cette PR 4.1 ne peut contenir qu’un registre/catalogue/ancres/approbations
vides, sans produit ni clé. Aucune signature ne peut ainsi s’auto-approuver en
ajoutant une clé dans sa propre PR de publication.

Pour l’activation future par le propriétaire :

1. Faire intégrer les workflows par le processus normal. Les événements planifiés
   et `repository_dispatch` exigent leur présence sur la branche par défaut ; une
   PR vers `dev` seule ne les active pas si `dev` n’est pas la branche par défaut.
2. Protéger `dev` : revue humaine et contrôles obligatoires Rust CI + Offline Ed25519
   publication gate. Réserver les modifications du registre, des ancres et workflows
   aux mainteneurs. Aucune de ces protections n’est modifiée par cette PR.
3. Provisionner les clés publiques de production, puis enregistrer les véritables
   applications par des PR éditoriales distinctes. Garder les clés privées hors ligne.
4. Configurer `CATALOG_SOURCE_READ_TOKEN` pour lire les dépôts logiciels autorisés,
   `CATALOG_PR_TOKEN` pour créer des PR dans ce dépôt et `CATALOG_PUBLISH_TOKEN`
   pour la seule publication des données. Jetons GitHub App distincts recommandés,
   permissions Contents read / Contents+Pull requests write / Contents write selon
   le rôle. Aucun de ces jetons n’est une clé Ed25519.
5. Protéger l’environnement `catalog-production` : branches autorisées, revue
   requise lorsqu’elle est disponible dans l’offre GitHub, secrets restreints.
   Sa présence seule ne garantit pas qu’une protection a été configurée.
6. Activer `CATALOG_SYNC_ENABLED=true` seulement après cette préparation.
   Inspecter la PR produite, vérifier les octets/provenance, signer hors ligne,
   ajouter les approbations et exiger tous les contrôles avant fusion manuelle.
7. Activer `CATALOG_PUBLICATION_ENABLED=true` seulement quand l’hébergement et le
   processus d’approbation ont été validés. Le workflow relit la baseline déjà
   publiée, refuse une proposition périmée et vérifie les signatures avant sa
   première mutation distante.

La publication préparée crée/met à jour la branche de **données** `catalog-published`
avec seulement `catalog.json` et `approvals.json`, dans un même commit. Une mise à
jour concurrente est refusée sans force push. Elle ne crée aucune GitHub Release,
ne fusionne aucune PR et n’exécute aucun package. Le fichier d’ancres n’est pas
publié comme source automatique de confiance.

Le launcher peut ensuite lire le catalogue depuis un endpoint HTTPS direct public
configuré par l’opérateur. Une branche dans un dépôt privé ne devient pas un
endpoint public par magie : prévoir l’hébergement autorisé approprié. Les clients
reçoivent leurs clés de confiance par un canal local distinct ; le cache et le
simulateur du launcher restent inchangés. Publier les versions approuvées dans
l’ordre : un décalage entre la baseline source et la branche publiée bloque la
publication jusqu’à résolution explicite, plutôt qu’un écrasement silencieux.

## Validation de cette étape

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --all-targets --all-features
python3 -m unittest discover -s tests -p 'test_catalog_automation.py'
```

Les nouveaux tests utilisent exclusivement des réponses API simulées et des mocks
de mutations GitHub : nouvelle release, release connue, prerelease, draft, asset
manquant, OS/CPU incompatibles, tag invalide, dépôt privé inaccessible, erreur API,
source usurpée, mauvais digest, miroir différent, publication sans approbation,
signature expirée/fausse/inconnue, séparation test/production et proposition altérée.
Les commandes CLI de préparation et signature hors ligne sont aussi exercées avec
une clé de test temporaire. Les workflows sont exercés via leurs fonctions Python
hors ligne : absence d’effet de bord, PR brouillon, préservation des signatures
humaines, refus avant mutation et mise à jour atomique sans force push.

La CI Rust continue de couvrir macOS, Windows et Ubuntu, y compris les tests
natifs isolés de l’étape 4. Aucun test de cette étape n’accède à un vrai dépôt
logiciel ni ne publie une vraie application. Les workflows de synchronisation et
publication ne sont pas activés en production et ne sont pas présentés comme des
déploiements réellement exécutés : leur orchestration est testée hors ligne.

## Références GitHub

Les APIs documentent les [releases](https://docs.github.com/en/rest/releases/releases)
et les [assets, digests, téléchargements 200/302 et permissions Contents read](https://docs.github.com/en/rest/releases/assets).
Le digest GitHub sert au contrôle des octets, jamais à l’approbation Ed25519.

Les événements d’une PR créée avec `GITHUB_TOKEN` peuvent demander une autorisation
avant exécution des workflows ; un token GitHub App dédié est prévu pour le flux
PR. Voir [GITHUB_TOKEN](https://docs.github.com/en/actions/concepts/security/github_token)
et [déclenchement des workflows](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow).
Les possibilités de protection des environnements dépendent de l’offre et de la
visibilité du dépôt : [documentation des environnements](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).
