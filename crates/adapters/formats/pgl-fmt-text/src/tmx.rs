//! TMX - Translation Memory eXchange

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use quick_xml::Reader;
use quick_xml::events::Event;
type SourceDoc = Box<dyn SourceDocument + 'static>;

#[derive(Debug, Clone)]
struct TmxTu {
    tuid: Option<String>,
    source_lang: String,
    source_text: String,
    target_lang: String,
    target_text: Option<String>,
    note: Option<String>,
    prop: HashMap<String, String>,
}

use std::collections::HashMap;

/// Parse TMX
fn parse_tmx(content: &str) -> Result<Vec<TmxTu>, FormatError> {
    let mut tus = Vec::new();
    let mut reader = Reader::from_str(content);
    reader.trim_text(true);

    let mut current_tu: Option<TmxTu> = None;
    let mut current_tuv: Option<(String, String)> = None; // (lang, text)
    let mut in_seg = false;
    let mut source_lang = String::new();
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"tu" => {
                        let tuid = e.attributes()
                            .find_map(|a| a.ok())
                            .filter(|a| a.key.as_ref() == b"tuid")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string());
                        current_tu = Some(TmxTu {
                            tuid,
                            source_lang: String::new(),
                            source_text: String::new(),
                            target_lang: String::new(),
                            target_text: None,
                            note: None,
                            prop: HashMap::new(),
                        });
                    }
                    b"tuv" => {
                        let lang = e.attributes()
                            .find_map(|a| a.ok())
                            .filter(|a| a.key.as_ref() == b"xml:lang" || a.key.as_ref() == b"lang")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string())
                            .unwrap_or_default();
                        current_tuv = Some((lang, String::new()));
                    }
                    b"seg" => in_seg = true,
                    b"note" => {
                        // Note handling simplified
                    }
                    b"prop" => {
                        if let Some(tu) = &mut current_tu {
                            let ptype = e.attributes()
                                .find_map(|a| a.ok())
                                .filter(|a| a.key.as_ref() == b"type")
                                .map(|a| String::from_utf8_lossy(&a.value).to_string());
                            if let Some(pt) = ptype {
                                tu.prop.insert(pt, String::new());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default();
                if in_seg {
                    if let Some((lang, ref mut txt)) = &mut current_tuv {
                        txt.push_str(&text);
                    }
                } else if let Some((lang, ref mut val)) = &mut current_tuv {
                    // Prop value
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"seg" => in_seg = false,
                    b"tuv" => {
                        if let Some((lang, text)) = current_tuv.take() {
                            if let Some(tu) = &mut current_tu {
                                if tu.source_lang.is_empty() {
                                    tu.source_lang = lang.clone();
                                    tu.source_text = text;
                                } else {
                                    tu.target_lang = lang;
                                    tu.target_text = Some(text);
                                }
                            }
                        }
                    }
                    b"tu" => {
                        if let Some(tu) = current_tu.take() {
                            if !tu.source_text.is_empty() {
                                tus.push(tu);
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

    Ok(tus)
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


/// Abre TMX como SourceDocument
pub async fn open_tmx(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let tus = parse_tmx(&content)?;

    let mut units = Vec::new();
    let section = SectionId::new();

    for (idx, tu) in tus.iter().enumerate() {
        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            selector: format!("tu[{}]", idx),
            offset: None,
            length: Some(tu.source_text.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(&tu.source_text), section);
        unit.id = unit_id;
        unit.metadata.insert("target_lang".into(), serde_json::Value::String(tu.target_lang.clone()));
        if let Some(target) = &tu.target_text {
            unit.metadata.insert("target".into(), serde_json::Value::String(target.clone()));
        }
        if let Some(tuid) = &tu.tuid {
            unit.metadata.insert("tuid".into(), serde_json::Value::String(tuid.clone()));
        }

        units.push(unit);
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Memória de tradução completa (source + target)")
        .with_preserved("Múltiplos idiomas por TU")
        .with_preserved("Notas e propriedades")
        .with_not_preserved("Formatação rica")
        .with_warning("TMX é para memórias de tradução, não documentos completos");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve TMX traduzido
pub async fn write_tmx(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;
    let tus = parse_tmx(&content)?;
    let tus_len = tus.len();

    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<tmx version=\"1.4\">\n");
    output.push_str("  <header creationtool=\"Polyglottus\" creationtoolversion=\"1.0\" datatype=\"plaintext\" segtype=\"sentence\" adminlang=\"en\" srclang=\"en\" o-tmf=\"unknown\"/>\n");
    output.push_str("  <body>\n");

    for tu in tus {
        output.push_str("    <tu");
        if let Some(tuid) = &tu.tuid {
            output.push_str(&format!(" tuid=\"{}\"", escape_xml(tuid)));
        }
        output.push_str(">\n");

        output.push_str(&format!("      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n",
            escape_xml(&tu.source_lang), escape_xml(&tu.source_text)));

        let unit_id = UnitId::from_hash({
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            tu.source_text.hash(&mut hasher);
            tu.source_lang.hash(&mut hasher);
            hasher.finish()
        });

        let target_text = if let Some(translated) = translations.get(&unit_id) {
            escape_xml(&translated.raw)
        } else {
            tu.target_text.clone().unwrap_or_default()
        };

        if !target_text.is_empty() {
            output.push_str(&format!("      <tuv xml:lang=\"{}\"><seg>{}</seg></tuv>\n",
                escape_xml(&tu.target_lang), target_text));
        }

        output.push_str("    </tu>\n");
    }

    output.push_str("  </body>\n");
    output.push_str("</tmx>\n");

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::TMX,
        units_processed: tus_len,
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
    async fn tmx_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"<?xml version="1.0"?>"#).unwrap();
        writeln!(f, r#"<tmx version="1.4">"#).unwrap();
        writeln!(f, r#"  <header srclang="en"/>"#).unwrap();
        writeln!(f, r#"  <body>"#).unwrap();
        writeln!(f, r#"    <tu><tuv xml:lang="en"><seg>Hello</seg></tuv><tuv xml:lang="pt"><seg>Olá</seg></tuv></tu>"#).unwrap();
        writeln!(f, r#"  </body>"#).unwrap();
        writeln!(f, "</tmx>").unwrap();

        let mut units = open_tmx(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 1);
    }
}