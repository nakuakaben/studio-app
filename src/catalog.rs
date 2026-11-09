//! Fictional embedded catalog. Remote loading and caching belong to stage two.
use crate::domain::{AppState, Application, Feature};
pub fn demonstration() -> Vec<Application> {
    let entries = [
        ("sequence", "Sequence", "Montage vidéo et étalonnage", "Du premier plan au dernier raccord. Un espace de création pour donner du rythme à vos histoires.", [173, 133, 255], "Sq", "1.8.0", Some("1.6.0"), 248, AppState::UpdateAvailable),
        ("prisme", "Prisme", "Photographie et développement", "La lumière, la couleur, le détail. Retrouvez toute la latitude de vos images dans un atelier photographique précis.", [255, 184, 58], "Pr", "2.4.0", Some("2.3.0"), 186, AppState::Downloading { progress: 0.44, paused: true }),
        ("onde", "Onde", "Création sonore et mixage", "Enregistrez une idée. Construisez un univers. Un environnement dédié à la création et au mixage sonore.", [83, 181, 255], "On", "1.2.0", Some("1.1.0"), 124, AppState::Installing { progress: 0.38 }),
        ("forme", "Forme", "Illustration vectorielle", "Des tracés simples aux compositions complexes. Dessinez librement avec des outils qui suivent votre geste.", [255, 108, 85], "Fo", "3.1.0", Some("3.1.0"), 96, AppState::Installed),
        ("folio", "Folio", "Mise en page et édition", "Mettez vos idées en page. Composez des publications avec un contrôle précis du texte et de la grille.", [117, 207, 168], "Fl", "1.5.0", None, 88, AppState::NotInstalled),
        ("trame", "Trame", "Interfaces et prototypage", "Des premières intentions aux parcours interactifs. Un atelier pour concevoir des interfaces cohérentes.", [231, 142, 203], "Tr", "0.9.0", None, 112, AppState::NotInstalled),
        ("relief", "Relief", "Modélisation et création 3D", "Explorez le volume, façonnez la matière et composez des scènes dans un espace de travail essentiel.", [144, 162, 255], "Re", "2.0.0", None, 312, AppState::NotInstalled),
        ("lueur", "Lueur", "Animation et effets visuels", "Animez une composition, modulez un mouvement, faites vivre vos images. Chaque image compte.", [102, 211, 219], "Lu", "1.3.0", None, 164, AppState::Error { message: "Transfert fictif interrompu. Réessayez pour explorer le parcours de démonstration.".into() }),
    ];
    entries.into_iter().map(|(id, name, category, description, color, monogram, version, installed_version, size_mb, state)| {
        let features = if id == "sequence" { vec![
            Feature { title: "Une timeline familière", description: "Organisez vos pistes, affinez vos raccords et naviguez dans le montage avec précision." },
            Feature { title: "La couleur au premier plan", description: "Courbes, roues chromatiques et scopes réunis dans un espace de travail lisible." },
            Feature { title: "Le son de votre histoire", description: "Mixez les dialogues, les ambiances et la musique au plus près de l’image." },
            Feature { title: "De l’idée à la livraison", description: "Préparez plusieurs formats de sortie à partir d’une même composition." },
        ] } else { vec![
            Feature { title: "Un atelier à votre mesure", description: "Un espace clair et des outils contextuels pour rester concentré sur la création." },
            Feature { title: "Un travail toujours précis", description: "Ajustez chaque détail et retrouvez vos réglages dans un environnement cohérent." },
            Feature { title: "Des projets indépendants", description: "Le logiciel cible restera utilisable sans Studio App une fois installé." },
        ] };
        let idle_state = match &state {
            AppState::Downloading { .. } | AppState::Installing { .. } => AppState::UpdateAvailable,
            AppState::Error { .. } => AppState::NotInstalled,
            other => other.clone(),
        };
        Application { id, name, category, description, color, monogram, version, installed_version, size_mb, features,
            notes: vec!["Nouvel espace de travail et navigation simplifiée.", "Amélioration de la fluidité sur les projets complexes.", "Ajustements de lisibilité et de précision des outils."], state, idle_state }
    }).collect()
}
