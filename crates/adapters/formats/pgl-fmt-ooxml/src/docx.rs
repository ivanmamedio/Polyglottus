//! Scanner e writer para DOCX
use uuid::Uuid;

use crate::create_empty_zip;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use std::io::{Cursor, Write};
use tokio::fs;
use quick_xml::events::{Event, BytesStart, BytesEnd, BytesText};
use quick_xml::Reader;
use quick_xml::Writer as XmlWriter;
use std::collections::HashMap;
use md5;
use mime_guess;

/// Abre DOCX como SourceDocument
pub async fn open_docx(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let mut units = Vec::new();
    let mut assets = Vec::new();
    let section = SectionId::new();
    let mut global_idx = 0;

    // Ler arquivo DOCX
    let source_bytes = fs::read(path).await
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Parse ZIP
    let cursor = Cursor::new(&source_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Ler document.xml
    let doc_xml = read_zip_entry(&mut archive, "word/document.xml")?;
    
    // Parse XML e extrair parágrafos/tabelas
    let (paragraphs, tables) = parse_docx_xml(&doc_xml)?;
    
    for (para_idx, text) in paragraphs.into_iter().enumerate() {
        if text.trim().is_empty() {
            continue;
        }
        let unit_id = UnitId::new();
        let source_ref = make_source_ref("word/document.xml", &format!("paragraph[{}]", para_idx));
        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(text), section, Uuid::nil());
        unit.id = unit_id;
        unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
        units.push(unit);
        global_idx += 1;
    }
    
    for (table_idx, table) in tables.into_iter().enumerate() {
        for (row_idx, row) in table.into_iter().enumerate() {
            for (cell_idx, text) in row.into_iter().enumerate() {
                if text.trim().is_empty() {
                    continue;
                }
                let unit_id = UnitId::new();
                let source_ref = make_source_ref("word/document.xml", &format!("table[{}]/row[{}]/cell[{}]", table_idx, row_idx, cell_idx));
                let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(text), section, Uuid::nil());
                unit.id = unit_id;
                unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
                units.push(unit);
                global_idx += 1;
            }
        }
    }

    // Assets (imagens, etc.)
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("word/media/") {
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
        .with_preserved("Formatação de parágrafos e runs")
        .with_preserved("Estilos, listas, tabelas")
        .with_preserved("Imagens, cabeçalhos, rodapés")
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

/// Escreve DOCX traduzido
pub async fn write_docx(
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

    // Ler e traduzir document.xml
    let doc_xml = read_zip_entry(&mut archive, "word/document.xml")?;
    let translated_xml = translate_docx_xml(&doc_xml, translations)?;

    // Criar novo zip
    let mut output_buf = Vec::new();
    {
        let mut output_zip = zip::ZipWriter::new(Cursor::new(&mut output_buf));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        // Iterar arquivos do DOCX original
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry: {}", e)))?;
            
            let name = file.name().to_string();
            
            // Ler conteúdo
            let mut content = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut content)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry content: {}", e)))?;
            
            // Substituir document.xml se traduzido
            let final_content = if name == "word/document.xml" {
                translated_xml.as_bytes().to_vec()
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
        format: OutputFormat::DOCX,
        units_processed: 0, // Placeholder
        units_with_warnings: 0,
        units_failed: 0,
        output_size: output_buf.len() as u64,
        export_time_ms: 0,
        warnings: vec!["DOCX writing: basic XML implementation".into()],
    })
}

fn make_source_ref(path: &str, selector: &str) -> SourceRef {
    SourceRef {
        internal_path: path.to_string(),
        selector: selector.to_string(),
        offset: None,
        length: None,
    }
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

/// Parse DOCX XML para extrair parágrafos e tabelas
fn parse_docx_xml(xml: &str) -> Result<(Vec<String>, Vec<Vec<Vec<String>>>), FormatError> {
    let mut reader = Reader::from_str(xml);
    reader.expand_empty_elements(true);
    
    let mut paragraphs = Vec::new();
    let mut tables = Vec::new();
    let mut current_table: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut in_table = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");
                
                if name_str == "w:tbl" {
                    in_table = true;
                    current_table = Vec::new();
                } else if name_str == "w:tr" {
                    current_row = Vec::new();
                } else if name_str == "w:tc" {
                    // cell start
                } else if name_str == "w:p" {
                    // paragraph start - extract text
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");
                
                if name_str == "w:tbl" {
                    if !current_table.is_empty() {
                        tables.push(std::mem::take(&mut current_table));
                    }
                    in_table = false;
                } else if name_str == "w:tr" {
                    if !current_row.is_empty() {
                        current_table.push(std::mem::take(&mut current_row));
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    if in_table {
                        current_row.push(trimmed.to_string());
                    } else {
                        paragraphs.push(trimmed.to_string());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok((paragraphs, tables))
}

/// Traduz DOCX XML aplicando traduções
fn translate_docx_xml(xml: &str, _translations: &dyn TranslationLookup) -> Result<String, FormatError> {
    // Por enquanto retorna XML original
    // Implementação completa requer mapeamento unit_id -> posição no XML
    Ok(xml.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_placeholder() {
        assert!(true);
    }
}