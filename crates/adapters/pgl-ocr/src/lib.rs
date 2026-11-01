//! Adaptador OCR

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

// pub mod ocrs_engine;
// #[cfg(feature = "tesseract")]
// pub mod tesseract_engine;

use pgl_app::{OcrEngine, OcrError, OcrResult, BoundingBox, Language};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

/// Engine OCR padrão (ocrs - Rust puro, alfabeto latino)
pub struct OcrsEngine {
    model: ocrs::OcrEngine,
}

impl OcrsEngine {
    pub fn new() -> Result<Self, OcrError> {
        let params = ocrs::OcrEngineParams::default();
        let model = ocrs::OcrEngine::new(params)
            .map_err(|e| OcrError::Engine(format!("Failed to load ocrs model: {}", e)))?;
        Ok(Self { model })
    }
}

use std::sync::LazyLock;

static SUPPORTED_LANGUAGES: LazyLock<Vec<Language>> = LazyLock::new(|| {
    vec![
        Language::new("en"), 
        Language::new("pt"), 
        Language::new("es"), 
        Language::new("fr"), 
        Language::new("de"), 
        Language::new("it")
    ]
});

#[async_trait]
impl pgl_app::OcrEngine for OcrsEngine {
    fn id(&self) -> &'static str {
        "ocrs"
    }

    fn supported_languages(&self) -> &'static [Language] {
        &SUPPORTED_LANGUAGES
    }

    async fn recognize(&self, image: &[u8], _mime: &str, _lang_hint: Option<&Language>) -> Result<pgl_app::OcrResult, OcrError> {
        // Carrega imagem
        let img = image::load_from_memory(image)
            .map_err(|e| OcrError::UnsupportedFormat(e.to_string()))?;

        // Converte para RGB
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();

        // Prepara input para ocrs usando from_bytes
        let input = ocrs::ImageSource::from_bytes(rgb.as_raw(), (width, height))
            .map_err(|e| OcrError::Engine(format!("Failed to prepare input: {}", e)))?;

        // Prepara OcrInput para detecção
        let ocr_input = self.model.prepare_input(input)
            .map_err(|e| OcrError::Engine(format!("Failed to prepare input: {}", e)))?;

        // Executa OCR
        let text = self.model.get_text(&ocr_input)
            .map_err(|e| OcrError::RecognitionFailed(e.to_string()))?;

        // Detecta palavras para bounding boxes
        let word_rects = self.model.detect_words(&ocr_input).unwrap_or_default();

        let mut boxes = Vec::new();
        for rect in word_rects {
            boxes.push(BoundingBox {
                x: rect.center().x as f32 - rect.width() as f32 / 2.0,
                y: rect.center().y as f32 - rect.height() as f32 / 2.0,
                width: rect.width() as f32,
                height: rect.height() as f32,
                text: String::new(), // Individual word text not available in this API
                confidence: 0.8,
            });
        }

        Ok(pgl_app::OcrResult {
            text,
            confidence: 0.8,
            language: None,
            bounding_boxes: boxes,
        })
    }
}

/// Factory para criar engine OCR apropriado
pub struct OcrEngineFactory;

impl OcrEngineFactory {
    pub fn create_default() -> Result<Arc<dyn pgl_app::OcrEngine>, OcrError> {
        // Tenta ocrs primeiro (Rust puro, leve)
        if let Ok(engine) = OcrsEngine::new() {
            info!("Using ocrs engine (Latin alphabet)");
            return Ok(Arc::new(engine));
        }

        // #[cfg(feature = "tesseract")]
        // {
        //     // Fallback para Tesseract
        //     if let Ok(engine) = TesseractEngine::new("eng") {
        //         warn!("ocrs failed, falling back to Tesseract");
        //         return Ok(Arc::new(engine));
        //     }
        // }

        Err(OcrError::Engine("No OCR engine available".into()))
    }

    pub fn create_for_language(lang: &Language) -> Result<Arc<dyn pgl_app::OcrEngine>, OcrError> {
        // ocrs só suporta latino
        let latin_langs = ["en", "pt", "es", "fr", "de", "it", "nl", "pl", "cs", "sk", "hu", "ro", "hr", "sl", "et", "lv", "lt"];
        if latin_langs.contains(&lang.code()) {
            if let Ok(engine) = OcrsEngine::new() {
                return Ok(Arc::new(engine));
            }
        }

        // #[cfg(feature = "tesseract")]
        // {
        //     // Tenta Tesseract com idioma específico
        //     if let Ok(engine) = TesseractEngine::new(lang.code()) {
        //         return Ok(Arc::new(engine));
        //     }
        // }

        Err(OcrError::LanguageNotSupported(lang.code().to_string()))
    }
}

/// Pipeline OCR para imagens de livros
pub struct BookOcrPipeline {
    engine: Arc<dyn pgl_app::OcrEngine>,
    min_confidence: f32,
    min_text_length: usize,
}

impl BookOcrPipeline {
    pub fn new(engine: Arc<dyn pgl_app::OcrEngine>) -> Self {
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