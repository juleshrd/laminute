/** Résumé affiché dans l'application — voir PRIVACY.md pour le texte complet. */
export const PRIVACY_POLICY_SHORT = `La Minute stocke vos réunions uniquement dans le dossier local que vous choisissez (base SQLite, dossiers imports/ et recordings/). Par défaut, il s’agit du répertoire de données de l’application.

Si vous choisissez un autre emplacement, un petit fichier de configuration reste dans le répertoire système de l’application afin que La Minute retrouve ce dossier au démarrage. Lors d’un changement, la copie et la base sont vérifiées avant la suppression de l’ancien emplacement ; tout résidu impossible à supprimer est signalé.

Fournisseurs IA :
- Mistral / OpenAI (BYOK) : la transcription envoie le fichier audio ; le compte-rendu envoie le texte. La clé API est transmise via TLS pour authentification, stockée dans le trousseau système, jamais dans les exports ni le contenu métier.
- Ollama : traitement local uniquement si l'URL est en loopback (127.0.0.1 / localhost). Une URL distante (opt-in) envoie le texte au serveur configuré.

Vous pouvez exporter une réunion en JSON ou tout effacer depuis cette section. Informez les participants avant d'enregistrer.

Un écran Diagnostic local (journaux bornés, bundle ZIP volontaire expurgé) reste sur votre machine jusqu'à partage manuel.

Aucune télémétrie. Licence GPL-3.0.`;
