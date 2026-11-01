//! SRT - SubRip Subtitle format

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use regex::Regex;
type SourceDoc = Box<dyn SourceDocument + 'static>;

#[derive(Debug, Clone)]
struct SubtitleEntry {
    index: usize,
    start: String,
    end: String,
    text: String,
}

/// Parse SRT content
fn parse_srt(content: &str) -> Result<Vec<SubtitleEntry>, FormatError> {
    let mut entries = Vec::new();
    let blocks: Vec<&str> = content.split("\n\n").collect();

    let time_regex = Regex::new(r"(\d{2}:\d{2}:\d{2},\d{3})\s*-->\s*(\d{2}:\d{2}:\d{2},\d{3})").unwrap();

    for block in blocks {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 3 { continue; }

        let index = lines[0].parse::<usize>().unwrap_or(0);
        let time_match = time_regex.captures(lines[1]);
        if time_match.is_none() { continue; }

        let caps = time_match.unwrap();
        let start = caps.get(1).unwrap().as_str().to_string();
        let end = caps.get(2).unwrap().as_str().to_string();
        let text = lines[2..].join("\n");

        entries.push(SubtitleEntry { index, start, end, text });
    }

    Ok(entries)
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


/// Abre SRT como SourceDocument
pub async fn open_srt(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let entries = parse_srt(&content)?;

    let mut units = Vec::new();
    let section = SectionId::new();

    for entry in entries {
        if entry.text.trim().is_empty() { continue; }

        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            selector: format!("subtitle[{}]", entry.index),
            offset: None,
            length: Some(entry.text.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Subtitle, MarkedText::new(&entry.text), section);
        unit.id = unit_id;
        // Armazena timing como metadata
        unit.metadata.insert("start_time".into(), serde_json::Value::String(entry.start));
        unit.metadata.insert("end_time".into(), serde_json::Value::String(entry.end));
        unit.metadata.insert("index".into(), serde_json::Value::Number(entry.index.into()));

        units.push(unit);
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Timing (início/fim)")
        .with_preserved("Numeração sequencial")
        .with_preserved("Formatação de texto básica (itálico, negrito via tags)")
        .with_not_preserved("Posicionamento avançado")
        .with_warning("Expansão de texto pode dessincronizar legendas");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve SRT traduzido
pub async fn write_srt(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;
    let entries = parse_srt(&content)?;
    let entries_len = entries.len();

    let mut output = String::new();

    for entry in entries {
        let unit_id = UnitId::from_hash({
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            entry.index.hash(&mut hasher);
            entry.text.hash(&mut hasher);
            hasher.finish()
        });

        let translated_text = if let Some(translated) = translations.get(&unit_id) {
            if options.bilingual {
                format!("{}\n{}", entry.text, translated.raw)
            } else {
                translated.raw.clone()
            }
        } else {
            entry.text
        };

        output.push_str(&format!("{}\n{} --> {}\n{}\n\n", entry.index, entry.start, entry.end, translated_text));
    }

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::SRT,
        units_processed: entries_len,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn srt_parse() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "1").unwrap();
        writeln!(f, "00:00:01,000 --> 00:00:04,000").unwrap();
        writeln!(f, "Hello world").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "2").unwrap();
        writeln!(f, "00:00:05,000 --> 00:00:08,000").unwrap();
        writeln!(f, "Second subtitle").unwrap();

        let mut units = open_srt(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 2);
    }
}