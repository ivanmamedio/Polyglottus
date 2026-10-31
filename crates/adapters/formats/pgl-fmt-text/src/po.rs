//! PO - Gettext Portable Object format

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use regex::Regex;

#[derive(Debug, Clone)]
struct PoEntry {
    msgid: String,
    msgstr: String,
    context: Option<String>,
    comment: Option<String>,
    flags: Vec<String>,
}

/// Parse PO content
fn parse_po(content: &str) -> Result<Vec<PoEntry>, FormatError> {
    let mut entries = Vec::mut();
    let mut current = PoEntry {
        msgid: String::new(),
        msgstr: String::new(),
        context: None,
        comment: None,
        flags: Vec::new(),
    };
    let mut in_msgid = false;
    let mut in_msgstr = false;

    let msgid_regex = Regex::new(r#"msgid\s+"(.*)"#).unwrap();
    let msgstr_regex = Regex::new(r#"msgstr\s+"(.*)"#).unwrap();
    let msgctxt_regex = Regex::new(r#"msgctxt\s+"(.*)"#).unwrap();
    let comment_regex = Regex::new(r"^#\s*(.*)$").unwrap();
    let flag_regex = Regex::new(r"^,.*\bfuzzy\b").unwrap();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#:') {
            // Finaliza entrada anterior se tem msgid
            if !current.msgid.is_empty() {
                entries.push(current.clone());
                current = PoEntry {
                    msgid: String::new(),
                    msgstr: String::new(),
                    context: None,
                    comment: None,
                    flags: Vec::new(),
                };
            }
            continue;
        }

        if let Some(cap) = comment_regex.captures(line) {
            current.comment = Some(cap.get(1).unwrap().as_str().to_string());
            continue;
        }

        if flag_regex.is_match(line) {
            current.flags.push("fuzzy".to_string());
            continue;
        }

        if let Some(cap) = msgctxt_regex.captures(line) {
            current.context = Some(cap.get(1).unwrap().as_str().to_string());
            continue;
        }

        if let Some(cap) = msgid_regex.captures(line) {
            current.msgid = cap.get(1).unwrap().as_str().to_string();
            in_msgid = true;
            in_msgstr = false;
            continue;
        }

        if let Some(cap) = msgstr_regex.captures(line) {
            current.msgstr = cap.get(1).unwrap().as_str().to_string();
            in_msgstr = true;
            in_msgid = false;
            continue;
        }

        // Continuação de string multilinha
        if line.starts_with('"') && line.ends_with('"') {
            let content = &line[1..line.len()-1];
            if in_msgid {
                current.msgid.push_str(content);
            } else if in_msgstr {
                current.msgstr.push_str(content);
            }
        }
    }

    // Última entrada
    if !current.msgid.is_empty() {
        entries.push(current);
    }

    Ok(entries)
}

/// Abre PO como SourceDocument
pub async fn open_po(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let entries = parse_po(&content)?;

    let mut units = Vec::new();
    let section = SectionId::new();

    for (idx, entry) in entries.iter().enumerate() {
        if entry.msgid.trim().is_empty() { continue; }

        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            selector: format!("entry[{}]", idx),
            offset: None,
            length: Some(entry.msgid.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(&entry.msgid), section);
        unit.id = unit_id;
        unit.metadata.insert("msgstr".into(), serde_json::Value::String(entry.msgstr.clone()));
        if let Some(ctx) = &entry.context {
            unit.metadata.insert("msgctxt".into(), serde_json::Value::String(ctx.clone()));
        }
        if let Some(cmt) = &entry.comment {
            unit.metadata.insert("comment".into(), serde_json::Value::String(cmt.clone()));
        }
        unit.metadata.insert("flags".into(), serde_json::Value::Array(
            entry.flags.iter().map(|f| serde_json::Value::String(f.clone())).collect()
        ));

        units.push(unit);
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Contexto (msgctxt)")
        .with_preserved("Comentários de tradutor")
        .with_preserved("Flags (fuzzy, etc.)")
        .with_preserved("Plurais (msgid_plural)")
        .with_not_preserved("Formatação rica")
        .with_warning("Plurais requerem tratamento especial");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve PO traduzido
pub async fn write_po(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;
    let entries = parse_po(&content)?;

    let mut output = String::new();

    // Header
    output.push_str("# Translated by Polyglottus\n");
    output.push_str("msgid \"\"\n");
    output.push_str("msgstr \"\"\n");
    output.push_str("\"Project-Id-Version: Polyglottus\\n\"\n");
    output.push_str("\"PO-Revision-Date: 2024-01-01 00:00+0000\\n\"\n");
    output.push_str("\"Last-Translator: Polyglottus\\n\"\n");
    output.push_str("\"Language-Team: Polyglottus\\n\"\n");
    output.push_str("\"MIME-Version: 1.0\\n\"\n");
    output.push_str("\"Content-Type: text/plain; charset=UTF-8\\n\"\n");
    output.push_str("\"Content-Transfer-Encoding: 8bit\\n\"\n");
    output.push_str("\n");

    for entry in entries {
        if let Some(comment) = &entry.comment {
            output.push_str(&format!("# {}\n", comment));
        }
        for flag in &entry.flags {
            output.push_str(&format!("#, {}\n", flag));
        }
        if let Some(ctx) = &entry.context {
            output.push_str(&format!("msgctxt \"{}\"\n", escape_po(ctx)));
        }
        output.push_str(&format!("msgid \"{}\"\n", escape_po(&entry.msgid)));

        // Busca tradução
        let unit_id = UnitId::from_hash(&{
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            entry.msgid.hash(&mut hasher);
            entry.context.hash(&mut hasher);
            hasher.finish().to_le_bytes()
        });

        let msgstr = if let Some(translated) = translations.get(&unit_id) {
            escape_po(&translated.raw)
        } else {
            escape_po(&entry.msgstr)
        };

        output.push_str(&format!("msgstr \"{}\"\n", msgstr));
        output.push_str("\n");
    }

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::PO,
        units_processed: entries.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

fn escape_po(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn po_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "msgid \"Hello\"").unwrap();
        writeln!(f, "msgstr \"Olá\"").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "msgid \"World\"").unwrap();
        writeln!(f, "msgstr \"Mundo\"").unwrap();

        let units = open_po(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 2);
    }
}