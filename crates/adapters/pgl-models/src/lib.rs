//! Catálogo de modelos e detecção de hardware
//!
//! TODO: Implementação completa do catálogo de modelos.
//! Atualmente é um stub que compila mas não tem funcionalidade completa.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use pgl_app::{ModelCatalog, ModelsError, HardwareProfile, GpuInfo, ModelRecommendation, ModelInfo, QualityTier, LangPair, Language};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

/// Configuração do catálogo de modelos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogConfig {
    pub models_dir: PathBuf,
    pub catalog_url: String,
    pub auto_update: bool,
    pub verify_checksums: bool,
}

impl Default for CatalogConfig {
    fn default() -> Self {
        Self {
            models_dir: PathBuf::from("."),
            catalog_url: String::new(),
            auto_update: false,
            verify_checksums: false,
        }
    }
}

/// Catálogo de modelos (stub - implementação completa pendente)
pub struct ModelCatalogImpl {
    config: CatalogConfig,
    client: Client,
    models_dir: PathBuf,
    hardware_profile: Option<HardwareProfile>,
}

impl ModelCatalogImpl {
    pub fn new(config: CatalogConfig) -> Result<Self, ModelsError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ModelsError::Io(e.into()))?;

        Ok(Self {
            config,
            client,
            models_dir: config.models_dir.clone(),
            hardware_profile: None,
        })
    }

    pub async fn initialize(&mut self) -> Result<(), ModelsError> {
        self.hardware_profile = Some(self.detect_hardware()?);
        Ok(())
    }

    fn detect_hardware(&self) -> Result<HardwareProfile, ModelsError> {
        Ok(HardwareProfile {
            cpu_cores: 1,
            ram_gb: 4.0,
            gpu: None,
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        })
    }
}

#[async_trait]
impl ModelCatalog for ModelCatalogImpl {
    async fn detect_hardware(&self) -> Result<HardwareProfile, ModelsError> {
        Ok(HardwareProfile {
            cpu_cores: 1,
            ram_gb: 4.0,
            gpu: None,
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        })
    }

    async fn recommend_model(&self, _profile: &HardwareProfile, _lang_pair: &LangPair) -> Result<ModelRecommendation, ModelsError> {
        Ok(ModelRecommendation {
            model_id: "stub".into(),
            reason: "Stub implementation".into(),
            estimated_speed: "unknown".into(),
            estimated_ram_gb: 1.0,
            quality_tier: QualityTier::Draft,
            download_size_gb: 0.0,
            license: "unknown".into(),
        })
    }

    async fn download_model(&self, _model_id: &str, _progress: &dyn Fn(u64, u64)) -> Result<ModelInfo, ModelsError> {
        Err(ModelsError::NotFound("Stub: download not implemented".into()))
    }

    async fn list_installed(&self) -> Result<Vec<ModelInfo>, ModelsError> {
        Ok(vec![])
    }

    async fn remove_model(&self, _model_id: &str) -> Result<(), ModelsError> {
        Ok(())
    }

    async fn get_model_path(&self, _model_id: &str) -> Result<Option<PathBuf>, ModelsError> {
        Ok(None)
    }
}
                need: "compatible model".into(),
                have: format!("RAM: {:.1}GB, GPU: {}", available_ram, has_gpu),
            })?;

        let estimated_speed = if has_gpu && vram_gb >= best.size_gb {
            "fast"
        } else if has_gpu {
            "medium"
        } else {
            "slow"
        };

        Ok(ModelRecommendation {
            model_id: best.id.clone(),
            reason: format!("Best match for {}GB RAM, GPU: {}", available_ram, has_gpu),
            estimated_speed: estimated_speed.into(),
            estimated_ram_gb: best.size_gb * 1.2, // Overhead
            quality_tier: best.quality_tier,
            download_size_gb: best.size_gb,
            license: best.license.clone(),
        })
    }
}

#[async_trait]
impl ModelCatalog for ModelCatalogImpl {
    async fn detect_hardware(&self) -> Result<HardwareProfile, ModelsError> {
        if let Some(profile) = &self.hardware_profile {
            Ok(profile.clone())
        } else {
            self.detect_hardware()
        }
    }

    async fn recommend_model(&self, profile: &HardwareProfile, lang_pair: &LangPair) -> Result<ModelRecommendation, ModelsError> {
        self.recommend_for_profile(profile, lang_pair)
    }

    async fn download_model(&self, model_id: &str, progress: &dyn Fn(u64, u64)) -> Result<ModelInfo, ModelsError> {
        let entry = self.catalog_data.get(model_id)
            .ok_or_else(|| ModelsError::NotFound(model_id.into()))?;

        let dest_path = self.models_dir.join(format!("{}.gguf", model_id));

        if dest_path.exists() {
            info!("Model already downloaded: {}", model_id);
            return self.get_model_info(model_id).await;
        }

        info!("Downloading model: {} ({:.1} GB)", model_id, entry.size_gb);

        let response = self.client.get(&entry.download_url).send().await?;
        let total_size = response.content_length().unwrap_or(0);

        let mut file = fs::File::create(&dest_path).await?;
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        let mut stream = std::pin::pin!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ModelsError::DownloadFailed(e.to_string()))?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            progress(downloaded, total_size);
        }

        file.flush().await?;

        // Verifica checksum se habilitado
        if self.config.verify_checksums {
            self.verify_checksum(&dest_path, &entry.sha256).await?;
        }

        info!("Model downloaded: {}", model_id);
        self.get_model_info(model_id).await
    }

    pub async fn verify_checksum(&self, path: &Path, expected: &str) -> Result<(), ModelsError> {
        use sha2::{Sha256, Digest};
        let content = fs::read(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let result = format!("{:x}", hasher.finalize());
        if result != expected {
            fs::remove_file(path).await.ok();
            return Err(ModelsError::DownloadFailed("Checksum mismatch".into()));
        }
        Ok(())
    }

    async fn list_installed(&self) -> Result<Vec<ModelInfo>, ModelsError> {
        let mut models = Vec::new();
        let mut entries = fs::read_dir(&self.models_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().map(|e| e == "gguf").unwrap_or(false) {
                if let Ok(info) = self.get_model_info_from_file(&entry.path()).await {
                    models.push(info);
                }
            }
        }

        Ok(models)
    }

    pub async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo, ModelsError> {
        let path = self.models_dir.join(format!("{}.gguf", model_id));
        self.get_model_info_from_file(&path).await
    }

    pub async fn get_model_info_from_file(&self, path: &Path) -> Result<ModelInfo, ModelsError> {
        let metadata = fs::metadata(path).await?;
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        // Tenta ler metadados do catálogo
        let entry = self.catalog_data.get(&name);

        Ok(ModelInfo {
            id: name.clone(),
            name,
            languages: entry.map(|e| e.languages.iter().map(|s| Language::new(s)).collect()).unwrap_or_default(),
            max_context: 8192,
            is_local: true,
            size_gb: Some((metadata.len() as f64 / (1024.0 * 1024.0 * 1024.0)) as f32),
            license: entry.map(|e| e.license.clone()),
        })
    }

    async fn remove_model(&self, model_id: &str) -> Result<(), ModelsError> {
        let path = self.models_dir.join(format!("{}.gguf", model_id));
        if path.exists() {
            fs::remove_file(path).await?;
            info!("Model removed: {}", model_id);
        }
        Ok(())
    }

    async fn get_model_path(&self, model_id: &str) -> Result<Option<PathBuf>, ModelsError> {
        let path = self.models_dir.join(format!("{}.gguf", model_id));
        Ok(if path.exists() { Some(path) } else { None })
    }
}

// Error type alias
type ModelsErrors = ModelsError;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn catalog_creation() {
        let temp = tempdir().unwrap();
        let config = CatalogConfig {
            models_dir: temp.path().to_path_buf(),
            ..Default::default()
        };
        let catalog = ModelCatalogImpl::new(config);
        assert!(catalog.is_ok());
    }

    #[test]
    fn hardware_detection() {
        let temp = tempdir().unwrap();
        let config = CatalogConfig {
            models_dir: temp.path().to_path_buf(),
            ..Default::default()
        };
        let catalog = ModelCatalogImpl::new(config).unwrap();
        let hw = catalog.detect_hardware().unwrap();
        assert!(hw.cpu_cores > 0);
        assert!(hw.ram_gb > 0.0);
    }
}