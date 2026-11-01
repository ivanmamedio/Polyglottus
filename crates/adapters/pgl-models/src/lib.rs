//! Catálogo de modelos e detecção de hardware

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod catalog;
pub mod hardware;
pub mod downloader;

use pgl_app::{ModelCatalog, ModelsError, HardwareProfile, GpuInfo, ModelRecommendation, ModelInfo, QualityTier, LangPair, Language};
use pgl_domain::UnitId;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use sysinfo::{System, CpuExt, DiskExt};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{info, debug, warn};
use sha2::{Sha256, Digest};
use futures_util::StreamExt;

/// Catálogo de modelos
pub struct ModelCatalogImpl {
    config: CatalogConfig,
    client: Client,
    models_dir: PathBuf,
    hardware_profile: Option<HardwareProfile>,
    catalog_data: HashMap<String, ModelCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogConfig {
    pub models_dir: PathBuf,
    pub catalog_url: String,
    pub auto_update: bool,
    pub verify_checksums: bool,
}

impl Default for CatalogConfig {
    fn default() -> Self {
        let models_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("polyglottus")
            .join("models");

        Self {
            models_dir,
            catalog_url: "https://raw.githubusercontent.com/polyglottus/models-catalog/main/catalog.json".into(),
            auto_update: true,
            verify_checksums: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelCatalogEntry {
    id: String,
    name: String,
    description: String,
    languages: Vec<String>,
    size_gb: f32,
    quantization: String,
    license: String,
    license_url: String,
    download_url: String,
    sha256: String,
    min_ram_gb: f32,
    recommended_ram_gb: f32,
    requires_gpu: bool,
    min_vram_gb: Option<f32>,
    quality_tier: QualityTier,
    architecture: String, // "llama", "mistral", "gemma", etc.
    family: String,
}

impl ModelCatalogImpl {
    pub fn new(config: CatalogConfig) -> Result<Self, ModelsError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ModelsError::Io(e.into()))?;

        std::fs::create_dir_all(&config.models_dir)?;

        Ok(Self {
            config,
            client,
            models_dir: config.models_dir.clone(),
            hardware_profile: None,
            catalog_data: HashMap::new(),
        })
    }

    /// Inicializa catálogo (carrega local + opcionalmente atualiza)
    pub async fn initialize(&mut self) -> Result<(), ModelsError> {
        self.hardware_profile = Some(self.detect_hardware().await?);
        self.load_local_catalog().await?;

        if self.config.auto_update {
            if let Err(e) = self.update_catalog().await {
                warn!("Failed to update model catalog: {}", e);
            }
        }

        Ok(())
    }

    async fn load_local_catalog(&mut self) -> Result<(), ModelsError> {
        let catalog_path = self.models_dir.join("catalog.json");
        if catalog_path.exists() {
            let content = fs::read_to_string(&catalog_path).await?;
            self.catalog_data = serde_json::from_str(&content)?;
            info!("Loaded local catalog with {} models", self.catalog_data.len());
        }
        Ok(())
    }

    async fn update_catalog(&mut self) -> Result<(), ModelsError> {
        info!("Updating model catalog from {}", self.config.catalog_url);
        let response = self.client.get(&self.config.catalog_url).send().await?;
        if response.status().is_success() {
            let data: HashMap<String, ModelCatalogEntry> = response.json().await?;
            self.catalog_data = data;

            // Salva localmente
            let catalog_path = self.models_dir.join("catalog.json");
            fs::write(&catalog_path, serde_json::to_string_pretty(&self.catalog_data)?).await?;
            info!("Catalog updated: {} models", self.catalog_data.len());
        }
        Ok(())
    }

    fn detect_hardware(&self) -> Result<HardwareProfile, ModelsError> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_cores = sys.cpus().len();
        let total_memory = sys.total_memory();
        let ram_gb = total_memory as f32 / (1024.0 * 1024.0 * 1024.0);

        // Detecta GPU
        let gpu = self.detect_gpu(&mut sys);

        Ok(HardwareProfile {
            cpu_cores,
            ram_gb,
            gpu,
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        })
    }

    fn detect_gpu(&self, _sys: &mut System) -> Option<GpuInfo> {
        // sysinfo tem suporte limitado a GPU
        // Em produção, usar bibliotecas específicas (nvml, metal, vulkan)
        None
    }

    fn recommend_for_profile(&self, profile: &HardwareProfile, lang_pair: &LangPair) -> Result<ModelRecommendation, ModelsError> {
        let available_ram = profile.ram_gb * 0.7; // Reserva 30% para sistema
        let has_gpu = profile.gpu.is_some();
        let vram_gb = profile.gpu.as_ref().map(|g| g.vram_gb).unwrap_or(0.0);

        // Filtra modelos compatíveis
        let mut candidates: Vec<_> = self.catalog_data.values()
            .filter(|m| {
                // Verifica RAM
                if m.min_ram_gb > available_ram {
                    return false;
                }
                // Verifica GPU se necessário
                if m.requires_gpu && !has_gpu {
                    return false;
                }
                if let Some(min_vram) = m.min_vram_gb {
                    if vram_gb < min_vram {
                        return false;
                    }
                }
                // Verifica idiomas
                let target_lang = lang_pair.target.code();
                if !m.languages.iter().any(|l| l == target_lang || l == "multilingual") {
                    return false;
                }
                true
            })
            .collect();

        // Ordena por qualidade (maior primeiro) e depois por tamanho (menor primeiro)
        candidates.sort_by(|a, b| {
            b.quality_tier.cmp(&a.quality_tier)
                .then(a.size_gb.partial_cmp(&b.size_gb).unwrap())
        });

        let best = candidates.first()
            .ok_or_else(|| ModelsError::InsufficientResources {
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