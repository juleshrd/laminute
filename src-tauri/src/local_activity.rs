//! Coordination des activités locales pendant une purge (JUL-176).
//!
//! Précurseur léger de JUL-182 : invalide les traitements en cours et refuse
//! les nouveaux démarrages pendant `delete_all_local_data`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::error::{AppError, AppResult};

/// Jeton d'une opération longue (transcription, résumé, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityToken(u64);

/// Garde d'activité partagée entre enregistrement, IA et purge.
pub struct LocalActivityGate {
    generation: AtomicU64,
    purge_active: AtomicBool,
}

impl LocalActivityGate {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            purge_active: AtomicBool::new(false),
        }
    }

    /// Refuse si une purge est en cours.
    pub fn ensure_not_purging(&self) -> AppResult<()> {
        if self.purge_active.load(Ordering::SeqCst) {
            return Err(AppError::Message(
                "suppression des données locales en cours — opération refusée".into(),
            ));
        }
        Ok(())
    }

    /// Démarre une opération longue ; le jeton doit être revalidé avant chaque écriture.
    pub fn begin_operation(&self) -> AppResult<ActivityToken> {
        self.ensure_not_purging()?;
        Ok(ActivityToken(self.generation.load(Ordering::SeqCst)))
    }

    /// Échoue si une purge a commencé ou invalidé le jeton depuis `begin_operation`.
    pub fn ensure_generation(&self, token: ActivityToken) -> AppResult<()> {
        self.ensure_not_purging()?;
        if self.generation.load(Ordering::SeqCst) != token.0 {
            return Err(AppError::Message(
                "opération annulée : les données locales ont été effacées".into(),
            ));
        }
        Ok(())
    }

    /// Marque la purge active et invalide tous les jetons en cours.
    pub fn begin_purge(&self) -> PurgeGuard<'_> {
        self.purge_active.store(true, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
        PurgeGuard { gate: self }
    }

    #[cfg(test)]
    pub fn is_purging(&self) -> bool {
        self.purge_active.load(Ordering::SeqCst)
    }
}

impl Default for LocalActivityGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Libère le verrou de purge à la fin (succès ou erreur).
pub struct PurgeGuard<'a> {
    gate: &'a LocalActivityGate,
}

impl Drop for PurgeGuard<'_> {
    fn drop(&mut self) {
        self.gate.purge_active.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn purge_invalidates_in_flight_token() {
        let gate = LocalActivityGate::new();
        let token = gate.begin_operation().unwrap();
        {
            let _guard = gate.begin_purge();
            assert!(gate.ensure_generation(token).is_err());
            assert!(gate.begin_operation().is_err());
        }
        assert!(!gate.is_purging());
        // Après purge, un nouveau jeton est accepté.
        let next = gate.begin_operation().unwrap();
        gate.ensure_generation(next).unwrap();
    }

    #[test]
    fn concurrent_purge_blocks_new_operations() {
        let gate = Arc::new(LocalActivityGate::new());
        let gate_purge = Arc::clone(&gate);
        let handle = thread::spawn(move || {
            let _guard = gate_purge.begin_purge();
            thread::sleep(std::time::Duration::from_millis(50));
        });

        // Attendre que la purge démarre.
        while !gate.is_purging() {
            thread::yield_now();
        }
        assert!(gate.begin_operation().is_err());
        handle.join().unwrap();
        assert!(gate.begin_operation().is_ok());
    }
}
