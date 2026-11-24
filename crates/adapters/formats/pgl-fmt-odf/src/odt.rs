//! Scanner e writer para ODT (OpenDocument Text)

use uuid::Uuid;

use crate::create_empty_zip;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, SourceDocument, AssetRef, FidelityReport, DocumentMetadata, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
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

/// Tags de texto ODT traduzíveis e seus UnitKind correspondentes
const ODT_TEXT_TAGS: &[(&str, UnitKind)] = &[
    ("text:p", UnitKind::Paragraph),
    ("text:h", UnitKind::Heading(1)),
    ("text:list-item", UnitKind::ListItem),
    ("text:span", UnitKind::Paragraph),
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

/// Extrair texto interno de um elemento XML até sua tag de fechamento
fn extract_element_inner_text(
    reader: &mut Reader<&[u8]>,
    tag_name: &str,
    buf: &mut Vec<u8>,
) -> Result<String, FormatError> {
    let mut text = String::new();
    let mut depth = 1u32;

    loop {
        match reader.read_event() {
            Ok(Event::Text(t)) => {
                text.push_str(&String::from_utf8_lossy(t.as_ref()));
                text.push(' ');
            }
            Ok(Event::Start(_)) => {
                depth += 1;
            }
            Ok(Event::End(ref e)) => {
                let end_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if end_tag == tag_name {
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

    Ok(text.trim().to_string())
}

/// Abre ODT como SourceDocument
pub async fn open_odt(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
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

    // Parsear content.xml procurando por elementos de texto
    let mut reader = Reader::from_str(&content_xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut para_idx = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                // Procurar por elementos de texto conhecidos
                if let Some((_, base_kind)) = ODT_TEXT_TAGS.iter().find(|(t, _)| t == &tag_name) {
                    // Extrair o nível do heading se for text:h
                    let mut level = 1u8;
                    if tag_name == "text:h" {
                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                let attr_name = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                if attr_name == "text:outline-level" {
                                    let val = String::from_utf8_lossy(attr.value.as_ref());
                                    level = val.parse().unwrap_or(1);
                                    break;
                                }
                            }
                        }
                    }

                    // Extrair texto do elemento (até o fechamento)
                    let element_text = extract_element_inner_text(&mut reader, &tag_name, &mut buf)?;
                    if !element_text.trim().is_empty() {
                        let unit_id = UnitId::new();
                        let source_ref = SourceRef {
                            internal_path: "content.xml".to_string(),
                            selector: format!("{}", tag_name),
                            offset: None,
                            length: Some(element_text.len()),
                        };
                        let final_kind = if tag_name == "text:h" {
                            UnitKind::Heading(level)
                        } else {
                            *base_kind
                        };
                        let mut unit = Unit::new(source_ref, final_kind, MarkedText::new(&element_text), section, Uuid::nil());
                        unit.id = unit_id;
                        unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
                        units.push(unit);
                        global_idx += 1;
                        para_idx += 1;
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
        .with_preserved("Estrutura de parágrafos e headings")
        .with_preserved("Estilos OpenDocument")
        .with_preserved("Tabelas e células")
        .with_preserved("Imagens e assets")
        .with_not_preserved("Macros")
        .with_warning("ODT implementação em Rust puro");

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

/// Escreve ODT traduzido
pub async fn write_odt(
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
    let translated_xml = translate_odt_xml(&content_xml, translations)?;

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
        format: OutputFormat::ODT,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: output_buf.len() as u64,
        export_time_ms: 0,
        warnings: vec!["ODT writing: basic XML implementation".into()],
    })
}

/// Traduz o XML do content.xml do ODT aplicando traduções aos elementos de texto
fn translate_odt_xml(
    xml: &str,
    translations: &dyn TranslationLookup,
) -> Result<String, FormatError> {
    // Coletar traduções por índice
    let mut para_map: HashMap<usize, String> = HashMap::new();
    for (source_ref, translation) in translations.all_with_refs() {
        if source_ref.internal_path == "content.xml" {
            let tag = source_ref.selector.split(':').next().unwrap_or("");
            if ODT_TEXT_TAGS.iter().any(|(t, _)| *t == tag) {
                if let Some(idx) = source_ref
                    .selector
                    .split('[')
                    .nth(1)
                    .and_then(|s| s.trim_end_matches(']').parse::<usize>().ok())
                {
                    para_map.insert(idx, translation.raw.clone());
                } else {
                    // Se não tem índice, usar o próximo disponível
                    let next_idx = para_map.len();
                    para_map.insert(next_idx, translation.raw.clone());
                }
            }
        }
    }

    if para_map.is_empty() {
        return Ok(xml.to_string());
    }

    // Parsear e reescrever XML com traduções
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut writer = XmlWriter::new(Vec::new());

    let mut para_idx = 0usize;
    let mut in_text_element = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if ODT_TEXT_TAGS.iter().any(|(t, _)| t == &tag_name) {
                    in_text_element = true;
                }

                writer.write_event(Event::Start(e.clone()))?;
            }
            Ok(Event::Text(t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).to_string();
                let mut new_text = text.clone();

                if in_text_element {
                    if let Some(translated) = para_map.get(&para_idx) {
                        new_text = translated.clone();
                    }
                    para_idx += 1;
                }

                writer.write_event(Event::Text(quick_xml::events::BytesText::new(&new_text)))?;
                in_text_element = false;
            }
            Ok(Event::End(ref e)) => {
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
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn odt_placeholder() {
        assert!(true);
    }
}
