# Direction visuelle — Charcoal Mist (JUL-162)

Références : **Notion** (respiration, peu de chrome) × **Linear** (densité outil) × **Apple glass sobre**.

## Palette

| Token           | Light      | Rôle                        |
| --------------- | ---------- | --------------------------- |
| `--bg-base`     | `#E8ECF1`  | Fond froid gris-bleu        |
| `--ink`         | `#1A1D24`  | Texte charcoal              |
| `--accent`      | `#3D6B8C`  | Soft blue-gray (pas violet) |
| `--bg-elevated` | blanc 72 % | Surfaces glass              |

## Typographie

- Display / marque : **Schibsted Grotesk**
- UI : **DM Sans**

## Glass

`backdrop-filter` uniquement sur le **header sticky** et les **modales**. Listes, CR et champs restent solides.

## Motion

Transitions ≤ 160 ms (onglet, bannière statut, drop-zone). Coupées si `prefers-reduced-motion: reduce`.
