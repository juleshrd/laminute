# Tests E2E natifs (JUL-204)

## Niveaux

| Niveau | Commande | Durée cible | Contenu |
| ------ | -------- | ----------- | ------- |
| PR rapide | `npm run test:e2e` | < 3 min | `e2e_smoke` Rust (fixture MP3, SQLite, search, export full, delete) + gardes config JUL-183/189 |
| Release | `npm run test:e2e:release` | selon OS | smoke PR + build bundle + job smoke installateur (Linux AppImage / macOS / Windows) |

## Prérequis

- Aucune clé API
- Aucun microphone réel
- Fixture : `src-tauri/tests/fixtures/tone-1s.mp3`

## Reproduction locale

```bash
npm run test:e2e
# charge complète release (après build:bundle) :
npm run test:e2e:release
```

## CI

- Job `e2e_smoke` sur chaque PR (Linux)
- Jobs smoke post-bundle dans `release.yml` (matrice OS) — bloquent la publication stable

## Artifacts

Captures / logs uniquement en cas d’échec (pas de contenu de réunion).

## Périmètre WebView

La navigation pendant enregistrement (JUL-178) reste couverte par les tests unitaires
`LmShell` / `useMeetingFlow`. Un driver WebDriverIO + tauri-driver pourra être ajouté
ensuite sans changer les scripts `test:e2e`.
