# Direction visuelle — Charcoal Mist (JUL-162)

Références : **Notion** (respiration, peu de chrome) × **Linear** (densité outil) × **Apple glass sobre**.

## Principes

- Une seule intention par vue : préparer une réunion, retrouver une réunion ou configurer l’app.
- La hiérarchie vient de l’espace, de la typographie et du contraste, pas d’une accumulation de cartes.
- Les actions principales utilisent l’accent bleu-gris ; les actions secondaires restent neutres.
- Les libellés, focus visibles et zones tactiles restent accessibles au clavier.
- Aucun asset ou service externe n’est requis pour afficher l’interface.

## Palette

| Token           | Light      | Rôle                        |
| --------------- | ---------- | --------------------------- |
| `--bg-base`     | `#E8ECF1`  | Fond froid gris-bleu        |
| `--ink`         | `#1A1D24`  | Texte charcoal              |
| `--accent`      | `#3D6B8C`  | Soft blue-gray (pas violet) |
| `--bg-elevated` | blanc 72 % | Surfaces glass              |

## Typographie

La marque utilise la graisse display de la police système et l’interface sa variante texte. Cette pile
native rappelle la sobriété Notion/Linear, évite un téléchargement au démarrage et reste nette sur
macOS, Windows et Linux.

## Glass

`backdrop-filter` uniquement sur le **header sticky** et les **modales**. Listes, CR et champs restent solides.
Une couleur translucide avec bordure reste lisible si le moteur ne prend pas le flou en charge.

## Motion

Transitions ≤ 160 ms (onglet, bannière statut, drop-zone). Coupées si `prefers-reduced-motion: reduce`.

## Écrans

- **Réunion** : l’import MP3 est l’action dominante ; l’enregistrement et le périphérique restent
  disponibles sans occuper le premier plan.
- **Historique** : recherche et calendrier compacts, puis liste dense et lisible ; les lignes hors écran
  utilisent un rendu différé.
- **Réglages** : fournisseur IA puis confidentialité, avec la zone destructive isolée.

## À éviter

- Violet, halos et dégradés décoratifs associés aux interfaces IA génériques.
- Glass sur chaque panneau, ombres fortes, pills systématiques et animations longues.
- Polices web, images lourdes et dépendances d’animation ou de virtualisation.

## Validation

La revue visuelle couvre les modes clair/sombre, la largeur minimale 800 px et
`prefers-reduced-motion`. Le test fonctionnel de référence reste l’import d’un MP3 jusqu’à la création
d’une réunion, suivi de son ouverture dans l’historique.
