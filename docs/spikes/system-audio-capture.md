# Spike — Capture de l’audio système (JUL-147)

Date : 2026-08-05  
Contexte : app desktop **La Minute** (Tauri 2 + Rust + React/TS), local-first.

## 1. Résumé exécutif

Capturer **les deux côtés d’un appel** (micro + audio système / distant) n’est **pas homogène** selon l’OS. Windows offre un loopback WASAPI relativement direct ; macOS exige en pratique un périphérique virtuel (BlackHole…) ou ScreenCaptureKit avec permissions lourdes ; Linux dépend de PulseAudio/PipeWire (monitor sources).

**Recommandation MVP :**

| Capacité | MVP |
|----------|-----|
| Enregistrement micro (dont Jabra vue comme input système) | **IN** |
| Import MP3 | **IN** |
| Capture audio système / « both sides » automatique | **OUT** (post-MVP) |

Le MVP reste fiable avec micro + import. L’audio système est un chantier séparé, documenté ici pour le backlog.

## 2. Matrice par OS

| OS | Faisabilité « both sides » | Dépendances typiques | Permissions / UX | Fiabilité MVP |
|----|----------------------------|----------------------|------------------|---------------|
| **Windows** | Bonne (loopback WASAPI) | WASAPI loopback ; parfois Stereo Mix (legacy) | Accès micro standard ; loopback souvent sans install tierce | Élevée pour un best-effort Windows-only |
| **macOS** | Moyenne / fragile | BlackHole / Loopback / Soundflower ; ou ScreenCaptureKit | Micro + souvent **enregistrement d’écran / audio système** ; TCC strict | Faible sans config utilisateur |
| **Linux** | Variable | PulseAudio / PipeWire monitor (`*.monitor`) | Droits session audio ; pas de modèle unique | Moyenne selon distro/session |

### Détails

**Windows**

- WASAPI loopback capture le mix de rendu (ce qui sort des enceintes), pas le micro.
- Pour « both sides » : mixer loopback + micro, ou s’appuyer sur un périphérique de conf qui agrège déjà.
- Stereo Mix : obsolète / souvent désactivé ; ne pas s’y fier.

**macOS**

- Pas de loopback public équivalent WASAPI pour apps sandboxed/desktop classiques.
- Approche grand public : installer **BlackHole** (ou équivalent), router la sortie système + micro vers un device agrégé, puis enregistrer ce device comme une entrée.
- **ScreenCaptureKit** peut exposer l’audio d’apps/affichage (macOS 13+) mais implique des permissions « screen recording », une UX Apple stricte, et une intégration Rust encore jeune.

**Linux**

- Sources `monitor` PulseAudio/PipeWire = équivalent loopback.
- Qualité et nommage dépendent du serveur audio ; Wayland/PipeWire vs Pulse change l’API pratique.
- Nécessite souvent de choisir manuellement la bonne source monitor.

## 3. Options techniques Tauri / Rust

| Approche | Rôle | Notes |
|----------|------|-------|
| **cpal** | Capture d’entrées (et sorties selon backend) | Bon pour le **micro MVP**. Loopback : OK côté WASAPI si exposé ; limité ailleurs. |
| **cpal + WASAPI loopback** | Audio système Windows | Chemin le plus réaliste pour un post-MVP Windows-first. |
| **screencapturekit-rs** / bindings SCK | Audio/écran macOS | Permissions lourdes ; surface API et stabilité à valider. |
| **Périphérique virtuel (BlackHole, etc.)** | Contournement multi-OS | Hors process app : guide utilisateur + sélection du device comme micro. Compatible avec le flux JUL-146. |
| **Plugins Tauri audio** | Abstraction frontend | Peuvent masquer cpal ; vérifier support loopback réel (souvent non). |

Pour La Minute, la couche audio MVP (JUL-146) doit rester centrée sur **liste d’inputs + enregistrement fichier**, sans brancher SCK ni loopback dans le chemin critique.

## 4. Recommandation MVP

1. **IN MVP** — Micro via cpal (ou équivalent), sélection de device, fichier WAV/MP3 local, import MP3 (JUL-149).
2. **OUT MVP** — Capture audio système native automatique « both sides ».
3. **Post-MVP (ordre suggéré)**  
   - **P0 doc UX** : guide « enregistrement d’appel » (BlackHole / agrégat sur macOS ; monitor PipeWire ; loopback Windows).  
   - **P1** : loopback WASAPI Windows optionnel.  
   - **P2** : exploration ScreenCaptureKit macOS si la demande produit le justifie.  
   - **P3** : détection assistée des sources monitor Linux.

Tant que l’audio système n’est pas productisé, le produit doit **expliquer clairement** que seul le micro (et l’import) sont garantis.

## 5. Limites et risques

- Promettre « enregistre les appels Zoom/Meet des deux côtés » sur les trois OS = **fausse promesse**.
- Installer un driver virtuel (BlackHole) est un frein onboarding et un support non trivial.
- Permissions macOS (écran / audio système) peuvent faire échouer silencieusement la capture.
- Mixer micro + loopback : sync, niveaux, écho ; complexité DSP hors scope MVP.
- Licences / notarization : drivers tiers hors du binaire Tauri.

## 6. Suites backlog

| Ticket / thème | Description |
|----------------|-------------|
| JUL-146 | Micro — **MVP** (ne pas dériver vers system audio) |
| Guide « call recording » | Doc in-app : config OS pour both sides via device virtuel |
| Feature Windows loopback | Option avancée post-MVP |
| Spike SCK macOS | PoC permissions + stabilité si priorisé |
| Linux monitor picker | UI de sélection source PipeWire/Pulse |

## Décision

**Le backlog distingue clairement :**

- **Inclus MVP** : microphone (+ Jabra comme input), import MP3, transcription/résumé.
- **Différé** : capture audio système native cross-platform.

Spike clos côté décision technique ; aucune feature system-audio à livrer dans le MVP.
