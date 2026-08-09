# Lessons

- Lorsqu’un problème produit renvoie au suivi projet, lire les critères et commentaires Linear avant de figer le périmètre ; un ticket marqué terminé peut masquer une condition de livraison restée incomplète.
- Ne jamais présenter la signature/notarisation macOS comme garantie si la CI possède un chemin de repli non signé : la publication doit échouer sans secrets Apple, et les notes de release doivent refléter le comportement réel du workflow.
- Sur macOS, ne jamais énumérer ou valider les périphériques audio pendant l’initialisation native ou le montage React. Regrouper l’accès au micro derrière une action utilisateur et une seule opération native afin d’éviter les demandes TCC concurrentes.
- Distinguer une garantie de sécurité souhaitée d’une contrainte produit acceptée : si l’utilisateur choisit explicitement de publier sans certificat payant, conserver le fallback non signé, mais l’identifier sans ambiguïté dans la CI, la release et la documentation au lieu de le présenter comme signé ou de bloquer toute publication.
