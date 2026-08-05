# AGENTS.md

## Cursor Cloud specific instructions

La Minute est une application **desktop Tauri 2** (backend Rust dans `src-tauri/`, frontend React + TypeScript + Vite dans `src/`). Les commandes standard sont dans `package.json` (`dev`, `lint`, `test`, `build`, `check`) et documentées dans le `README.md` ; référez-vous-y plutôt que de les dupliquer.

### Toolchain

- Le backend nécessite **Rust stable ≥ 1.85** : une dépendance transitive (`dlopen2_derive`) exige `edition2024`. La toolchain `stable` est déjà installée et définie par défaut (`rustup default stable`) dans le snapshot ; ne repassez pas sur Rust 1.83.
- Les dépendances système Tauri Linux (webkit2gtk 4.1, GTK3, ALSA, librsvg, appindicator, `xvfb`, `dbus`, `gnome-keyring`) sont installées dans le snapshot.

### Lancer l'application

- L'app est une fenêtre native GTK/WebKit, pas une page web : elle a besoin d'un display X. Un serveur X est disponible sur `DISPLAY=:1` (celui que computer-use observe).
- Lancer en dev : `DISPLAY=:1 XDG_RUNTIME_DIR=/tmp/runtime-ubuntu npm run dev`. Vite sert le frontend sur le port 1420 (port fixe), puis Tauri ouvre la fenêtre « La Minute ».
- Les avertissements `libEGL warning: DRI3 ...` au démarrage sont normaux (rendu logiciel, pas d'accélération GPU en VM) et n'empêchent pas l'app de tourner.

### Comportements attendus en VM headless

- **Aucun périphérique audio** : les sections « Microphone » et « Transcription » affichent « aucun périphérique d'entrée audio détecté ». C'est normal (pas de micro dans la VM) et cela **ne bloque pas** l'import MP3 ni les fonctions base de données. L'enregistrement micro n'est donc pas testable ici.
- **Trousseau / clés API IA** : les clés API (BYOK Mistral) sont stockées via le Secret Service (`keyring`). Sans trousseau déverrouillé, enregistrer/valider une clé peut échouer ; les tests Rust qui touchent le trousseau sont annotés `#[ignore]` et ne s'exécutent pas par défaut. Les flux cœur (import MP3, création de réunion, compte-rendu à partir d'un texte collé) ne nécessitent pas de trousseau.

### Données applicatives

- La base SQLite et les fichiers importés sont sous `~/.local/share/app.laminute.desktop/` (`laminute.db`, dossier `imports/`). Supprimez ce dossier pour repartir d'un état vierge.

### Tâche de fumée (hello-world) validée

Importer un fichier MP3 valide (≥ 1 s) via « Choisir un fichier MP3 » crée une réunion : le panneau « Réunion créée » s'affiche et une ligne est insérée dans les tables `meetings` / `audio_files`. C'est le moyen le plus simple de vérifier le pipeline frontend → commande Tauri → SQLite de bout en bout.
