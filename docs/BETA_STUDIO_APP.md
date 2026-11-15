# Bêta de Studio App avec Hyper Media Puller

Les paquets contiennent le launcher et son catalogue par défaut. Au démarrage,
Studio App consulte les releases publiques de HMP. Le bouton **Installateur**
ouvre la page GitHub de la version compatible. Suivre la procédure de HMP pour
l’installation et la mise à jour, puis actualiser la bibliothèque. Voir
[l’intégration HMP](HYPER_MEDIA_PULLER.md).

| Plateforme | Fichier | Utilisation |
| --- | --- | --- |
| macOS Apple Silicon | `Studio-App-macos-arm64.dmg` | Monter le DMG et placer Studio App dans Applications. |
| Windows x64 | `Studio-App-windows-x64.zip` | Extraire l’archive entière et lancer `studio-app.exe` en conservant le dossier `catalog` à côté. |

Les artefacts de CI servent aux essais. Le bundle macOS est signé ad hoc et
le binaire Windows est sans Authenticode. Les systèmes peuvent bloquer leur
ouverture. Pour une publication signée, utiliser les signatures de l’éditeur
et valider la notarisation macOS sur les fichiers à publier.

## Qualification des artefacts

- Télécharger les deux artefacts du workflow **Studio App beta packages**
  sur le SHA à qualifier. Extraire les enveloppes ZIP de GitHub Actions et
  vérifier les paquets avec leur fichier `SHA256SUMS.txt` respectif.
- Le DMG contient `Studio App.app` avec un exécutable Mach-O arm64. Le ZIP
  Windows contient `Studio App/studio-app.exe`, un exécutable PE x64. Chaque
  paquet contient les fichiers `config.json`, `catalog.json` et
  `hyper-media-puller.json`. La fiche HMP est résolue à partir de cette
  configuration et des métadonnées publiques GitHub. Les installateurs de
  HMP restent dans sa release.
- Sur macOS, la CI monte le DMG en lecture seule et vérifie la signature ad
  hoc du bundle. Sur Windows, elle extrait le ZIP. Chaque job exécute le
  binaire distribué avec `--check-catalog`, sans Cargo dans le PATH, depuis
  un dossier extérieur au paquet et sans fournir de chemin de configuration.
  Ce test utilise la même recherche de configuration que l’interface au
  démarrage. Il valide la lecture du catalogue et le code de sortie du
  programme.

### Essais interactifs sur poste physique

- Démarrer le DMG téléchargé depuis le Finder sur un Mac Apple Silicon,
  puis démarrer l’application copiée dans Applications. Vérifier le
  comportement de Gatekeeper et la présence de HMP dans la bibliothèque.
- Extraire le ZIP téléchargé sur un poste Windows x64, ouvrir
  `studio-app.exe` depuis l’Explorateur et vérifier l’affichage de la
  bibliothèque et le comportement de SmartScreen.
- Sur les deux postes, vérifier que le bouton ouvre la bonne release HMP.
  Après installation manuelle de HMP, vérifier sa détection et son ouverture
  depuis Studio App. Documenter les résultats séparément des tests CI.

## Préparer une publication

1. Exécuter le workflow **Studio App beta packages**. Il produit les deux
   paquets et leur SHA-256 sur le commit construit, puis vérifie la lecture
   du catalogue depuis les paquets montés ou extraits. Le workflow peut
   s’exécuter sur la PR qui le modifie. Le déclenchement manuel est disponible
   après son intégration à la branche par défaut.
2. Qualifier les fichiers signés sur macOS Apple Silicon et Windows x64.
   Ouvrir Studio App depuis le Finder et l’Explorateur, vérifier que HMP
   apparaît et que le bouton ouvre la bonne release. Installer HMP avec le
   guide officiel, actualiser la bibliothèque et vérifier la détection et
   l’ouverture. Si une version compatible plus récente est disponible,
   vérifier le lien de mise à jour depuis une installation antérieure.
3. Recalculer les SHA-256 après signature et vérifier qu’ils correspondent
   aux fichiers distribués. Créer la préversion GitHub de Studio App sur le
   commit approuvé et joindre les deux paquets définitifs et leurs empreintes.
