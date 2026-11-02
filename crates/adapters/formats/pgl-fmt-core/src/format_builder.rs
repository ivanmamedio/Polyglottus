//! Format Builder - Elimina boilerplate de FormatReader/FormatWriter
//!
//! Em vez de implementar manualmente 150+ linhas por formato,
//! use `FormatReaderBuilder::new().build()` ou `FormatWriterBuilder::new().build()`

use std::path::Path;
use std::collections::HashMap;
use async_trait::async_trait;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, DocumentMetadata, OutputFormat};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use crate::{ZipSourceDocument, default_zip_fidelity, make_unit_id, make_source_ref, make_section_id, sniff_zip_format, create_empty_zip};
use zip::ZipArchive;
use std::fs::File;
use std::io::{Read, Seek, Write, Cursor};
use uuid::Uuid;
use serde_json;
use md5::compute;
use mime_guess::from_path;

/// Configuração para FormatReader baseado em ZIP
pub struct ZipFormatReaderConfig {
    pub format_id: FormatId,
    pub extensions: &'static [&'static str],
    pub mime_types: &'static [&'static str],
    pub sniff_files: &'static [&'static str],
    pub content_files: Vec<String>, // Arquivos de conteúdo a ler (ex: "word/document.xml")
    pub content_parser: fn(&str) -> Vec<(String, String)>, // (selector, text)
    pub asset_paths: Vec<String>, // Prefixos de assets (ex: "word/media/")
    pub fidelity: fn() -> pgl_app::FidelityReport,
    pub metadata_extractor: Option<fn(&ZipArchive<File>) -> DocumentMetadata>,
}

/// Builder para FormatReader baseado em ZIP
pub struct ZipFormatReaderBuilder {
    config: ZipFormatReaderConfig,
}

impl ZipFormatReaderBuilder {
    pub fn new(format_id: FormatId) -> Self {
        Self {
            config: ZipFormatReaderConfig {
                format_id,
                extensions: &[],
                mime_types: &[],
                sniff_files: &[],
                content_files: vec![],
                content_parser: |_| vec![],
                asset_paths: vec![],
                fidelity: || default_zip_fidelity(),
                metadata_extractor: None,
            },
        }
    }

    pub fn extensions(mut self, ext: &'static [&'static str]) -> Self {
        self.config.extensions = ext;
        self
    }

    pub fn mime_types(mut self, mime: &'static [&'static str]) -> Self {
        self.config.mime_types = mime;
        self
    }

    pub fn sniff_files(mut self, files: &'static [&'static str]) -> Self {
        self.config.sniff_files = files;
        self
    }

    pub fn content_files(mut self, files: Vec<String>) -> Self {
        self.config.content_files = files;
        self
    }

    pub fn content_parser(mut self, parser: fn(&str) -> Vec<(String, String)>) -> Self {
        self.config.content_parser = parser;
        self
    }

    pub fn asset_paths(mut self, paths: Vec<String>) -> Self {
        self.config.asset_paths = paths;
        self
    }

    pub fn fidelity(mut self, fidelity: fn() -> pgl_app::FidelityReport) -> Self {
        self.config.fidelity = fidelity;
        self
    }

    pub fn metadata_extractor(mut self, extractor: fn(&ZipArchive<File>) -> DocumentMetadata) -> Self {
        self.config.metadata_extractor = Some(extractor);
        self
    }

    pub fn build(self) -> Box<dyn FormatReader + Send + Sync> {
        Box::new(BuiltinZipFormatReader { config: self.config })
    }
}

/// Implementação genérica de FormatReader para formatos ZIP
pub struct BuiltinZipFormatReader {
    config: ZipFormatReaderConfig,
}

#[async_trait]
impl FormatReader for BuiltinZipFormatReader {
    fn id(&self) -> FormatId {
        self.config.format_id
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.config.extensions
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), self.config.sniff_files)
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| FormatError::ParseError(e.to_string()))?;

        // Extrai metadados
        let metadata = if let Some(extractor) = self.config.metadata_extractor {
            extractor(&archive)
        } else {
            DocumentMetadata::default()
        };

        // Extrai unidades de conteúdo
        let mut units = Vec::new();
        let section = make_section_id();
        let mut global_idx = 0;

        for content_file in &self.config.content_files {
            if let Ok(content) = read_zip_entry(&mut archive, content_file) {
                let parsed = (self.config.content_parser)(&content);
                
                for (selector, text) in parsed {
                    if text.trim().is_empty() || text.len() < 10 {
                        continue;
                    }
                    let source_ref = make_source_ref(content_file, &selector);
                    let unit_id = make_unit_id(&source_ref, &text);
                    let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(text), make_section_id(), Uuid::nil());
                    unit.id = unit_id;
                    unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
                    units.push(unit);
                    global_idx += 1;
                }
            }
        }

        // Extrai assets
        let mut assets: Vec<AssetRef> = Vec::new();
        for asset_path in &self.config.asset_paths {
            for i in 0..archive.len() {
                // Get entry info first (in a small scope to avoid borrow issues)
                let (name, is_dir) = {
                    let entry = match archive.by_index(i) {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    (entry.name().to_string(), entry.is_dir())
                };
                
                if name.starts_with(asset_path) && !is_dir {
                    // Re-read the entry to get content (need fresh handle)
                    let mut entry2 = archive.by_index(i)?;
                    let mut content = Vec::new();
                    std::io::Read::read_to_end(&mut entry2, &mut content)?;
                    
                    let mime = from_path(&name).first_or_octet_stream();
                    let hash = format!("{:x}", compute(&content));
                    
                    assets.push(AssetRef {
                        id: hash.clone(),
                        internal_path: name.clone(),
                        mime_type: mime.to_string(),
                        size: content.len() as u64,
                        hash,
                    });
                }
            }
        }

        let fidelity = (self.config.fidelity)();

        let empty_archive = ZipArchive::new(Cursor::new(create_empty_zip())).unwrap();

        Ok(Box::new(ZipSourceDocument::new(
            empty_archive, units, assets, fidelity, metadata
        )))
    }
}

/// Configuração para FormatWriter baseado em ZIP
pub struct ZipFormatWriterConfig {
    pub format_id: FormatId,
    pub output_formats: &'static [OutputFormat],
    pub template_files: HashMap<String, String>, // arquivo -> template/content
    pub supports_bilingual: bool,
    pub content_updater: fn(&mut ZipArchive<Cursor<Vec<u8>>>, &dyn TranslationLookup, &ExportOptions) -> Result<(), FormatError>,
}

/// Builder para FormatWriter baseado em ZIP
pub struct ZipFormatWriterBuilder {
    config: ZipFormatWriterConfig,
}

impl ZipFormatWriterBuilder {
    pub fn new(format_id: FormatId) -> Self {
        Self {
            config: ZipFormatWriterConfig {
                format_id,
                output_formats: &[],
                template_files: HashMap::new(),
                supports_bilingual: false,
                content_updater: |_, _, _| Ok(()),
            },
        }
    }

    pub fn output_formats(mut self, formats: &'static [OutputFormat]) -> Self {
        self.config.output_formats = formats;
        self
    }

    pub fn template_file(mut self, name: String, content: String) -> Self {
        self.config.template_files.insert(name, content);
        self
    }

    pub fn supports_bilingual(mut self, val: bool) -> Self {
        self.config.supports_bilingual = val;
        self
    }

    pub fn content_updater(mut self, updater: fn(&mut ZipArchive<Cursor<Vec<u8>>>, &dyn TranslationLookup, &ExportOptions) -> Result<(), FormatError>) -> Self {
        self.config.content_updater = updater;
        self
    }

    pub fn build(self) -> Box<dyn FormatWriter + Send + Sync> {
        Box::new(BuiltinZipFormatWriter { config: self.config })
    }
}

/// Implementação genérica de FormatWriter para formatos ZIP
pub struct BuiltinZipFormatWriter {
    config: ZipFormatWriterConfig,
}

#[async_trait]
impl FormatWriter for BuiltinZipFormatWriter {
    fn id(&self) -> FormatId {
        self.config.format_id
    }

    fn supports_bilingual(&self) -> bool {
        self.config.supports_bilingual
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        self.config.output_formats
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        // Lê arquivo origem
        let source_file = File::open(source)?;
        let mut archive = ZipArchive::new(source_file)
            .map_err(|e| FormatError::WriteError(e.to_string()))?;

        // Cria novo ZIP em memória
        let mut output_buf = Vec::new();
        {
            let mut output_zip = zip::ZipWriter::new(Cursor::new(&mut output_buf));
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(6));

            // Copia todos os arquivos do original
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)
                    .map_err(|e| FormatError::WriteError(e.to_string()))?;
                
                let name = entry.name().to_string();
                let mut content = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut content)?;

                // Se tem template, usa template; senão usa conteúdo original
                let final_content = self.config.template_files.get(&name)
                    .cloned()
                    .unwrap_or_else(|| String::from_utf8_lossy(&content).to_string())
                    .into_bytes();

                output_zip.start_file(&name, options)?;
                output_zip.write_all(&final_content)?;
            }

            output_zip.finish()?;
        }

        // Aplica atualizações de conteúdo (traduções)
        let mut output_archive = ZipArchive::new(Cursor::new(output_buf))
            .map_err(|e| FormatError::WriteError(e.to_string()))?;
        
        (self.config.content_updater)(&mut output_archive, translations, options)?;

        // Salva
        let mut final_buf = Vec::new();
        {
            let mut final_zip = zip::ZipWriter::new(Cursor::new(&mut final_buf));
            for i in 0..output_archive.len() {
                let mut entry = output_archive.by_index(i)?;
                let name = entry.name().to_string();
                let mut content = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut content)?;
                
                let opts = zip::write::FileOptions::default()
                    .compression_method(entry.compression())
                    .compression_level(Some(6));
                final_zip.start_file(&name, opts)?;
                final_zip.write_all(&content)?;
            }
            final_zip.finish()?;
        }

        std::fs::write(out, final_buf)?;

        Ok(ExportReport {
            output_path: out.display().to_string(),
            format: self.config.format_id.to_output_format(),
            units_processed: 0,
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec![],
        })
    }
}

/// Helper para ler entrada do ZIP
fn read_zip_entry(archive: &mut ZipArchive<File>, name: &str) -> Result<String, FormatError> {
    let mut entry = archive.by_name(name)
        .map_err(|e| FormatError::ParseError(format!("Entry {} not found: {}", name, e)))?;
    let mut content = String::new();
    std::io::Read::read_to_string(&mut entry, &mut content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn builder_creation() {
        let reader = ZipFormatReaderBuilder::new(pgl_app::FormatId::EPUB)
            .extensions(&[".epub"])
            .mime_types(&["application/epub+zip"])
            .sniff_files(&[".epub"])
            .content_files(vec!["OEBPS/content.opf".to_string()])
            .asset_paths(vec!["OEBPS/".to_string()])
            .build();
        
        assert_eq!(reader.id(), pgl_app::FormatId::EPUB);
        assert_eq!(reader.extensions(), &[".epub"]);
    }
}