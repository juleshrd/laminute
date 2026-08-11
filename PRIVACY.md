# Politique de confidentialité — La Minute

_Dernière mise à jour : août 2026_

## Données stockées localement

La Minute est une application **desktop** : vos réunions, fichiers audio importés ou enregistrés, transcriptions et comptes-rendus sont enregistrés **uniquement sur votre ordinateur**, dans le dossier local que vous choisissez. Sans personnalisation, le répertoire de données de l'application est utilisé :

- **macOS** : `~/Library/Application Support/app.laminute.desktop/`
- **Linux** : `~/.local/share/app.laminute.desktop/`
- **Windows** : `%APPDATA%\app.laminute.desktop\`

Ce répertoire contient notamment :

| Élément               | Fichier / dossier |
| --------------------- | ----------------- |
| Base de données       | `laminute.db`     |
| Imports MP3           | `imports/`        |
| Enregistrements micro | `recordings/`     |
| Journaux locaux       | `logs/`           |

Au premier lancement et depuis **Réglages → Confidentialité et données locales**, vous pouvez choisir un autre emplacement. La Minute crée un sous-dossier `La Minute` dans le dossier parent sélectionné. La base, les réglages applicatifs et les audios sont copiés puis vérifiés avant la bascule ; l’ancien emplacement n’est supprimé qu’après validation. Toute donnée résiduelle impossible à supprimer est signalée.

Un petit fichier `storage-config.json` reste dans le répertoire système indiqué ci-dessus afin que l’application retrouve le dossier choisi au prochain démarrage. Il contient uniquement le chemin local et une version de format. Les clés API restent dans le trousseau sécurisé du système et ne sont pas déplacées avec les données.

Si le dossier choisi devient inaccessible — disque externe déconnecté, permissions retirées ou volume indisponible — La Minute bloque l’accès au stockage et affiche une erreur explicite au lieu de créer silencieusement une nouvelle base ailleurs.

Les données y restent **jusqu'à ce que vous les supprimiez** (réunion par réunion ou effacement complet depuis les réglages « Confidentialité »).

Le **stockage** est toujours local. Le **traitement** (transcription / compte-rendu) dépend du fournisseur IA configuré.

## Fournisseurs IA

### Mistral et OpenAI (BYOK)

Si vous configurez une clé API Mistral ou OpenAI :

- **Transcription** : le fichier audio de la réunion est envoyé à l'API du fournisseur pour produire le texte.
- **Compte-rendu structuré** : uniquement le **texte** (transcrit ou collé) est envoyé pour générer la synthèse.

Votre clé API est :

- **au repos** : stockée dans le **trousseau système** (Secret Service / Keychain / Credential Manager), jamais en clair dans la base ni dans les exports ;
- **en transit** : transmise au fournisseur via **TLS** dans l'en-tête d'authentification (`Authorization: Bearer …`), jamais intégrée au contenu audio/texte ni aux exports.

La Minute n'initie ces appels que lorsque vous déclenchez une transcription ou un compte-rendu.

### Ollama

- **URL loopback** (`127.0.0.1`, `localhost`, `::1`) : le compte-rendu est généré sur votre machine ; aucune donnée n'est envoyée à un service cloud.
- **URL distante / LAN** : uniquement avec opt-in explicite ; le texte du compte-rendu est envoyé au serveur Ollama configuré. Aucune clé API n'est transmise.
- La transcription audio n'est pas disponible via Ollama.

## Export et suppression

- **Export** : vous pouvez exporter une réunion (métadonnées, transcription, compte-rendu, actions) au format JSON, Markdown ou PDF depuis l'historique. L'export ne contient pas de clé API ni de chemin absolu vers vos fichiers.
- **Suppression** : vous pouvez supprimer une réunion (données et fichier audio associé) ou effacer **toutes** les données locales depuis l'application (base, imports, enregistrements, réglages, **journaux** `logs/`). L'effacement complet ne supprime pas automatiquement les clés API du trousseau ; vous pouvez les retirer manuellement dans les réglages IA.

## Diagnostic local et bundle de support

Depuis **Réglages → Diagnostic et support**, La Minute affiche un état de santé local (version, système, chemins, schéma de base, fournisseur / modèles, trousseau, micro, updater, derniers codes d'erreur).

Les **journaux locaux** (`logs/`) sont bornés (rotation et plafond disque). Ils enregistrent des **codes d'erreur**, messages techniques courts et identifiants de corrélation — **jamais** de clé API, de transcription, de compte-rendu ni de contenu audio.

Vous pouvez **volontairement** générer un bundle ZIP de support :

- un **aperçu exact** du contenu est affiché avant toute sauvegarde ;
- le ZIP ne contient que des métadonnées de diagnostic, la queue des journaux déjà expurgée, et un rapport court pour GitHub Issues ;
- aucune donnée de réunion ni secret n'est inclus par défaut (contrôlé par tests).

Le bouton **Copier le rapport GitHub** place un résumé textuel (version / configuration / codes) dans le presse-papiers, sans quitter votre machine.

## Enregistrement et tiers

L'enregistrement via microphone peut capturer la voix d'autres participants. **Informez-les** et obtenez leur accord avant d'enregistrer une réunion.

## Télémétrie

La Minute **n'envoie aucune télémétrie** ni donnée d'usage à ses développeurs. Seuls les appels que vous initiez vers le fournisseur IA configuré (Mistral, OpenAI, ou Ollama distant) quittent votre machine. Le diagnostic et le bundle de support restent **strictement locaux** jusqu'à ce que *vous* choisissiez de les partager (fichier ou presse-papiers).

## Licence

La Minute est distribuée sous licence **GPL-3.0** — voir [LICENSE](LICENSE).

Pour toute question : consultez le résumé intégré dans l'application (section « Confidentialité ») ou ce fichier dans le dépôt source.
