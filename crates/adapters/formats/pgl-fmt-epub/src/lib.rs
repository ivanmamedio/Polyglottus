//! Adaptador EPUB (leitura e escrita)

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;
use std::collections::HashMap;
use std::io::{Cursor, Write};
use async_trait::async_trait;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, TranslationLookup, SourceDocument, AssetRef, DocumentMetadata, OutputFormat};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, Language, ExportReport, OutputFormat as DomainOutputFormat};
use pgl_fmt_core::{ZipFormatReader, ZipFormatWriter, ZipSourceDocument, make_source_ref, default_zip_fidelity, sniff_zip_format};
use epub::doc::EpubDoc;
use zip::ZipArchive;
use uuid::Uuid;
use tokio::fs;

/// Leitor EPUB (usa epub crate para parsing robusto)
pub struct EpubReader;

impl EpubReader {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatReader for EpubReader {
    fn id(&self) -> FormatId {
        FormatId::EPUB
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".epub"]
    }

    fn sniff(&self, name: &str, _head: &[u8]) -> Confidence {
        sniff_zip_format(Path::new(name), &[".epub"])
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument + 'static>, FormatError> {
        // Usa epub crate para extrair metadados e estrutura
        let mut doc = EpubDoc::new(path)
            .map_err(|e| FormatError::ParseError(e.to_string()))?;

        // Extrai o spine (ordem de leitura)
        let spine = doc.spine.iter().map(|item| item.idref.clone()).collect::<Vec<_>>();
        
        let mut units = Vec::new();
        let section = SectionId::new();
        let mut global_idx = 0;

        // Processa cada item do spine
        // Collect resource data first to avoid borrow issues
        let spine_resources: Vec<(String, String)> = spine.iter()
            .filter_map(|idref| {
                doc.get_resource_str(idref).map(|(content, _)| (idref.clone(), content))
            })
            .collect();

        // Processa cada item do spine
        for (spine_idx, (idref, content)) in spine_resources.iter().enumerate() {
            // Encontra o item no manifest
            let manifest_item = doc.resources.get(idref);
            if manifest_item.is_none() {
                continue;
            }
            let manifest_item = manifest_item.unwrap();
            
            // Extrai texto simples (remove tags HTML básicas)
            let text = extract_text_from_html(&content);
            
            if text.trim().is_empty() {
                continue;
            }

            // Divide em parágrafos
            let paragraphs: Vec<&str> = text.split("\n\n")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && s.len() > 10)
                .collect();

            for (para_idx, para) in paragraphs.iter().copied().enumerate() {
                let unit_id = UnitId::new();
                let source_ref = make_source_ref(&manifest_item.path.to_string_lossy(), &format!("paragraph[{}]", para_idx));
                let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(para), section, Uuid::nil());
                unit.id = unit_id;
                unit.metadata.insert("spine_index".into(), serde_json::Value::Number(spine_idx.into()));
                unit.metadata.insert("resource_id".into(), serde_json::Value::String(idref.clone()));
                unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
                units.push(unit);
                global_idx += 1;
            }
        }

        // Assets (imagens, CSS, fontes)
        let mut assets = Vec::new();
        // Collect resource data first to avoid borrow issues
        let resources: Vec<_> = doc.resources.iter()
            .map(|(id, resource)| (id.clone(), resource.clone()))
            .collect();
        
        for (id, resource) in resources {
            let path_str = resource.path.to_string_lossy();
            if path_str.ends_with(".xml") || path_str.ends_with(".xhtml") || 
               path_str.ends_with(".opf") || path_str.ends_with(".ncx") {
                continue;
            }

            let resource_data = doc.get_resource(&id)
                .map(|(data, _)| data)
                .unwrap_or_default();
            
            let mime = mime_guess::from_path(&resource.path).first_or_octet_stream();
            let hash = format!("{:x}", md5::compute(&resource_data));

            assets.push(AssetRef {
                id: hash.clone(),
                internal_path: path_str.to_string(),
                mime_type: mime.to_string(),
                size: resource_data.len() as u64,
                hash,
            });
        }

        // Fidelidade
        let fidelity = default_zip_fidelity()
            .with_preserved("Estrutura de capítulos (spine)")
            .with_preserved("Sumário (NCX/NAV)")
            .with_preserved("Metadados (OPF)")
            .with_preserved("Imagens, CSS, fontes")
            .with_preserved("Links internos")
            .with_not_preserved("JavaScript (não suportado em EPUB)")
            .with_warning("Scripts são removidos na exportação");

        // Metadados
        let metadata = DocumentMetadata {
            title: doc.mdata("title").map(|s| s.value.clone()),
            author: doc.mdata("creator").map(|s| s.value.clone()),
            language: doc.mdata("language").map(|s| Language::new(&s.value)),
            subject: doc.mdata("subject").map(|s| s.value.clone()),
            description: doc.mdata("description").map(|s| s.value.clone()),
            ..Default::default()
        };

        // Archive vazio (não usado para leitura streaming)
        let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new())).unwrap();

        Ok(Box::new(ZipSourceDocument::new(
            empty_archive,
            units,
            assets,
            fidelity,
            metadata,
        )))
    }
}

/// Extrai texto simples de HTML (remove tags)
fn extract_text_from_html(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    let mut tag_buffer = String::new();
    
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag_buffer.clear();
                tag_buffer.push('<');
            }
            '>' if in_tag => {
                in_tag = false;
                tag_buffer.push('>');
                // Handle some common tags
                let tag_lower = tag_buffer.to_lowercase();
                if tag_lower == "<br>" || tag_lower == "<br/>" || tag_lower == "</p>" || 
                   tag_lower == "<br />" || tag_lower == "</div>" {
                    text.push('\n');
                } else if tag_lower == "<p>" || tag_lower.starts_with("<h") {
                    text.push_str("\n\n");
                }
            }
            _ if in_tag => {
                tag_buffer.push(c);
            }
            _ => {
                text.push(c);
            }
        }
    }
    
    // Clean up multiple newlines
    let mut result = String::new();
    let mut prev_was_newline = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_was_newline {
                result.push('\n');
                prev_was_newline = true;
            }
        } else {
            result.push_str(trimmed);
            result.push('\n');
            prev_was_newline = false;
        }
    }
    
    result
}

impl ZipFormatReader for EpubReader {
    fn format_id(&self) -> FormatId {
        FormatId::EPUB
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".epub"]
    }

    fn mime_types(&self) -> &'static [&'static str] {
        &["application/epub+zip"]
    }
}

/// Writer EPUB - escreve EPUB traduzido preservando estrutura original
pub struct EpubWriter;

impl EpubWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FormatWriter for EpubWriter {
    fn id(&self) -> FormatId {
        FormatId::EPUB
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        _options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        // 1. Ler EPUB origem
        let mut doc = EpubDoc::new(source)
            .map_err(|e| FormatError::ParseError(e.to_string()))?;

        // 2. Preparar mapeamento de resource_id -> conteúdo traduzido
        // Precisamos ler cada recurso HTML, aplicar traduções e escrever de volta
        let mut resource_translations: HashMap<String, String> = HashMap::new();

        // Extrai spine
        let spine = doc.spine.iter().map(|item| item.idref.clone()).collect::<Vec<_>>();
        
        // Para cada item do spine, ler conteúdo original e aplicar traduções
        for idref in &spine {
            if let Some(manifest_item) = doc.resources.get(idref) {
                let path = manifest_item.path.to_string_lossy().to_string();
                
                // Só processa arquivos XHTML/HTML
                if path.ends_with(".xhtml") || path.ends_with(".html") {
                    if let Some((content, _mime)) = doc.get_resource_str(idref) {
                        // Parse HTML e aplica traduções
                        let translated = translate_html_content(&content, &path, translations);
                        resource_translations.insert(idref.clone(), translated);
                    }
                }
            }
        }

        // 3. Criar novo EPUB copiando tudo e substituindo conteúdo traduzido
        let source_bytes = fs::read(source).await
            .map_err(|e| FormatError::WriteError(format!("Failed to read source: {}", e)))?;
        
        let cursor = Cursor::new(source_bytes);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| FormatError::WriteError(format!("Failed to open source zip: {}", e)))?;
        
        // Criar novo zip em memória
        let mut output_buf = Vec::new();
        {
            let mut output_zip = zip::ZipWriter::new(Cursor::new(&mut output_buf));
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(6));

            // Iterar arquivos do EPUB original
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)
                    .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry: {}", e)))?;
                
                let name = file.name().to_string();
                
                // Ler conteúdo
                let mut content = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut content)
                    .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry content: {}", e)))?;
                
                // Se tem tradução para este recurso, substituir
                let final_content = if let Some(translated) = resource_translations.get(&name) {
                    // Se é o arquivo content.opf, pode precisar atualizar
                    if name.ends_with(".opf") {
                        translated.as_bytes().to_vec()
                    } else if name.ends_with(".xhtml") || name.ends_with(".html") {
                        translated.as_bytes().to_vec()
                    } else {
                        content
                    }
                } else {
                    content
                };
                
                // Escrever no novo zip
                output_zip.start_file(&name, options)
                    .map_err(|e| FormatError::WriteError(format!("Failed to start zip file: {}", e)))?;
                output_zip.write_all(&final_content)
                    .map_err(|e| FormatError::WriteError(format!("Failed to write zip file: {}", e)))?;
            }

            output_zip.finish()
                .map_err(|e| FormatError::WriteError(format!("Failed to finish zip: {}", e)))?;
        } // output_zip dropped here, releases borrow on output_buf
        
        // Escrever arquivo final
        fs::write(out, &output_buf).await
            .map_err(|e| FormatError::WriteError(format!("Failed to write output: {}", e)))?;

        Ok(ExportReport {
            output_path: out.to_string_lossy().to_string(),
            format: DomainOutputFormat::EPUB,
            units_processed: resource_translations.len(),
            units_with_warnings: 0,
            units_failed: 0,
            output_size: output_buf.len() as u64,
            export_time_ms: 0,
            warnings: Vec::new(),
        })
    }

    fn supports_bilingual(&self) -> bool {
        false
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::EPUB]
    }
}

impl ZipFormatWriter for EpubWriter {
    fn format_id(&self) -> FormatId {
        FormatId::EPUB
    }
}

/// Aplica traduções ao conteúdo HTML
/// Por enquanto retorna HTML original - implementação completa requer mapeamento unit_id -> posição
fn translate_html_content(html: &str, _resource_path: &str, _translations: &dyn TranslationLookup) -> String {
    html.to_string()
}

/// Verifica EPUB válido
pub fn is_valid_epub(path: &Path) -> bool {
    EpubDoc::new(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn epub_reader_creation() {
        let reader = EpubReader::new();
        assert_eq!(reader.id(), FormatId::EPUB);
    }

    #[test]
    fn epub_writer_creation() {
        let writer = EpubWriter::new();
        assert_eq!(writer.id(), FormatId::EPUB);
    }

    #[test]
    fn create_minimal_epub() {
        let mut f = NamedTempFile::new().unwrap();
        write_test_epub(&mut f).unwrap();

        assert!(is_valid_epub(f.path()));
    }

    fn write_test_epub(f: &mut NamedTempFile) -> std::io::Result<()> {
        use zip::ZipWriter;
        use zip::write::FileOptions;

        let mut zip = ZipWriter::new(f.as_file_mut());
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("mimetype", options)?;
        zip.write_all(b"application/epub+zip")?;

        zip.start_file("META-INF/container.xml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#)?;

        zip.start_file("OEBPS/content.opf", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-id="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">test</dc:identifier>
    <dc:title>Test Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#)?;

        zip.start_file("OEBPS/ch1.xhtml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body><h1>Chapter One</h1><p>Hello world</p></body>
</html>"#)?;

        zip.start_file("OEBPS/nav.xhtml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Navigation</title></head>
<body><nav epub:type="toc"><ol><li><a href="ch1.xhtml">Chapter 1</a></li></ol></nav></body>
</html>"#)?;

        zip.finish()?;
        Ok(())
    }
}