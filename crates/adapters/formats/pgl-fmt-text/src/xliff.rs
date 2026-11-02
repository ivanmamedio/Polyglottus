//! XLIFF - XML Localization Interchange File Format

use uuid::Uuid;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use quick_xml::Reader;
use quick_xml::events::Event;
type SourceDoc = Box<dyn SourceDocument + 'static>;

#[derive(Debug, Clone)]
struct XliffUnit {
    id: String,
    source: String,
    target: Option<String>,
    note: Option<String>,
    datatype: Option<String>,
}

/// Parse XLIFF 1.2/2.0
fn parse_xliff(content: &str) -> Result<Vec<XliffUnit>, FormatError> {
    let mut units = Vec::new();
    let mut reader = Reader::from_str(content);
    reader.trim_text(true);

    let mut current_unit: Option<XliffUnit> = None;
    let mut in_source = false;
    let mut in_target = false;
    let mut in_note = false;
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"trans-unit" | b"unit" => {
                        let id = e.attributes()
                            .find_map(|a| a.ok())
                            .filter(|a| a.key.as_ref() == b"id" || a.key.as_ref() == b"resname")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string())
                            .unwrap_or_default();
                        let datatype = e.attributes()
                            .find_map(|a| a.ok())
                            .filter(|a| a.key.as_ref() == b"datatype")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string());
                        current_unit = Some(XliffUnit {
                            id,
                            source: String::new(),
                            target: None,
                            note: None,
                            datatype,
                        });
                    }
                    b"source" => in_source = true,
                    b"target" => in_target = true,
                    b"note" => in_note = true,
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default();
                if in_source {
                    if let Some(unit) = &mut current_unit {
                        unit.source.push_str(&text);
                    }
                } else if in_target {
                    if let Some(unit) = &mut current_unit {
                        unit.target.get_or_insert_default().push_str(&text);
                    }
                } else if in_note {
                    if let Some(unit) = &mut current_unit {
                        unit.note.get_or_insert_default().push_str(&text);
                    }
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"source" => in_source = false,
                    b"target" => in_target = false,
                    b"note" => in_note = false,
                    b"trans-unit" | b"unit" => {
                        if let Some(unit) = current_unit.take() {
                            if !unit.source.is_empty() {
                                units.push(unit);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(FormatError::ParseError(e.to_string())),
            _ => {}
        }
        buffer.clear();
    }

    Ok(units)
}

fn create_empty_zip() -> Vec<u8> {
    use zip::ZipWriter;
    use std::io::Cursor;
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        zip.finish().unwrap();
    }
    buf
}


/// Abre XLIFF como SourceDocument
pub async fn open_xliff(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let units_data = parse_xliff(&content)?;

    let mut units = Vec::new();
    let section = SectionId::new();

    for (idx, entry) in units_data.iter().enumerate() {
        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            selector: format!("trans-unit[@id='{}']", entry.id),
            offset: None,
            length: Some(entry.source.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(&entry.source), section, Uuid::nil());
        unit.id = unit_id;
        if let Some(target) = &entry.target {
            unit.metadata.insert("target".into(), serde_json::Value::String(target.clone()));
        }
        if let Some(note) = &entry.note {
            unit.metadata.insert("note".into(), serde_json::Value::String(note.clone()));
        }
        if let Some(dt) = &entry.datatype {
            unit.metadata.insert("datatype".into(), serde_json::Value::String(dt.clone()));
        }

        units.push(unit);
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura XLIFF (unidades, grupos)")
        .with_preserved("Traduções existentes (target)")
        .with_preserved("Notas e contexto")
        .with_preserved("Datatypes (html, plaintext, etc.)")
        .with_not_preserved("Validação de esquema")
        .with_warning("XLIFF 2.0 tem recursos avançados não totalmente suportados");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve XLIFF traduzido
pub async fn write_xliff(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;
    let units_data = parse_xliff(&content)?;
    let units_data_len = units_data.len();

    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<xliff version=\"1.2\" xmlns=\"urn:oasis:names:tc:xliff:document:1.2\">\n");
    output.push_str("  <file original=\"\" source-language=\"en\" target-language=\"pt-BR\" datatype=\"plaintext\">\n");
    output.push_str("    <body>\n");

    for entry in units_data {
        output.push_str(&format!("      <trans-unit id=\"{}\"", escape_xml(&entry.id)));
        if let Some(dt) = &entry.datatype {
            output.push_str(&format!(" datatype=\"{}\"", escape_xml(dt)));
        }
        output.push_str(">\n");
        output.push_str(&format!("        <source>{}</source>\n", escape_xml(&entry.source)));

        let unit_id = UnitId::from_hash({
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            entry.id.hash(&mut hasher);
            entry.source.hash(&mut hasher);
            hasher.finish()
        });

        let target = if let Some(translated) = translations.get(&unit_id) {
            escape_xml(&translated.raw)
        } else {
            entry.target.clone().unwrap_or_default()
        };

        if !target.is_empty() {
            output.push_str(&format!("        <target>{}</target>\n", target));
        }

        if let Some(note) = &entry.note {
            output.push_str(&format!("        <note>{}</note>\n", escape_xml(note)));
        }

        output.push_str("      </trans-unit>\n");
    }

    output.push_str("    </body>\n");
    output.push_str("  </file>\n");
    output.push_str("</xliff>\n");

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::XLIFF,
        units_processed: units_data_len,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&")
        .replace('<', "<")
        .replace('>', ">")
        .replace('"', "\"")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn xliff_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"<?xml version="1.0"?>"#).unwrap();
        writeln!(f, r#"<xliff version="1.2">"#).unwrap();
        writeln!(f, r#"  <file original="test" source-language="en" target-language="pt-BR">"#).unwrap();
        writeln!(f, r#"    <body>"#).unwrap();
        writeln!(f, r#"      <trans-unit id="1"><source>Hello</source><target>Olá</target></trans-unit>"#).unwrap();
        writeln!(f, r#"    </body>"#).unwrap();
        writeln!(f, "  </file>").unwrap();
        writeln!(f, "</xliff>").unwrap();

        let mut units = open_xliff(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 1);
    }
}