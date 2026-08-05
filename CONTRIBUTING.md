# Contribuer à La Minute

Merci de votre intérêt pour **La Minute** ! Ce guide complète le [README](README.md) pour les contributions au dépôt.

## Avant de commencer

- Consultez le [README](README.md) pour les prérequis (Node.js 20+, Rust, dépendances Tauri selon l'OS) et le démarrage local (`npm install`, `npm run dev`).
- Ouvrez une issue pour discuter d'une fonctionnalité importante avant d'implémenter, si possible.

## Validation locale

Avant d'ouvrir une pull request, exécutez :

```bash
npm run check
```

Cette commande enchaîne :

1. **Format** — `prettier --check` (frontend) et `cargo fmt --check` (Rust)
2. **Lint** — ESLint, TypeScript (`tsc --noEmit`), Clippy
3. **Tests** — Vitest (frontend) et `cargo test` (Rust)
4. **Build** — compilation frontend + backend

Pour corriger le formatage automatiquement : `npm run format`.

Pour produire un bundle installable (non signé) en local : `npm run build:bundle`.

## Tests Rust et trousseau (keyring)

Certains tests Rust qui accèdent au **Secret Service** (stockage des clés API via `keyring`) sont annotés `#[ignore]` et ne s'exécutent pas par défaut. C'est normal sur les environnements sans trousseau déverrouillé (CI, VM headless). Les flux principaux (import MP3, base SQLite, compte-rendu sans clé API) ne dépendent pas du trousseau.

Pour lancer les tests ignorés localement (avec trousseau disponible) :

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
```

## CI

Les pull requests et les pushes sur `main` déclenchent le workflow [CI](.github/workflows/ci.yml) : format, lint, tests, build, vérification des licences Rust (`cargo-deny`) et audit npm (niveau high).

Les tags `v*` déclenchent le workflow [Release](.github/workflows/release.yml) qui produit des artefacts installables non signés pour Linux, macOS et Windows.

## Style et portée

- Restez focalisé : une PR par sujet logique.
- Suivez les conventions existantes du dépôt ; évitez les refactorings hors sujet.
- Le code applicatif et la documentation utilisateur sont en **français** lorsque c'est pertinent.

## Licence

En contribuant, vous acceptez que vos contributions soient publiées sous la licence [GPL-3.0](LICENSE) du projet.
