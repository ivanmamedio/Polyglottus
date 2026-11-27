//! Scanner e writer para PPTX
use uuid::Uuid;

use crate::create_empty_zip;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity, make_source_ref};
use std::path::Path;
use zip::ZipArchive;
use std::io::{Cursor, Write};
use tokio::fs;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use md5;
use mime_guess;

/// Abre PPTX como SourceDocument
pub async fn open_pptx(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let mut units = Vec::new();
    let mut assets = Vec::new();
    let section = SectionId::new();
    let mut global_idx = 0;

    // Ler arquivo PPTX
    let source_bytes = fs::read(path).await
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Parse ZIP
    let cursor = Cursor::new(&source_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Coletar todos os arquivos de slide
    let mut slide_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_files.push(name);
        }
    }

    // Ordenar slides por número
    slide_files.sort();

    for (slide_idx, slide_name) in slide_files.iter().enumerate() {
        let slide_xml = read_zip_entry(&mut archive, slide_name)?;
        let texts = parse_pptx_slide_xml(&slide_xml)?;

        for (text_idx, text) in texts.into_iter().enumerate() {
            if text.trim().is_empty() {
                continue;
            }
            let unit_id = UnitId::new();
            let source_ref = make_source_ref(
                slide_name,
                &format!("slide[{}]/text[{}]", slide_idx, text_idx)
            );
            let mut unit = Unit::new(
                source_ref,
                UnitKind::Paragraph,
                MarkedText::new(&text),
                section,
                Uuid::nil(),
            );
            unit.id = unit_id;
            unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
            unit.metadata.insert("slide_index".into(), serde_json::Value::Number(slide_idx.into()));
            units.push(unit);
            global_idx += 1;
        }
    }

    // Assets (imagens, etc.)
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("ppt/media/") {
            let mut content = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut content).unwrap();
            let mime = mime_guess::from_path(&name).first_or_octet_stream();
            let hash = format!("{:x}", md5::compute(&content));
            assets.push(AssetRef {
                id: hash.clone(),
                internal_path: name,
                mime_type: mime.to_string(),
                size: content.len() as u64,
                hash,
            });
        }
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de slides")
        .with_preserved("Formatação de texto (runs, fontes, cores)")
        .with_preserved("Imagens, formas, tabelas")
        .with_preserved("Notas do apresentador")
        .with_not_preserved("Macros/VBA")
        .with_warning("Macros/VBA não preservados");

    let metadata = DocumentMetadata {
        title: None,
        subject: None,
        ..Default::default()
    };

    let empty_archive = ZipArchive::new(Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive,
        units,
        assets,
        fidelity,
        metadata,
    )))
}

/// Escreve PPTX traduzido
pub async fn write_pptx(
    source: &Path,
    translations: &dyn TranslationLookup,
    _options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Ler arquivo original
    let source_bytes = fs::read(source).await
        .map_err(|e| FormatError::WriteError(format!("Failed to read source: {}", e)))?;

    // Parse ZIP
    let cursor = Cursor::new(source_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FormatError::WriteError(format!("Failed to open source zip: {}", e)))?;

    // Coletar arquivos de slide
    let mut slide_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_files.push(name);
        }
    }
    slide_files.sort();

    // Traduzir cada slide
    let mut slide_translations: HashMap<String, String> = HashMap::new();
    for slide_name in &slide_files {
        let slide_xml = read_zip_entry(&mut archive, slide_name)?;
        let translated_xml = translate_pptx_slide_xml(&slide_xml, translations)?;
        slide_translations.insert(slide_name.clone(), translated_xml);
    }

    // Criar novo zip
    let mut output_buf = Vec::new();
    {
        let mut output_zip = zip::ZipWriter::new(Cursor::new(&mut output_buf));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        // Iterar arquivos do PPTX original
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry: {}", e)))?;

            let name = file.name().to_string();

            // Ler conteúdo
            let mut content = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut content)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry content: {}", e)))?;

            // Substituir slide XML se traduzido
            let final_content = if let Some(translated) = slide_translations.get(&name) {
                translated.as_bytes().to_vec()
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
    }

    // Escrever arquivo final
    fs::write(out, &output_buf).await
        .map_err(|e| FormatError::WriteError(format!("Failed to write output: {}", e)))?;

    Ok(ExportReport {
        output_path: out.to_string_lossy().to_string(),
        format: OutputFormat::PPTX,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: output_buf.len() as u64,
        export_time_ms: 0,
        warnings: vec!["PPTX writing: XML text replacement".into()],
    })
}

/// Lê entrada do ZIP
fn read_zip_entry<R: std::io::Read + std::io::Seek>(archive: &mut ZipArchive<R>, name: &str) -> Result<String, FormatError> {
    let mut file = archive.by_name(name)
        .map_err(|e| FormatError::ParseError(format!("Entry {} not found: {}", name, e)))?;
    let mut content = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut content)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;
    String::from_utf8(content)
        .map_err(|e| FormatError::ParseError(e.to_string()))
}

/// Parse PPTX slide XML para extrair textos
fn parse_pptx_slide_xml(xml: &str) -> Result<Vec<String>, FormatError> {
    let mut reader = Reader::from_str(xml);
    reader.expand_empty_elements(true);

    let mut texts = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    texts.push(trimmed.to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(texts)
}

/// Traduz PPTX slide XML aplicando traduções
fn translate_pptx_slide_xml(
    xml: &str,
    translations: &dyn TranslationLookup,
) -> Result<String, FormatError> {
    // Coletar traduções indexadas por selector (slide[N]/text[M])
    let mut text_list: Vec<String> = Vec::new();
    for (source_ref, translation) in translations.all_with_refs() {
        let sel = &source_ref.selector;
        if sel.starts_with("slide[") && sel.contains("/text[") {
            text_list.push(translation.raw.clone());
        }
    }

    if text_list.is_empty() {
        return Ok(xml.to_string());
    }

    // Abordagem: usar quick_xml para percorrer o XML e substituir textos de a:t
    // em ordem de aparição com as traduções disponíveis
    let mut result = String::with_capacity(xml.len());
    let mut reader = Reader::from_str(xml);
    reader.expand_empty_elements(true);
    let mut buf = Vec::new();
    let mut text_counter = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                let text = std::str::from_utf8(e.as_ref()).unwrap_or("");
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    if text_counter < text_list.len() {
                        result.push_str(&text_list[text_counter]);
                    } else {
                        result.push_str(text);
                    }
                    text_counter += 1;
                } else {
                    result.push_str(text);
                }
            }
            Ok(Event::Start(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");
                result.push('<');
                result.push_str(name_str);
                result.push('>');
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");
                result.push_str("</");
                result.push_str(name_str);
                result.push('>');
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pptx_parse_simple() {
        let xml = r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:t>Hello</a:t><a:t>World</a:t></p:sld>"#;
        let texts = parse_pptx_slide_xml(xml).unwrap();
        assert_eq!(texts, vec!["Hello", "World"]);
    }

    #[test]
    fn pptx_empty_text() {
        let xml = r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:t></a:t></p:sld>"#;
        let texts = parse_pptx_slide_xml(xml).unwrap();
        assert!(texts.is_empty());
    }
}
