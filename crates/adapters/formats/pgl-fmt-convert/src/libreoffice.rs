//! LibreOffice Converter (headless)

use super::{ExternalConverter, FormatError};
use pgl_app::{FormatId, Confidence, ExportOptions, ExportReport, TranslationLookup, OutputFormat, SourceDocument};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;

/// Conversor LibreOffice (soffice --headless)
pub struct LibreOfficeConverter {
    soffice_path: String,
    supported_formats: HashMap<FormatId, FormatInfo>,
}

struct FormatInfo {
    extensions: &'static [&'static str],
    input_only: bool,
}

impl LibreOfficeConverter {
    pub fn new() -> Result<Self, FormatError> {
        let soffice_path = which::which("soffice")
            .or_else(|_| which::which("libreoffice"))
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| FormatError::ParseError("LibreOffice (soffice) not found in PATH".into()))?;

        let mut supported = HashMap::new();
        // Entrada
        supported.insert(FormatId("doc"), FormatInfo { extensions: &[".doc"], input_only: true });
        supported.insert(FormatId("xls"), FormatInfo { extensions: &[".xls"], input_only: true });
        supported.insert(FormatId("ppt"), FormatInfo { extensions: &[".ppt"], input_only: true });
        supported.insert(FormatId("rtf"), FormatInfo { extensions: &[".rtf"], input_only: true });
        supported.insert(FormatId("odt"), FormatInfo { extensions: &[".odt"], input_only: false });
        supported.insert(FormatId("ods"), FormatInfo { extensions: &[".ods"], input_only: false });
        supported.insert(FormatId("odp"), FormatInfo { extensions: &[".odp"], input_only: false });
        supported.insert(FormatId("sxw"), FormatInfo { extensions: &[".sxw"], input_only: true });
        supported.insert(FormatId("stw"), FormatInfo { extensions: &[".stw"], input_only: true });
        // Saída
        supported.insert(FormatId::DOCX, FormatInfo { extensions: &[".docx"], input_only: false });
        supported.insert(FormatId::XLSX, FormatInfo { extensions: &[".xlsx"], input_only: false });
        supported.insert(FormatId::PPTX, FormatInfo { extensions: &[".pptx"], input_only: false });
        supported.insert(FormatId::PDF, FormatInfo { extensions: &[".pdf"], input_only: false });
        supported.insert(FormatId::HTML, FormatInfo { extensions: &[".html"], input_only: false });
        supported.insert(FormatId::TXT, FormatInfo { extensions: &[".txt"], input_only: false });

        Ok(Self { soffice_path, supported_formats: supported })
    }

    fn get_filter_name(&self, format: FormatId) -> Option<&'static str> {
        match format.0 {
            "docx" => Some("MS Word 2007 XML"),
            "xlsx" => Some("Calc MS Excel 2007 XML"),
            "pptx" => Some("Impress MS PowerPoint 2007 XML"),
            "pdf" => Some("writer_pdf_Export"),
            "html" => Some("HTML (StarWriter)"),
            "txt" => Some("Text"),
            "rtf" => Some("Rich Text Format"),
            "odt" => Some("OpenDocument Text"),
            "ods" => Some("OpenDocument Spreadsheet"),
            "odp" => Some("OpenDocument Presentation"),
            _ => None,
        }
    }
}

#[async_trait::async_trait]
impl ExternalConverter for LibreOfficeConverter {
    fn format_id(&self) -> FormatId {
        FormatId("libreoffice")
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".doc", ".xls", ".ppt", ".rtf", ".sxw", ".stw"]
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
        // Para DOC/XLS/PPT antigos, converte para OOXML via LibreOffice
        let temp_dir = tempfile::tempdir()?;
        let out_dir = temp_dir.path();

        let filter = self.get_filter_name(FormatId::DOCX).unwrap_or("MS Word 2007 XML");

        let status = Command::new(&self.soffice_path)
            .args([
                "--headless",
                "--convert-to", format!("{}:{}", "docx", filter),
                "--outdir", out_dir.to_str().unwrap(),
                path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| FormatError::ParseError(format!("LibreOffice failed: {}", e)))?;

        if !status.success() {
            return Err(FormatError::ParseError("LibreOffice conversion failed".into()));
        }

        // Encontra arquivo convertido
        let converted = std::fs::read_dir(out_dir)?
            .filter_map(|e| e.ok())
            .find(|e| e.path().extension().map(|s| s == "docx").unwrap_or(false))
            .ok_or_else(|| FormatError::ParseError("Converted file not found".into()))?;

        // Usa leitor DOCX
        crate::docx::open_docx_as_pivot(&converted.path()).await
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        let filter = self.get_filter_name(options.output_format)
            .ok_or_else(|| FormatError::WriteError(format!("Unsupported output format: {:?}", options.output_format)))?;

        let temp_dir = tempfile::tempdir()?;
        let epub_path = temp_dir.path().join("input.epub");

        // Escreve EPUB primeiro
        crate::epub::write_epub_as_pivot(source, translations, options, &epub_path).await?;

        // Converte EPUB para formato alvo via LibreOffice
        // Nota: LibreOffice pode não ler EPUB diretamente, precisa de Writer
        let status = Command::new(&self.soffice_path)
            .args([
                "--headless",
                "--convert-to", format!("{}:{}", out.extension().unwrap().to_str().unwrap(), filter),
                "--outdir", out.parent().unwrap().to_str().unwrap(),
                epub_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| FormatError::WriteError(format!("LibreOffice failed: {}", e)))?;

        if !status.success() {
            return Err(FormatError::WriteError("LibreOffice conversion failed".into()));
        }

        Ok(ExportReport {
            output_path: out.display().to_string(),
            format: options.output_format,
            units_processed: 0,
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec!["Converted via LibreOffice - fidelity may vary".into()],
        })
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::DOCX, OutputFormat::XLSX, OutputFormat::PPTX, OutputFormat::PDF, OutputFormat::HTML, OutputFormat::TXT, OutputFormat::RTF, OutputFormat::ODT, OutputFormat::ODS, OutputFormat::ODP]
    }

    fn is_available(&self) -> bool {
        which::which(&self.soffice_path).is_ok()
    }
}

pub mod epub {
    use super::*;

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

pub mod docx {
    use super::*;

    pub async fn open_docx_as_pivot(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
        let reader = crate::super::super::ooxml::DocxReader::new();
        reader.open(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libreoffice_converter_creation() {
        let _ = LibreOfficeConverter::new();
    }
}