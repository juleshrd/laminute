# Politique de confidentialité — La Minute

_Dernière mise à jour : août 2026_

## Données stockées localement

La Minute est une application **desktop** : vos réunions, fichiers audio importés ou enregistrés, transcriptions et comptes-rendus sont enregistrés **uniquement sur votre ordinateur**, dans le répertoire de données de l'application :

- **macOS** : `~/Library/Application Support/app.laminute.desktop/`
- **Linux** : `~/.local/share/app.laminute.desktop/`
- **Windows** : `%APPDATA%\app.laminute.desktop\`

Ce répertoire contient notamment :

| Élément               | Fichier / dossier |
| --------------------- | ----------------- |
| Base de données       | `laminute.db`     |
| Imports MP3           | `imports/`        |
| Enregistrements micro | `recordings/`     |

Les données y restent **jusqu'à ce que vous les supprimiez** (réunion par réunion ou effacement complet depuis les réglages « Confidentialité »).

## Fournisseur IA (BYOK — Mistral)

Si vous configurez une clé API Mistral :

- **Transcription** : le fichier audio de la réunion est envoyé à l'API Mistral pour produire le texte.
- **Compte-rendu structuré** : uniquement le **texte transcrit** est envoyé à Mistral pour générer la synthèse.

Votre clé API est stockée dans le **trousseau système** (Secret Service / Keychain / Credential Manager), jamais en clair dans la base ni dans les exports. La Minute n'a pas accès à vos serveurs Mistral en dehors de ces appels que vous déclenchez.

## Export et suppression

- **Export** : vous pouvez exporter une réunion (métadonnées, transcription, compte-rendu, actions) au format JSON depuis l'historique. L'export ne contient pas de clé API ni de chemin absolu vers vos fichiers.
- **Suppression** : vous pouvez supprimer une réunion (données et fichier audio associé) ou effacer **toutes** les données locales depuis l'application. L'effacement complet ne supprime pas automatiquement les clés API du trousseau ; vous pouvez les retirer manuellement dans les réglages IA.

## Enregistrement et tiers

L'enregistrement via microphone peut capturer la voix d'autres participants. **Informez-les** et obtenez leur accord avant d'enregistrer une réunion.

## Télémétrie

La Minute **n'envoie aucune télémétrie** ni donnée d'usage à ses développeurs. Seuls les appels que vous initiez vers Mistral (si configuré) quittent votre machine.

## Licence

La Minute est distribuée sous licence **GPL-3.0** — voir [LICENSE](LICENSE).

Pour toute question : consultez le résumé intégré dans l'application (section « Confidentialité ») ou ce fichier dans le dépôt source.
