//! Adaptador OCR

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod ocrs_engine;
#[cfg(feature = "tesseract")]
pub mod tesseract_engine;

use pgl_app::{OcrEngine, OcrError, OcrResult, BoundingBox, Language};
use pgl_domain::UnitId;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, debug, warn};

/// Engine OCR padrão (ocrs - Rust puro, alfabeto latino)
pub struct OcrsEngine {
    model: ocrs::OcrEngine,
}

impl OcrsEngine {
    pub fn new() -> Result<Self, OcrError> {
        let model = ocrs::OcrEngine::new()
            .map_err(|e| OcrError::Engine(format!("Failed to load ocrs model: {}", e)))?;
        Ok(Self { model })
    }
}

#[async_trait]
impl OcrEngine for OcrsEngine {
    fn id(&self) -> &'static str {
        "ocrs"
    }

    fn supported_languages(&self) -> &'static [Language] {
        &[Language::new("en"), Language::new("pt"), Language::new("es"), Language::new("fr"), Language::new("de"), Language::new("it")]
    }

    async fn recognize(&self, image: &[u8], mime: &str, lang_hint: Option<&Language>) -> Result<OcrResult, OcrError> {
        // Carrega imagem
        let img = image::load_from_memory(image)
            .map_err(|e| OcrError::UnsupportedFormat(e.to_string()))?;

        // Converte para RGB
        let rgb = img.to_rgb8();

        // Executa OCR
        let result = self.model.detect_text(&rgb)
            .map_err(|e| OcrError::RecognitionFailed(e.to_string()))?;

        // Converte resultado
        let text = result.text();
        let confidence = result.confidence();

        let mut boxes = Vec::new();
        for word in result.words() {
            boxes.push(BoundingBox {
                x: word.bbox.x as f32,
                y: word.bbox.y as f32,
                width: word.bbox.width as f32,
                height: word.bbox.height as f32,
                text: word.text.clone(),
                confidence: word.confidence,
            });
        }

        Ok(OcrResult {
            text,
            confidence,
            language: lang_hint.cloned(),
            bounding_boxes: boxes,
        })
    }
}

/// Engine Tesseract (opcional, mais idiomas)
#[cfg(feature = "tesseract")]
pub struct TesseractEngine {
    api: tesseract::Tesseract,
}

#[cfg(feature = "tesseract")]
impl TesseractEngine {
    pub fn new(lang: &str) -> Result<Self, OcrError> {
        let mut api = tesseract::Tesseract::new(None, Some(lang))
            .map_err(|e| OcrError::Engine(format!("Failed to init Tesseract: {}", e)))?;
        api.set_page_seg_mode(tesseract::PageSegMode::Auto)
            .map_err(|e| OcrError::Engine(e.to_string()))?;
        Ok(Self { api })
    }
}

#[cfg(feature = "tesseract")]
#[async_trait]
impl OcrEngine for TesseractEngine {
    fn id(&self) -> &'static str {
        "tesseract"
    }

    fn supported_languages(&self) -> &'static [Language] {
        // Tesseract suporta 100+ idiomas
        &[
            Language::new("en"), Language::new("pt"), Language::new("es"),
            Language::new("fr"), Language::new("de"), Language::new("it"),
            Language::new("ru"), Language::new("zh"), Language::new("ja"),
            Language::new("ko"), Language::new("ar"), Language::new("hi"),
        ]
    }

    async fn recognize(&self, image: &[u8], mime: &str, lang_hint: Option<&Language>) -> Result<OcrResult, OcrError> {
        // Salva imagem temporária
        let temp_path = std::env::temp_dir().join(format!("ocr_{}.png", UnitId::new()));
        std::fs::write(&temp_path, image)
            .map_err(|e| OcrError::Engine(e.to_string()))?;

        // Executa Tesseract
        self.api.set_image(&temp_path)
            .map_err(|e| OcrError::RecognitionFailed(e.to_string()))?;

        let text = self.api.get_text()
            .map_err(|e| OcrError::RecognitionFailed(e.to_string()))?;

        // Limpa temp
        std::fs::remove_file(&temp_path).ok();

        // Tesseract não retorna bounding boxes facilmente via binding
        Ok(OcrResult {
            text,
            confidence: 0.8, // Placeholder
            language: lang_hint.cloned(),
            bounding_boxes: Vec::new(),
        })
    }
}

/// Factory para criar engine OCR apropriado
pub struct OcrEngineFactory;

impl OcrEngineFactory {
    pub fn create_default() -> Result<Arc<dyn OcrEngine>, OcrError> {
        // Tenta ocrs primeiro (Rust puro, leve)
        if let Ok(engine) = OcrsEngine::new() {
            info!("Using ocrs engine (Latin alphabet)");
            return Ok(Arc::new(engine));
        }

        #[cfg(feature = "tesseract")]
        {
            // Fallback para Tesseract
            if let Ok(engine) = TesseractEngine::new("eng") {
                warn!("ocrs failed, falling back to Tesseract");
                return Ok(Arc::new(engine));
            }
        }

        Err(OcrError::Engine("No OCR engine available".into()))
    }

    pub fn create_for_language(lang: &Language) -> Result<Arc<dyn OcrEngine>, OcrError> {
        // ocrs só suporta latino
        let latin_langs = ["en", "pt", "es", "fr", "de", "it", "nl", "pl", "cs", "sk", "hu", "ro", "hr", "sl", "et", "lv", "lt"];
        if latin_langs.contains(&lang.code()) {
            if let Ok(engine) = OcrsEngine::new() {
                return Ok(Arc::new(engine));
            }
        }

        #[cfg(feature = "tesseract")]
        {
            // Tenta Tesseract com idioma específico
            if let Ok(engine) = TesseractEngine::new(lang.code()) {
                return Ok(Arc::new(engine));
            }
        }

        Err(OcrError::LanguageNotSupported(lang.code().to_string()))
    }
}

/// Pipeline OCR para imagens de livros
pub struct BookOcrPipeline {
    engine: Arc<dyn OcrEngine>,
    min_confidence: f32,
    min_text_length: usize,
}

impl BookOcrPipeline {
    pub fn new(engine: Arc<dyn OcrEngine>) -> Self {
        Self {
            engine,
            min_confidence: 0.6,
            min_text_length: 10,
        }
    }

    pub fn with_thresholds(mut self, min_confidence: f32, min_text_length: usize) -> Self {
        self.min_confidence = min_confidence;
        self.min_text_length = min_text_length;
        self
    }

    /// Processa imagem de página de livro
    pub async fn process_page_image(&self, image: &[u8], mime: &str, lang_hint: Option<&Language>) -> Result<OcrPageResult, OcrError> {
        let result = self.engine.recognize(image, mime, lang_hint).await?;

        // Filtra por confiança
        if result.confidence < self.min_confidence || result.text.len() < self.min_text_length {
            return Ok(OcrPageResult {
                text: String::new(),
                confidence: result.confidence,
                language: result.language,
                has_meaningful_text: false,
                bounding_boxes: result.bounding_boxes,
            });
        }

        Ok(OcrPageResult {
            text: result.text,
            confidence: result.confidence,
            language: result.language,
            has_meaningful_text: true,
            bounding_boxes: result.bounding_boxes,
        })
    }

    /// Processa múltiplas imagens (páginas)
    pub async fn process_pages(&self, images: &[(&[u8], &str)], lang_hint: Option<&Language>) -> Result<Vec<OcrPageResult>, OcrError> {
        let mut results = Vec::new();
        for (image, mime) in images {
            results.push(self.process_page_image(image, mime, lang_hint).await?);
        }
        Ok(results)
    }
}

/// Resultado de OCR para página
#[derive(Debug, Clone)]
pub struct OcrPageResult {
    pub text: String,
    pub confidence: f32,
    pub language: Option<Language>,
    pub has_meaningful_text: bool,
    pub bounding_boxes: Vec<BoundingBox>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocrs_engine_creation() {
        // Pode falhar se modelo não estiver disponível
        let _ = OcrsEngine::new();
    }

    #[test]
    fn factory_creation() {
        let _ = OcrEngineFactory::create_default();
    }
}