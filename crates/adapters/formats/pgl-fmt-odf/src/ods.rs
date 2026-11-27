//! Scanner e writer para ODS (OpenDocument Spreadsheet)

use uuid::Uuid;

use crate::create_empty_zip;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity, make_source_ref};
use std::path::Path;
use zip::ZipArchive;
use quick_xml::events::{Event, BytesStart};
use quick_xml::Reader;
use quick_xml::Writer as XmlWriter;
use std::io::{Cursor, Read, Seek, Write};
use std::collections::HashMap;
use tokio::fs;
use md5;
use mime_guess;

/// Tags de célula ODS traduzíveis
const ODS_CELL_TAGS: &[(&str, UnitKind)] = &[
    ("table:table-cell", UnitKind::TableCell),
];

fn create_empty_zip_local() -> Vec<u8> {
    use zip::ZipWriter;
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        zip.finish().unwrap();
    }
    buf
}

/// Ler entrada do ZIP (genérico sobre o tipo de leitor)
fn read_zip_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, FormatError>
where
    R: Read + Seek,
{
    let mut entry = archive.by_name(name)
        .map_err(|e| FormatError::ParseError(format!("Entry {} not found: {}", name, e)))?;
    let mut content = String::new();
    entry.read_to_string(&mut content)?;
    Ok(content)
}

/// Abre ODS como SourceDocument
pub async fn open_ods(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let source_bytes = fs::read(path).await
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    let cursor = Cursor::new(&source_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Ler content.xml
    let content_xml = read_zip_entry(&mut archive, "content.xml")?;

    let mut units = Vec::new();
    let section = SectionId::new();
    let mut global_idx = 0usize;
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    // Parsear content.xml procurando por células de tabela
    let mut reader = Reader::from_str(&content_xml);
    reader.trim_text(true);
    let mut buf: Vec<u8> = Vec::new();

    let mut cell_idx = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if tag_name == "table:table-cell" {
                    // Extrair o valor de texto da célula
                    let mut cell_text = String::new();
                    let mut cell_type = String::new();

                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let attr_name = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if attr_name == "office:value-type" {
                                cell_type = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            }
                        }
                    }

                    // Ler o conteúdo da célula (texto dentro de <text:p> ou diretamente em <text>)
                    let mut depth = 1u32;
                    let mut in_cell = true;
                    let mut found_text = false;

                    loop {
                        match reader.read_event() {
                            Ok(Event::Text(t)) => {
                                let text = String::from_utf8_lossy(t.as_ref());
                                let trimmed = text.trim();
                                if !trimmed.is_empty() && cell_type == "string" {
                                    cell_text.push_str(&trimmed);
                                    cell_text.push(' ');
                                    found_text = true;
                                }
                            }
                            Ok(Event::Start(_)) => {
                                depth += 1;
                            }
                            Ok(Event::End(ref e)) => {
                                let end_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                                if end_tag == "table:table-cell" {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                            }
                            Ok(Event::Eof) => break,
                            Ok(_) => {}
                            Err(e) => return Err(FormatError::Xml(e)),
                        }
                        buf.clear();
                    }

                    if found_text && !cell_text.trim().is_empty() {
                        let unit_id = UnitId::new();
                        let source_ref = SourceRef {
                            internal_path: "content.xml".to_string(),
                            selector: format!("table:table-cell[{}]", cell_idx),
                            offset: None,
                            length: Some(cell_text.len()),
                        };
                        let mut unit = Unit::new(source_ref, UnitKind::TableCell, MarkedText::new(&cell_text), section, Uuid::nil());
                        unit.id = unit_id;
                        unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
                        units.push(unit);
                        global_idx += 1;
                        cell_idx += 1;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(FormatError::Xml(e)),
        }
        buf.clear();
    }

    // Assets (imagens, etc.)
    let mut assets = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("Pictures/") || name.starts_with("Thumbnails/") {
            let mut content = Vec::new();
            file.read_to_end(&mut content).unwrap();
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
        .with_preserved("Estrutura de planilhas")
        .with_preserved("Fórmulas (não traduzidas)")
        .with_preserved("Formatação de células")
        .with_preserved("Múltiplas planilhas")
        .with_not_preserved("Macros")
        .with_warning("ODS implementação em Rust puro");

    let metadata = DocumentMetadata {
        title: None,
        subject: None,
        ..Default::default()
    };

    let empty_archive = ZipArchive::new(Cursor::new(create_empty_zip_local())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, metadata
    )))
}

/// Escreve ODS traduzido
pub async fn write_ods(
    source: &Path,
    translations: &dyn TranslationLookup,
    _options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let source_bytes = fs::read(source).await
        .map_err(|e| FormatError::WriteError(format!("Failed to read source: {}", e)))?;

    let cursor = Cursor::new(&source_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FormatError::WriteError(format!("Failed to open source zip: {}", e)))?;

    // Ler e traduzir content.xml
    let content_xml = read_zip_entry(&mut archive, "content.xml")?;
    let translated_xml = translate_ods_xml(&content_xml, translations)?;

    // Criar novo zip
    let mut output_buf = Vec::new();
    {
        let mut output_zip = zip::ZipWriter::new(Cursor::new(&mut output_buf));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry: {}", e)))?;

            let name = file.name().to_string();
            let mut content = Vec::new();
            file.read_to_end(&mut content)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry content: {}", e)))?;

            let final_content = if name == "content.xml" {
                translated_xml.as_bytes().to_vec()
            } else {
                content
            };

            output_zip.start_file(&name, options)
                .map_err(|e| FormatError::WriteError(format!("Failed to start zip file: {}", e)))?;
            output_zip.write_all(&final_content)
                .map_err(|e| FormatError::WriteError(format!("Failed to write zip file: {}", e)))?;
        }

        output_zip.finish()
            .map_err(|e| FormatError::WriteError(format!("Failed to finish zip: {}", e)))?;
    }

    fs::write(out, &output_buf).await
        .map_err(|e| FormatError::WriteError(format!("Failed to write output: {}", e)))?;

    Ok(ExportReport {
        output_path: out.to_string_lossy().to_string(),
        format: OutputFormat::ODS,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: output_buf.len() as u64,
        export_time_ms: 0,
        warnings: vec!["ODS writing: basic XML implementation".into()],
    })
}

/// Traduz o XML do content.xml do ODS aplicando traduções às células de texto
fn translate_ods_xml(
    xml: &str,
    translations: &dyn TranslationLookup,
) -> Result<String, FormatError> {
    // Coletar traduções por índice de célula
    let mut cell_map: HashMap<usize, String> = HashMap::new();
    for (source_ref, translation) in translations.all_with_refs() {
        if source_ref.internal_path == "content.xml" {
            if source_ref.selector.starts_with("table:table-cell[") {
                if let Some(idx) = source_ref
                    .selector
                    .split('[')
                    .nth(1)
                    .and_then(|s| s.trim_end_matches(']').parse::<usize>().ok())
                {
                    cell_map.insert(idx, translation.raw.clone());
                } else {
                    let next_idx = cell_map.len();
                    cell_map.insert(next_idx, translation.raw.clone());
                }
            }
        }
    }

    if cell_map.is_empty() {
        return Ok(xml.to_string());
    }

    // Parsear e reescrever XML com traduções
    // Para ODS, substituímos o texto dentro de elementos <text:p> que estão dentro de <table:table-cell>
    // com office:value-type="string"
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut writer = XmlWriter::new(Vec::<u8>::new());

    let mut cell_idx = 0usize;
    let mut in_table_cell = false;
    let mut cell_value_type = String::new();
    let mut in_text_p = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if tag_name == "table:table-cell" {
                    in_table_cell = true;
                    cell_value_type = String::new();
                    // Verificar se é uma célula de string
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let attr_name = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if attr_name == "office:value-type" {
                                cell_value_type = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                            }
                        }
                    }
                }

                if tag_name == "text:p" && in_table_cell && cell_value_type == "string" {
                    in_text_p = true;
                }

                writer.write_event(Event::Start(e.clone()))?;
            }
            Ok(Event::Text(t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).to_string();
                let mut new_text = text.clone();

                if in_text_p && cell_value_type == "string" {
                    if let Some(translated) = cell_map.get(&cell_idx) {
                        new_text = translated.clone();
                    }
                    cell_idx += 1;
                }

                writer.write_event(Event::Text(quick_xml::events::BytesText::new(&new_text)))?;
                in_text_p = false;
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "table:table-cell" {
                    in_table_cell = false;
                    cell_value_type = String::new();
                }
                writer.write_event(Event::End(e.clone()))?;
            }
            Ok(Event::Eof) => break,
            Ok(e) => writer.write_event(e)?,
            Err(e) => return Err(FormatError::Xml(e)),
        }
    }

    let result = String::from_utf8(writer.into_inner())
        .map_err(|e| FormatError::Utf8(e))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ods_parse_simple() {
        let xml = r#"<office:spreadsheet xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><table:table><table:table-cell office:value-type="string"><text:p>Hello</text:p></table:table-cell><table:table-cell office:value-type="string"><text:p>World</text:p></table:table-cell></table:table></office:spreadsheet>"#;
        // Verificar que o XML pode ser parseado
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut buf: Vec<u8> = Vec::new();
        let mut cell_count = 0;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if tag_name == "table:table-cell" {
                        cell_count += 1;
                    }
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }

        assert_eq!(cell_count, 2);
    }
}
