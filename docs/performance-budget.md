# Budget de performance (JUL-187)

Objectifs mesurables pour éviter les régressions de taille et de démarrage.
Les seuils CI portent sur le **binaire release Linux** (environnement reproductible).

## Profil release Rust

Dans `src-tauri/Cargo.toml` :

| Option          | Valeur | Intention                          |
| --------------- | ------ | ---------------------------------- |
| `lto`           | `true` | Réduction taille / inlining global |
| `codegen-units` | `1`    | Meilleures optimisations           |
| `strip`         | `true` | Retirer les symboles               |
| `opt-level`     | `"s"`  | Optimiser pour la taille           |

## Budgets CI (Linux)

| Métrique                     | Seuil max | Notes                                        |
| ---------------------------- | --------- | -------------------------------------------- |
| Binaire `laminute` (release) | 45 MiB    | `src-tauri/target/release/laminute`          |
| Artefact frontend `dist/`    | 3 MiB     | Somme des fichiers après `npm run build:web` |

Les installateurs (AppImage / DMG / NSIS) sont mesurés et publiés avec les checksums de release ; ils varient fortement selon l’OS (WebView / bundling) et ne sont pas encore des seuils hard CI multi-OS.

## Baseline Linux (agent Cloud, 2026-08-08)

Mesures après activation du profil release (`lto` + `codegen-units=1` + `strip` + `opt-level=s`) et alignement `reqwest` 0.13 :

| Métrique           | Valeur mesurée                                        |
| ------------------ | ----------------------------------------------------- |
| Binaire release    | 12 717 992 octets (~12,1 MiB)                         |
| Frontend `dist/`   | 279 612 octets (~273 KiB)                             |
| Cold start fenêtre | dépend du display / WebView ; smoke sous `DISPLAY=:1` |
| RSS idle           | observation manuelle via `ps` (hors CI)               |

Procédure locale :

```bash
npm run build:web
cargo build --manifest-path src-tauri/Cargo.toml --release
stat -c '%s' src-tauri/target/release/laminute
DISPLAY=:1 XDG_RUNTIME_DIR=/tmp/runtime-ubuntu npm run build:bundle   # optionnel
```

## Nettoyage livré avec ce budget

- Assets Vite/Tauri morts retirés de `public/`
- `removeUnusedCommands` déjà actif
- Plugin opener déjà absent des capacités
- Alignement `reqwest` 0.13 pour réduire le double graphe avec l’updater

## Régression RAM — contenus de réunion (JUL-196)

Le chargement du détail de réunion ne sérialise désormais que les métadonnées
des transcriptions et comptes-rendus. Les corps complets passent par des
commandes dédiées (`get_latest_summary` et `get_latest_transcription`) après
l’ouverture de l’onglet concerné. Les exports continuent d’utiliser le chemin
interne complet, sans le faire transiter par la WebView.

Protocole de mesure (5 transcriptions de 1 Mio chacune) :

1. Créer une réunion de test avec cinq versions de transcription de 1 048 576
   octets et cinq résumés.
2. Mesurer la taille JSON de la réponse `get_meeting` et le RSS WebView après
   l’ouverture de l’onglet **Essentiel**, puis **Audio**.
3. Ouvrir **Transcription** et relever la taille de `get_latest_transcription`.

Attendu : `get_meeting` reste indépendant des 5 Mio de transcriptions et les
onglets **Essentiel** et **Audio** ne demandent jamais `get_latest_transcription`.
L’onglet **Transcription** ne charge qu’une seule version. Le test repository
`get_detail_excludes_heavy_content_and_full_detail_keeps_it_for_exports`
vérifie explicitement que les champs `content` sont absents du payload IPC,
tout en restant disponibles au chemin d’export.
