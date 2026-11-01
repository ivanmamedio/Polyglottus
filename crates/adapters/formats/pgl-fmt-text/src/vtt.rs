//! VTT - WebVTT Subtitle format

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use regex::Regex;
type SourceDoc = Box<dyn SourceDocument + 'static>;

#[derive(Debug, Clone)]
struct VttCue {
    identifier: Option<String>,
    start: String,
    end: String,
    settings: String,
    text: String,
}

/// Parse VTT content
fn parse_vtt(content: &str) -> Result<Vec<VttCue>, FormatError> {
    let mut cues = Vec::new();

    // Skip WEBVTT header
    let content = content.strip_prefix("WEBVTT").unwrap_or(content).trim_start();

    let cue_regex = Regex::new(r"(?:(\S+)\n)?(\d{2}:\d{2}:\d{2}\.\d{3})\s*-->\s*(\d{2}:\d{2}:\d{2}\.\d{3})(?:\s+(.+))?\n((?:.+\n)*)").unwrap();

    for cap in cue_regex.captures_iter(content) {
        let identifier = cap.get(1).map(|m| m.as_str().to_string());
        let start = cap.get(2).unwrap().as_str().to_string();
        let end = cap.get(3).unwrap().as_str().to_string();
        let settings = cap.get(4).map(|m| m.as_str().to_string()).unwrap_or_default();
        let text = cap.get(5).unwrap().as_str().trim().to_string();

        if !text.is_empty() {
            cues.push(VttCue { identifier, start, end, settings, text });
        }
    }

    Ok(cues)
}

/// Abre VTT como SourceDocument
pub async fn open_vtt(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let cues = parse_vtt(&content)?;

    let mut units = Vec::new();
    let section = SectionId::new();

    for (idx, cue) in cues.iter().enumerate() {
        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            selector: format!("cue[{}]", idx),
            offset: None,
            length: Some(cue.text.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Subtitle, MarkedText::new(&cue.text), section);
        unit.id = unit_id;
        unit.metadata.insert("start_time".into(), serde_json::Value::String(cue.start.clone()));
        unit.metadata.insert("end_time".into(), serde_json::Value::String(cue.end.clone()));
        unit.metadata.insert("settings".into(), serde_json::Value::String(cue.settings.clone()));
        if let Some(id) = &cue.identifier {
            unit.metadata.insert("identifier".into(), serde_json::Value::String(id.clone()));
        }

        units.push(unit);
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Timing preciso (milissegundos)")
        .with_preserved("Configurações de posicionamento")
        .with_preserved("Identificadores de cue")
        .with_not_preserved("Comentários (NOTE)")
        .with_warning("Expansão de texto pode dessincronizar legendas");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve VTT traduzido
pub async fn write_vtt(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;
    let cues = parse_vtt(&content)?;

    let mut output = String::from("WEBVTT\n\n");

    for cue in cues {
        let unit_id = UnitId::from_hash(&{
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            cue.identifier.hash(&mut hasher);
            cue.start.hash(&mut hasher);
            cue.text.hash(&mut hasher);
            hasher.finish().to_le_bytes()
        });

        let translated_text = if let Some(translated) = translations.get(&unit_id) {
            if options.bilingual {
                format!("{}\n{}", cue.text, translated.raw)
            } else {
                translated.raw.clone()
            }
        } else {
            cue.text
        };

        if let Some(id) = &cue.identifier {
            output.push_str(&format!("{}\n", id));
        }
        output.push_str(&format!("{} --> {}", cue.start, cue.end));
        if !cue.settings.is_empty() {
            output.push_str(&format!(" {}", cue.settings));
        }
        output.push_str(&format!("\n{}\n\n", translated_text));
    }

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::VTT,
        units_processed: cues.len(),
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
    async fn vtt_parse() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "WEBVTT").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "1").unwrap();
        writeln!(f, "00:00:01.000 --> 00:00:04.000").unwrap();
        writeln!(f, "Hello world").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "00:00:05.000 --> 00:00:08.000").unwrap();
        writeln!(f, "Second subtitle").unwrap();

        let units = open_vtt(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 2);
    }
}