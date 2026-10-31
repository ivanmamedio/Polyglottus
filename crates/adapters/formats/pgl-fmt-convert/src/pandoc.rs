//! Pandoc Converter

use super::{ExternalConverter, FormatError};
use pgl_app::{FormatId, Confidence, ExportOptions, ExportReport, TranslationLookup, OutputFormat, SourceDocument, AssetRef, FidelityReport, DocumentMetadata};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;
use zip::ZipArchive;

/// Conversor Pandoc
pub struct PandocConverter {
    pandoc_path: String,
    supported_formats: HashMap<FormatId, FormatInfo>,
}

struct FormatInfo {
    extensions: &'static [&'static str],
    input_only: bool,
}

impl PandocConverter {
    pub fn new() -> Result<Self, FormatError> {
        let pandoc_path = which::which("pandoc")
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| FormatError::ParseError("pandoc not found in PATH".into()))?;

        let mut supported = HashMap::new();
        supported.insert(FormatId::EPUB, FormatInfo { extensions: &[".epub"], input_only: false });
        supported.insert(FormatId::DOCX, FormatInfo { extensions: &[".docx"], input_only: false });
        supported.insert(FormatId::HTML, FormatInfo { extensions: &[".html", ".htm"], input_only: false });
        supported.insert(FormatId::MD, FormatInfo { extensions: &[".md", ".markdown"], input_only: false });
        supported.insert(FormatId::TXT, FormatInfo { extensions: &[".txt"], input_only: false });
        // Formatos de entrada apenas
        supported.insert(FormatId("mobi"), FormatInfo { extensions: &[".mobi"], input_only: true });
        supported.insert(FormatId("azw3"), FormatInfo { extensions: &[".azw3"], input_only: true });
        supported.insert(FormatId("rtf"), FormatInfo { extensions: &[".rtf"], input_only: true });
        supported.insert(FormatId("doc"), FormatInfo { extensions: &[".doc"], input_only: true });
        supported.insert(FormatId("latex"), FormatInfo { extensions: &[".tex", ".latex"], input_only: true });
        supported.insert(FormatId("odt"), FormatInfo { extensions: &[".odt"], input_only: false });
        supported.insert(FormatId("rst"), FormatInfo { extensions: &[".rst"], input_only: true });
        supported.insert(FormatId("textile"), FormatInfo { extensions: &[".textile"], input_only: true });
        supported.insert(FormatId("org"), FormatInfo { extensions: &[".org"], input_only: true });
        supported.insert(FormatId("djvu"), FormatInfo { extensions: &[".djvu"], input_only: true });

        Ok(Self { pandoc_path, supported_formats: supported })
    }

    fn get_output_format(&self, format: FormatId) -> Option<&'static str> {
        match format.0 {
            "epub" => Some("epub"),
            "docx" => Some("docx"),
            "html" => Some("html"),
            "md" => Some("markdown"),
            "txt" => Some("plain"),
            "odt" => Some("odt"),
            _ => None,
        }
    }
}

#[async_trait::async_trait]
impl ExternalConverter for PandocConverter {
    fn format_id(&self) -> FormatId {
        FormatId("pandoc")
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".mobi", ".azw3", ".rtf", ".doc", ".tex", ".latex", ".rst", ".textile", ".org", ".djvu"]
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext = Path::new(name).extension().and_then(|s| s.to_str()).unwrap_or("");
        let ext_with_dot = format!(".{}", ext);

        for (_, info) in &self.supported_formats {
            if info.extensions.iter().any(|e| *e == ext_with_dot) {
                return Confidence::high();
            }
        }
        Confidence::none()
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
        // Converte para EPUB (pivô) e depois usa leitor EPUB
        let temp_dir = tempfile::tempdir()?;
        let epub_path = temp_dir.path().join("converted.epub");

        let status = Command::new(&self.pandoc_path)
            .args([
                "--from", self.detect_input_format(path)?,
                "--to", "epub",
                "--output", epub_path.to_str().unwrap(),
                path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| FormatError::ParseError(format!("pandoc failed: {}", e)))?;

        if !status.success() {
            return Err(FormatError::ParseError("pandoc conversion failed".into()));
        }

        // Usa leitor EPUB no arquivo convertido
        crate::epub::open_epub_as_pivot(&epub_path).await
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        // Para escrita, converte do pivô (EPUB) para formato alvo
        let output_format = self.get_output_format(options.output_format.into())
            .ok_or_else(|| FormatError::WriteError(format!("Unsupported output format: {:?}", options.output_format)))?;

        let temp_dir = tempfile::tempdir()?;
        let epub_path = temp_dir.path().join("input.epub");

        // Primeiro escreve EPUB
        crate::epub::write_epub_as_pivot(source, translations, options, &epub_path).await?;

        // Depois converte para formato final
        let status = Command::new(&self.pandoc_path)
            .args([
                "--from", "epub",
                "--to", output_format,
                "--output", out.to_str().unwrap(),
                epub_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| FormatError::WriteError(format!("pandoc failed: {}", e)))?;

        if !status.success() {
            return Err(FormatError::WriteError("pandoc conversion failed".into()));
        }

        Ok(ExportReport {
            output_path: out.display().to_string(),
            format: options.output_format,
            units_processed: 0, // Seria contado pelo EPUB writer
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec!["Converted via Pandoc - fidelity may vary".into()],
        })
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::EPUB, OutputFormat::DOCX, OutputFormat::HTML, OutputFormat::MD, OutputFormat::TXT, OutputFormat::ODT]
    }

    fn is_available(&self) -> bool {
        which::which(&self.pandoc_path).is_ok()
    }
}

impl PandocConverter {
    fn detect_input_format(&self, path: &Path) -> Result<String, FormatError> {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "mobi" => Ok("mobi"),
            "azw3" => Ok("azw3"),
            "rtf" => Ok("rtf"),
            "doc" => Ok("doc"),
            "tex" | "latex" => Ok("latex"),
            "rst" => Ok("rst"),
            "textile" => Ok("textile"),
            "org" => Ok("org"),
            "djvu" => Ok("djvu"),
            "odt" => Ok("odt"),
            _ => Err(FormatError::ParseError(format!("Unknown format: {}", ext))),
        }
    }
}

/// Helper para usar EPUB como pivô
pub mod epub {
    use super::*;

    pub async fn open_epub_as_pivot(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
        // Reutiliza o leitor EPUB normal
        let reader = crate::super::super::epub::EpubReader::new();
        reader.open(path).await
    }

    pub async fn write_epub_as_pivot(
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        let writer = crate::super::super::epub::EpubWriter::new();
        writer.write(source, translations, options, out).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pandoc_converter_creation() {
        // Só testa se pandoc está disponível
        let _ = PandocConverter::new();
    }
}