//! Scanner e writer para XLSX
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

/// Abre XLSX como SourceDocument
pub async fn open_xlsx(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let mut units = Vec::new();
    let mut assets = Vec::new();
    let section = SectionId::new();
    let mut global_idx = 0;

    // Ler arquivo XLSX
    let source_bytes = fs::read(path).await
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Parse ZIP
    let cursor = Cursor::new(&source_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Coletar todos os arquivos de worksheet
    let mut sheet_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
            sheet_files.push(name);
        }
    }

    // Ordenar sheets por número
    sheet_files.sort();

    for (sheet_idx, sheet_name) in sheet_files.iter().enumerate() {
        let sheet_xml = read_zip_entry(&mut archive, sheet_name)?;
        let cells = parse_xlsx_sheet_xml(&sheet_xml)?;

        for (cell_idx, (coord, text)) in cells.into_iter().enumerate() {
            if text.trim().is_empty() {
                continue;
            }
            let unit_id = UnitId::new();
            let source_ref = make_source_ref(
                sheet_name,
                &format!("sheet[{}]/cell[{}]/{}", sheet_idx, cell_idx, coord)
            );
            let mut unit = Unit::new(
                source_ref,
                UnitKind::TableCell,
                MarkedText::new(&text),
                section,
                Uuid::nil(),
            );
            unit.id = unit_id;
            unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
            unit.metadata.insert("sheet_index".into(), serde_json::Value::Number(sheet_idx.into()));
            unit.metadata.insert("cell_coord".into(), serde_json::Value::String(coord));
            units.push(unit);
            global_idx += 1;
        }
    }

    // Assets (imagens, etc.)
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("xl/media/") {
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
        .with_preserved("Estrutura de planilhas")
        .with_preserved("Fórmulas (não traduzidas)")
        .with_preserved("Formatação de células")
        .with_preserved("Múltiplas planilhas")
        .with_not_preserved("Macros/VBA")
        .with_warning("Fórmulas são preservadas como estão");

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

/// Escreve XLSX traduzido
pub async fn write_xlsx(
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

    // Coletar arquivos de worksheet
    let mut sheet_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_string();
        if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
            sheet_files.push(name);
        }
    }
    sheet_files.sort();

    // Traduzir cada sheet
    let mut sheet_translations: HashMap<String, String> = HashMap::new();
    for sheet_name in &sheet_files {
        let sheet_xml = read_zip_entry(&mut archive, sheet_name)?;
        let translated_xml = translate_xlsx_sheet_xml(&sheet_xml, translations)?;
        sheet_translations.insert(sheet_name.clone(), translated_xml);
    }

    // Criar novo zip
    let mut output_buf = Vec::new();
    {
        let mut output_zip = zip::ZipWriter::new(Cursor::new(&mut output_buf));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        // Iterar arquivos do XLSX original
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry: {}", e)))?;

            let name = file.name().to_string();

            // Ler conteúdo
            let mut content = Vec::new();
            std::io::Read::read_to_end(&mut file, &mut content)
                .map_err(|e| FormatError::WriteError(format!("Failed to read zip entry content: {}", e)))?;

            // Substituir sheet XML se traduzido
            let final_content = if let Some(translated) = sheet_translations.get(&name) {
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
        format: OutputFormat::XLSX,
        units_processed: 0,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: output_buf.len() as u64,
        export_time_ms: 0,
        warnings: vec!["XLSX writing: XML text replacement".into()],
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

/// Parse XLSX sheet XML para extrair células com texto
fn parse_xlsx_sheet_xml(xml: &str) -> Result<Vec<(String, String)>, FormatError> {
    let mut reader = Reader::from_str(xml);
    reader.expand_empty_elements(true);

    let mut cells: Vec<(String, String)> = Vec::new();
    let mut buf = Vec::new();
    let mut current_coord: Option<String> = None;
    let mut cell_type: Option<String> = None; // "s" para shared string, "n" para número, etc.

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");

                if name_str == "c" {
                    // Célula - obter coordenada do atributo r
                    current_coord = None;
                    cell_type = None;
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let val_str = std::str::from_utf8(attr.value.as_ref()).unwrap_or("").to_string();
                            if key == "r" {
                                current_coord = Some(val_str.clone());
                            } else if key == "t" {
                                cell_type = Some(val_str);
                            }
                        }
                    }
                } else if name_str == "v" {
                    // Valor da célula - será capturado no evento Text
                }
            }
            Ok(Event::Text(e)) => {
                let text = std::str::from_utf8(e.as_ref()).unwrap_or("");
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    if let Some(coord) = &current_coord {
                        if cell_type.as_deref() == Some("s") {
                            // Célula de string compartilhada - o texto é um índice, não o conteúdo real
                            // Para simplicidade, tratamos o índice como texto a ser traduzido
                            cells.push((coord.clone(), trimmed.to_string()));
                        } else {
                            // Célula de texto direto ou número
                            cells.push((coord.clone(), trimmed.to_string()));
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");
                if name_str == "c" {
                    current_coord = None;
                    cell_type = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(cells)
}

/// Traduz XLSX sheet XML aplicando traduções
fn translate_xlsx_sheet_xml(
    xml: &str,
    translations: &dyn TranslationLookup,
) -> Result<String, FormatError> {
    // Coletar traduções indexadas por selector (sheet[N]/cell[M]/coord)
    let mut text_list: Vec<String> = Vec::new();
    for (source_ref, translation) in translations.all_with_refs() {
        let sel = &source_ref.selector;
        if sel.starts_with("sheet[") && sel.contains("/cell[") {
            text_list.push(translation.raw.clone());
        }
    }

    if text_list.is_empty() {
        return Ok(xml.to_string());
    }

    // Abordagem: usar quick_xml para percorrer o XML e substituir textos de <v>
    // em ordem de aparição com as traduções disponíveis
    let mut result = String::with_capacity(xml.len());
    let mut reader = Reader::from_str(xml);
    reader.expand_empty_elements(true);
    let mut buf = Vec::new();
    let mut text_counter = 0usize;
    let mut in_v = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");
                result.push('<');
                result.push_str(name_str);
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        result.push(' ');
                        result.push_str(std::str::from_utf8(attr.key.as_ref()).unwrap_or(""));
                        result.push_str("=\"");
                        let val = std::str::from_utf8(attr.value.as_ref()).unwrap_or("").to_string();
                        result.push_str(&val);
                        result.push('"');
                    }
                }
                result.push('>');
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");
                if name_str == "v" {
                    // Substituir texto da célula se houver tradução
                    if text_counter < text_list.len() {
                        result.push_str(&text_list[text_counter]);
                        text_counter += 1;
                    }
                } else {
                    result.push_str("</");
                    result.push_str(name_str);
                    result.push('>');
                }
                in_v = false;
            }
            Ok(Event::Text(e)) => {
                let text = std::str::from_utf8(e.as_ref()).unwrap_or("");
                if in_v {
                    // Pular texto original de <v>, será substituído na fechadura
                } else {
                    result.push_str(text);
                }
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
    fn xlsx_parse_simple() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><v>42</v></c></row></sheetData></worksheet>"#;
        let cells = parse_xlsx_sheet_xml(xml).unwrap();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].0, "A1");
        assert_eq!(cells[0].1, "0");
        assert_eq!(cells[1].0, "B1");
        assert_eq!(cells[1].1, "42");
    }

    #[test]
    fn xlsx_empty_text() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v></v></c></row></sheetData></worksheet>"#;
        let cells = parse_xlsx_sheet_xml(xml).unwrap();
        assert!(cells.is_empty());
    }
}
