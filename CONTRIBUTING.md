# Contribuer à La Minute

Merci de votre intérêt pour **La Minute** ! Ce guide complète le [README](README.md) (parcours utilisateur) pour le travail sur le dépôt.

## Prérequis

### Tous les systèmes

- [Node.js](https://nodejs.org/) 20+ et npm
- [Rust](https://www.rust-lang.org/tools/install) stable ≥ 1.85 (rustup)

### macOS

- Xcode Command Line Tools : `xcode-select --install`

### Windows

- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (inclus sur Windows 11)

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev
```

## Démarrage local

```bash
npm install
npm run dev
```

`npm run dev` lance Tauri en mode développement (Vite sur le port 1420 + fenêtre native).

| Commande               | Description                                     |
| ---------------------- | ----------------------------------------------- |
| `npm run build`        | Compile le frontend et le backend Rust          |
| `npm run build:bundle` | Produit un installable Tauri (selon l’OS)       |
| `npm run lint`         | ESLint + TypeScript + Clippy                    |
| `npm run format`       | Prettier + rustfmt                              |
| `npm run format:check` | Vérifie le formatage sans modifier les fichiers |
| `npm run test`         | Tests Vitest (frontend) + `cargo test` (Rust)   |
| `npm run check`        | Format + lint + tests + build                   |
| `npm run audit:npm`    | Audit npm (sévérité high)                       |
| `npm run audit:rust`   | `cargo deny check` (licences / advisories)      |
| `npm run check:ci`     | Équivalent local complet de la CI GitHub        |

## Structure du dépôt

```
.
├── src/                  # Frontend React + TypeScript (Vite)
│   ├── lib/              # Utilitaires et métadonnées partagées
│   └── test/             # Configuration des tests Vitest
├── src-tauri/            # Backend Rust et configuration Tauri
│   ├── src/              # Code Rust (commandes, point d'entrée)
│   ├── capabilities/     # Permissions Tauri 2
│   └── tauri.conf.json   # Configuration application / bundle
├── docs/                 # Assets et notes (ex. bannière README)
├── public/               # Assets statiques servis par Vite
├── package.json          # Scripts npm et dépendances frontend
├── LICENSE               # GPL-3.0
└── README.md             # Guide utilisateur
```

Identifiant application : `app.laminute.desktop`.

## Validation locale

Avant d'ouvrir une pull request, exécutez l’équivalent complet de la CI :

```bash
npm run check:ci
```

(`cargo deny` doit être installé : `cargo install cargo-deny`.)

Cette commande enchaîne :

1. **Format** — `prettier --check` (frontend) et `cargo fmt --check` (Rust)
2. **Lint** — ESLint, TypeScript (`tsc --noEmit`), Clippy
3. **Tests** — Vitest (frontend) et `cargo test` (Rust)
4. **Build** — compilation frontend + backend
5. **Audits** — `cargo deny check` et `npm audit --audit-level=high`

Pour une validation produit sans audits : `npm run check`.

Pour corriger le formatage automatiquement : `npm run format`.

Pour produire un bundle installable (non signé) en local : `npm run build:bundle`.

## Tests Rust et trousseau (keyring)

Certains tests Rust qui accèdent au **Secret Service** (stockage des clés API via `keyring`) sont annotés `#[ignore]` et ne s'exécutent pas par défaut. C'est normal sur les environnements sans trousseau déverrouillé (CI, VM headless). Les flux principaux (import MP3, base SQLite, compte-rendu sans clé API) ne dépendent pas du trousseau.

Pour lancer les tests ignorés localement (avec trousseau disponible) :

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
```

## CI

Les pull requests et les pushes sur `main` déclenchent le workflow [CI](.github/workflows/ci.yml), découpé en checks relançables :

| Check GHA       | Commande locale                               |
| --------------- | --------------------------------------------- |
| `format_lint`   | `npm run format:check` puis `npm run lint`    |
| `test_frontend` | `npm run test:web`                            |
| `test_rust`     | `npm run test:rust`                           |
| `build`         | `npm run build`                               |
| `audits`        | `npm run audit:rust` puis `npm run audit:npm` |

`npm run check:ci` exécute l’ensemble. Un job `summary` classe les échecs en **produit** (format, tests, build, audits) ou **infrastructure** (annulation runner / concurrence PR). Sur les PR, les anciens runs sont annulés ; un push sur `main` ne l’est jamais.

Les tags `v*` déclenchent le workflow [Release](.github/workflows/release.yml) qui produit des artefacts installables non signés pour Linux, macOS et Windows.

## Style et portée

- Restez focalisé : une PR par sujet logique.
- Suivez les conventions existantes du dépôt ; évitez les refactorings hors sujet.
- Le code applicatif et la documentation utilisateur sont en **français** lorsque c'est pertinent.
- Ouvrez une issue pour discuter d'une fonctionnalité importante avant d'implémenter, si possible.

## Licence

En contribuant, vous acceptez que vos contributions soient publiées sous la licence [GPL-3.0](LICENSE) du projet.
