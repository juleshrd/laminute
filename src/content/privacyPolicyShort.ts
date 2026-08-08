/** Résumé affiché dans l'application — voir PRIVACY.md pour le texte complet. */
export const PRIVACY_POLICY_SHORT = `La Minute stocke vos réunions uniquement sur cet ordinateur (base SQLite, dossiers imports/ et recordings/ sous le répertoire de données de l'application).

Fournisseurs IA :
- Mistral / OpenAI (BYOK) : la transcription envoie le fichier audio ; le compte-rendu envoie le texte. La clé API est transmise via TLS pour authentification, stockée dans le trousseau système, jamais dans les exports ni le contenu métier.
- Ollama : traitement local uniquement si l'URL est en loopback (127.0.0.1 / localhost). Une URL distante (opt-in) envoie le texte au serveur configuré.

Vous pouvez exporter une réunion en JSON ou tout effacer depuis cette section. Informez les participants avant d'enregistrer.

Aucune télémétrie. Licence GPL-3.0.`;
