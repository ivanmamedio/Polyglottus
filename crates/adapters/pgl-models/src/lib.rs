//! Catálogo de modelos e detecção de hardware
//! Stub - implementação completa pendente

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use pgl_app::{ModelCatalog, ModelsError, HardwareProfile, GpuInfo, ModelRecommendation, ModelInfo, QualityTier, LangPair, Language};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

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

/// Catálogo de modelos (stub)
pub struct ModelCatalogImpl {
    config: CatalogConfig,
    hardware_profile: Option<HardwareProfile>,
}

impl ModelCatalogImpl {
    pub fn new(config: CatalogConfig) -> Result<Self, ModelsError> {
        Ok(Self {
            config,
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
        if let Some(profile) = &self.hardware_profile {
            Ok(profile.clone())
        } else {
            self.detect_hardware()
        }
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

    async fn download_model(&self, _model_id: &str, _progress: Arc<dyn Fn(u64, u64) + Send + Sync>) -> Result<ModelInfo, ModelsError> {
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
