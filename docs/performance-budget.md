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
