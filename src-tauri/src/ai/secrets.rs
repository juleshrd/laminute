use keyring::Entry;
use thiserror::Error;

use crate::APP_IDENTIFIER;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("erreur du trousseau système : {0}")]
    Keyring(#[from] keyring::Error),
}

fn entry_for_provider(provider_id: &str) -> Result<Entry, SecretError> {
    let account = format!("ai-provider-{provider_id}");
    Ok(Entry::new(APP_IDENTIFIER, &account)?)
}

/// Enregistre une clé API dans le trousseau système (jamais en base locale).
pub fn store_api_key(provider_id: &str, api_key: &str) -> Result<(), SecretError> {
    let entry = entry_for_provider(provider_id)?;
    entry.set_password(api_key)?;
    Ok(())
}

/// Récupère une clé API depuis le trousseau système.
pub fn get_api_key(provider_id: &str) -> Result<Option<String>, SecretError> {
    let entry = entry_for_provider(provider_id)?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(SecretError::Keyring(error)),
    }
}

/// Supprime une clé API du trousseau système.
pub fn delete_api_key(provider_id: &str) -> Result<(), SecretError> {
    let entry = entry_for_provider(provider_id)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(SecretError::Keyring(error)),
    }
}

/// Indique si une clé est enregistrée pour un fournisseur.
pub fn has_api_key(provider_id: &str) -> Result<bool, SecretError> {
    Ok(get_api_key(provider_id)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PROVIDER: &str = "test-provider-jul152";

    fn cleanup() {
        let _ = delete_api_key(TEST_PROVIDER);
    }

    #[test]
    #[ignore = "nécessite un trousseau système accessible (Keychain / Secret Service)"]
    fn store_and_retrieve_api_key() {
        cleanup();
        store_api_key(TEST_PROVIDER, "sk-test-secret").expect("stockage");
        let key = get_api_key(TEST_PROVIDER)
            .expect("lecture")
            .expect("clé présente");
        assert_eq!(key, "sk-test-secret");
        cleanup();
    }

    #[test]
    #[ignore = "nécessite un trousseau système accessible (Keychain / Secret Service)"]
    fn delete_removes_api_key() {
        cleanup();
        store_api_key(TEST_PROVIDER, "sk-test-secret").expect("stockage");
        delete_api_key(TEST_PROVIDER).expect("suppression");
        assert!(get_api_key(TEST_PROVIDER).expect("lecture").is_none());
    }
}
