# JUL-165 — Vérification des APIs IA

Document d’écarts et correctifs pour la couche fournisseurs (Mistral, OpenAI, Ollama).

## Pipeline attendu

| Étape               | Mistral                                           | OpenAI                                                                 | Ollama                |
| ------------------- | ------------------------------------------------- | ---------------------------------------------------------------------- | --------------------- |
| Transcription audio | Voxtral (`voxtral-mini-latest`)                   | GPT audio (`gpt-4o-mini-transcribe`, `gpt-4o-transcribe`, `whisper-1`) | Non supporté          |
| Diarisation         | `diarize=true` + segments locuteurs               | Modèle dédié `gpt-4o-transcribe-diarize` + `diarized_json`             | —                     |
| Compte-rendu LLM    | `mistral-small-latest` ou `mistral-medium-latest` | `gpt-4o-mini`, `gpt-4o`, `gpt-4.1-mini`, `gpt-4.1`                     | Modèle local installé |

Les préférences (modèles + diarisation) sont persistées dans `ai-settings.json` (pas dans le trousseau).

## Écarts constatés avant correctif

1. **Un seul modèle implicite** — transcription et compte-rendu utilisaient des constantes hardcodées ; l’UI ne proposait aucun choix.
2. **OpenAI audio obsolète** — défaut `whisper-1` au lieu des modèles GPT Transcribe recommandés.
3. **Pas de diarisation** — ni flag Mistral `diarize`, ni modèle OpenAI diarize.
4. **Liste `/models` brute** — la validation de clé renvoyait des centaines de modèles non filtrés ; inutile pour le produit.
5. **Pas de séparation audio / LLM** — un seul fournisseur global (conservé), mais sans choix de modèle par étape.

## Correctifs livrés

- Catalogue produit de modèles (`model_catalog.rs`) distinct de `GET /models`.
- Réglages UI : modèle audio, modèle compte-rendu, toggle diarisation.
- Commande `set_model_preferences` + wiring dans `transcribe_audio_file` / `generate_structured_summary`.
- Mistral : `diarize` + formatage des segments `speaker_id`.
- OpenAI : défaut `gpt-4o-mini-transcribe` ; si diarisation → `gpt-4o-transcribe-diarize` + `chunking_strategy=auto`.

## Limites connues (VM / runtime)

- **Pas de micro** en VM headless : l’enregistrement live n’est pas testable ; l’import MP3 + transcription API oui (avec clé réelle).
- **Trousseau** : sans Secret Service déverrouillé, `save_api_key` / `validate_api_key` peuvent échouer ; les tests keyring restent `#[ignore]`.
- **Appels réseau réels** : les tests CI mockent HTTP (`wiremock`) ; une validation manuelle avec clés BYOK reste recommandée pour confirmer quotas / formats audio.

## Checklist manuelle (avec clé API)

1. Onboarding ou Réglages → enregistrer clé Mistral → Valider.
2. Choisir Voxtral + Mistral Small, traiter un MP3 ≥ 1 s → transcription + CR.
3. Activer diarisation → retraiter → labels locuteurs dans le texte.
4. Passer à Mistral Medium → nouveau CR.
5. Répéter avec OpenAI : choisir modèle audio + LLM ; activer diarisation (modèle diarize auto).
6. Ollama : coller un texte → CR uniquement (pas de transcription).
