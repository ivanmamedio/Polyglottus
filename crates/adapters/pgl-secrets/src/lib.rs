//! Adaptador de segredos (cofre do sistema)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use pgl_app::{SecretStore, SecretsError};
use async_trait::async_trait;
use keyring::Entry;
use tracing::{debug, warn};

/// Armazenamento de segredos usando keyring do sistema
pub struct SystemSecretStore {
    service: String,
}

impl SystemSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }

    fn entry(&self, key: &str) -> Result<Entry, SecretsError> {
        Entry::new(&self.service, key)
            .map_err(|e| SecretsError::Backend(e.to_string()))
    }
}

#[async_trait]
impl SecretStore for SystemSecretStore {
    async fn get(&self, key: &str) -> Result<Option<String>, SecretsError> {
        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretsError::Backend(e.to_string())),
        }
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), SecretsError> {
        let entry = self.entry(key)?;
        entry.set_password(value)
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        debug!("Secret stored: {}", key);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), SecretsError> {
        let entry = self.entry(key)?;
        match entry.delete_credential() {
            Ok(_) => {
                debug!("Secret deleted: {}", key);
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()), // Já não existe
            Err(e) => Err(SecretsError::Backend(e.to_string())),
        }
    }

    async fn list(&self) -> Result<Vec<String>, SecretsError> {
        // keyring não suporta listagem nativamente
        // Em implementação real, manter índice separado
        warn!("Listing secrets not supported by keyring backend");
        Ok(Vec::new())
    }
}

/// Store em memória (para testes)
#[cfg(test)]
pub struct MemorySecretStore {
    store: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

#[cfg(test)]
impl MemorySecretStore {
    pub fn new() -> Self {
        Self { store: std::sync::Mutex::new(std::collections::HashMap::new()) }
    }
}

#[cfg(test)]
#[async_trait]
impl SecretStore for MemorySecretStore {
    async fn get(&self, key: &str) -> Result<Option<String>, SecretsError> {
        Ok(self.store.lock().unwrap().get(key).cloned())
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), SecretsError> {
        self.store.lock().unwrap().insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), SecretsError> {
        self.store.lock().unwrap().remove(key);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, SecretsError> {
        Ok(self.store.lock().unwrap().keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_basic() {
        let store = MemorySecretStore::new();
        store.set("test_key", "test_value").await.unwrap();
        assert_eq!(store.get("test_key").await.unwrap(), Some("test_value".to_string()));
        store.delete("test_key").await.unwrap();
        assert_eq!(store.get("test_key").await.unwrap(), None);
    }
}