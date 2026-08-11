# Stockage local et migration

La Minute utilise une racine de stockage unique pour toutes les données applicatives ordinaires :

- `laminute.db` : réunions, transcriptions, résumés, actions et métadonnées audio ;
- `imports/` et `recordings/` : fichiers audio gérés par l’application ;
- `ai-settings.json` et `audio-settings.json` : préférences non secrètes.

Les clés API restent dans le trousseau sécurisé du système. Les exports restent à l’emplacement choisi séparément par l’utilisateur.

## Emplacement

Sans choix explicite, la racine est le répertoire de données fourni par Tauri pour `app.laminute.desktop`. Quand l’utilisateur sélectionne un autre dossier parent, La Minute crée et utilise son sous-dossier `La Minute` afin de ne jamais mélanger ses fichiers avec des documents existants.

Le répertoire système conserve alors uniquement `storage-config.json`, un pointeur versionné vers la racine choisie. Cette exception est affichée dans l’interface et documentée dans `PRIVACY.md`.

## Changement de dossier

Avant confirmation, l’application :

1. refuse les liens symboliques, les imbrications avec la racine actuelle et les destinations non vides ;
2. vérifie l’accès en écriture avec un fichier temporaire immédiatement supprimé ;
3. mesure les données à déplacer et l’espace disponible, avec une marge libre de 10 Mio.

Après confirmation, les nouvelles opérations longues sont bloquées et les traitements en cours sont invalidés. L’enregistrement actif est arrêté. La Minute copie les fichiers sans suivre de liens symboliques, stabilise SQLite, ouvre la copie, réécrit les chemins audio absolus et exécute `PRAGMA integrity_check`.

La racine active, la connexion SQLite, les réglages IA, les chemins audio et la portée du protocole média sont basculés uniquement après ces vérifications. L’ancien emplacement est ensuite supprimé. Si ce dernier nettoyage échoue, la nouvelle racine reste active et l’interface indique précisément que des données résiduelles doivent être supprimées manuellement.

## Dossier indisponible

Une racine personnalisée contient un marqueur `.laminute-storage.json`. Au démarrage, La Minute exige que le dossier et ce marqueur soient présents, lisibles et accessibles en écriture. Un disque déconnecté ou des permissions retirées produit une erreur explicite ; l’application ne crée pas silencieusement une base vide dans le répertoire par défaut.
