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
