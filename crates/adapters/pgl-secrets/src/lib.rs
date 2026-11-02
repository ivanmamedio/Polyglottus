//! Adaptador de segredos (cofre do sistema)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use pgl_app::{SecretStore, SecretsError};
use async_trait::async_trait;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Armazenamento de segredos usando keyring do sistema
pub struct SystemSecretStore {
    service: String,
    index_path: PathBuf,
    index: Mutex<HashSet<String>>,
}

impl SystemSecretStore {
    pub async fn new(service: impl Into<String>) -> Result<Self, SecretsError> {
        let service = service.into();
        let index_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("polyglottus")
            .join("secrets")
            .join(format!("{}.index.json", service.replace('/', "_")));

        // Carrega índice existente ou cria novo
        let index = if index_path.exists() {
            let content = fs::read_to_string(&index_path)
                .await
                .map_err(|e| SecretsError::Backend(e.to_string()))?;
            serde_json::from_str(&content).ok().unwrap_or_default()
        } else {
            HashSet::new()
        };

        Ok(Self {
            service,
            index_path,
            index: Mutex::new(index),
        })
    }

    fn entry(&self, key: &str) -> Result<Entry, SecretsError> {
        Entry::new(&self.service, key)
            .map_err(|e| SecretsError::Backend(e.to_string()))
    }

    /// Adiciona chave ao índice
    async fn add_to_index(&self, key: &str) -> Result<(), SecretsError> {
        let mut index = self.index.lock().await;
        index.insert(key.to_string());
        // Release lock before saving
        std::mem::drop(index);
        self.save_index().await
    }

    /// Remove chave do índice
    async fn remove_from_index(&self, key: &str) -> Result<(), SecretsError> {
        {
            let mut index = self.index.lock().await;
            index.remove(key);
        }
        self.save_index().await
    }

    /// Salva índice em arquivo
    async fn save_index(&self) -> Result<(), SecretsError> {
        let index = self.index.lock().await;
        if let Some(parent) = self.index_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| SecretsError::Backend(e.to_string()))?;
        }
        let json = serde_json::to_string(&*index).map_err(|e| SecretsError::Backend(e.to_string()))?;
        fs::write(&self.index_path, json).await.map_err(|e| SecretsError::Backend(e.to_string()))?;
        Ok(())
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
        self.add_to_index(key).await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), SecretsError> {
        let entry = self.entry(key)?;
        match entry.delete_credential() {
            Ok(_) => {
                debug!("Secret deleted: {}", key);
                self.remove_from_index(key).await?;
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()), // Já não existe
            Err(e) => Err(SecretsError::Backend(e.to_string())),
        }
    }

    async fn list(&self) -> Result<Vec<String>, SecretsError> {
        let index = self.index.lock().await;
        let mut keys: Vec<String> = index.iter().cloned().collect();
        keys.sort();
        Ok(keys)
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