# La Minute

Application desktop **La Minute** — monorepo Tauri 2 (Rust) + React + TypeScript + Vite.

Identifiant : `app.laminute.desktop`

## Prérequis

### macOS

- [Node.js](https://nodejs.org/) 20+ et npm
- [Rust](https://www.rust-lang.org/tools/install) (rustup)
- Xcode Command Line Tools : `xcode-select --install`

### Windows

- Node.js 20+ et npm
- Rust (rustup)
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (inclus sur Windows 11)

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Installez aussi Node.js 20+ et Rust via rustup.

## Démarrage local

```bash
npm install
npm run dev
```

`npm run dev` lance Tauri en mode développement (Vite sur le port 1420 + fenêtre native).

Autres commandes utiles :

| Commande | Description |
|----------|-------------|
| `npm run build` | Compile le frontend et le backend Rust |
| `npm run lint` | ESLint + TypeScript + Clippy |
| `npm run format` | Prettier + rustfmt |
| `npm run test` | Tests Vitest (frontend) + `cargo test` (Rust) |
| `npm run check` | **Lint + tests + build** (commande unique pour CI / validation locale) |

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
├── public/               # Assets statiques servis par Vite
├── package.json          # Scripts npm et dépendances frontend
├── vite.config.ts        # Configuration Vite + Vitest
├── eslint.config.js      # ESLint (flat config)
├── rustfmt.toml          # Formatage Rust
├── LICENSE               # GPL-3.0
└── README.md
```

## Validation complète

Avant de pousser ou ouvrir une PR :

```bash
npm run check
```

Cette commande enchaîne le lint frontend/Rust, les tests, puis la compilation complète.

## Licence

GPL-3.0 — voir [LICENSE](LICENSE).
