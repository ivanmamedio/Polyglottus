//! JSON i18n - Formatos de localização baseados em JSON

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use serde_json::{Value, Map};
type SourceDoc = Box<dyn SourceDocument + 'static>;

/// Detecta formato JSON i18n comum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonI18nFormat {
    /// { "key": "value" } - simples
    Simple,
    /// { "locale": "en", "messages": { "key": "value" } } - i18next-like
    I18next,
    /// { "key": { "message": "value", "description": "..." } } - Chrome extensions
    ChromeExt,
    /// { "key": ["value", "plural_form"] } - ICU/messageformat
    Icu,
}

/// Detecta formato
fn detect_format(json: &Value) -> JsonI18nFormat {
    if let Value::Object(obj) = json {
        // i18next
        if obj.contains_key("locale") && obj.contains_key("messages") {
            return JsonI18nFormat::I18next;
        }
        // Chrome extension
        if obj.values().any(|v| {
            if let Value::Object(o) = v {
                o.contains_key("message")
            } else { false }
        }) {
            return JsonI18nFormat::ChromeExt;
        }
        // ICU plural arrays
        if obj.values().any(|v| matches!(v, Value::Array(_))) {
            return JsonI18nFormat::Icu;
        }
    }
    JsonI18nFormat::Simple
}

/// Extrai pares chave-valor para tradução
fn extract_pairs(json: &Value, prefix: &str, pairs: &mut Vec<(String, String, Option<String>)>) {
    match json {
        Value::Object(obj) => {
            for (key, value) in obj {
                let new_prefix = if prefix.is_empty() { key.clone() } else { format!("{}.{}", prefix, key) };
                match value {
                    Value::String(s) => pairs.push((new_prefix, s.clone(), None)),
                    Value::Object(o) => {
                        // Chrome extension format: { "message": "...", "description": "..." }
                        if let Some(msg) = o.get("message").and_then(|v| v.as_str()) {
                            let desc = o.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                            pairs.push((new_prefix, msg.to_string(), desc));
                        } else {
                            extract_pairs(value, &new_prefix, pairs);
                        }
                    }
                    Value::Array(arr) => {
                        // ICU plurals: first element is singular
                        if let Some(Value::String(s)) = arr.first() {
                            pairs.push((new_prefix, s.clone(), Some(format!("plural: {:?}", arr))));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
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


/// Abre JSON i18n como SourceDocument
pub async fn open_json_i18n(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let json: Value = serde_json::from_str(&content)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    let format = detect_format(&json);
    let mut pairs = Vec::new();
    extract_pairs(&json, "", &mut pairs);

    let mut units = Vec::new();
    let section = SectionId::new();

    for (idx, (key, value, desc)) in pairs.iter().enumerate() {
        let unit_id = UnitId::new();
        let source_ref = SourceRef {
            internal_path: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            selector: format!("$.{}", key),
            offset: None,
            length: Some(value.len()),
        };

        let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(value), section);
        unit.id = unit_id;
        unit.metadata.insert("json_key".into(), serde_json::Value::String(key.clone()));
        unit.metadata.insert("format".into(), serde_json::Value::String(format!("{:?}", format)));
        if let Some(d) = desc {
            unit.metadata.insert("description".into(), serde_json::Value::String(d.clone()));
        }

        units.push(unit);
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura de chaves aninhadas")
        .with_preserved("Descrições/contexto (se houver)")
        .with_preserved("Plurais ICU (se detectados)")
        .with_not_preserved("Comentários JSON (não padrão)")
        .with_warning(&format!("Formato detectado automaticamente: {:?}", format));

    let empty_archive = ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve JSON i18n traduzido
pub async fn write_json_i18n(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;
    let mut json: Value = serde_json::from_str(&content)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    let format = detect_format(&json);
    let mut pairs = Vec::new();
    extract_pairs(&json, "", &mut pairs);

    // Aplica traduções
    let mut translated_count = 0;
    for (key, original, _) in &pairs {
        let unit_id = UnitId::from_hash({
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut hasher);
            original.hash(&mut hasher);
            hasher.finish()
        });

        if let Some(translated) = translations.get(&unit_id) {
            set_json_value(&mut json, key, &translated.raw);
            translated_count += 1;
        }
    }

    // Bilíngue: adiciona sufixo _translated ou cria estrutura paralela
    if options.bilingual {
        for (key, original, _) in &pairs {
            let unit_id = UnitId::from_hash({
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut hasher);
                original.hash(&mut hasher);
                hasher.finish()
            });

            if let Some(translated) = translations.get(&unit_id) {
                let translated_key = format!("{}_translated", key);
                set_json_value(&mut json, &translated_key, &format!("{} | {}", original, translated.raw));
            }
        }
    }

    let output = serde_json::to_string_pretty(&json)
        .map_err(|e| FormatError::WriteError(e.to_string()))?;

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::JSON_I18N,
        units_processed: pairs.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

fn set_json_value(json: &mut Value, path: &str, value: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for (i, part) in parts.iter().copied().enumerate() {
        if i == parts.len() - 1 {
            // Último nível - define valor
            if let Value::Object(obj) = current {
                // Verifica se é Chrome extension format
                if let Some(existing) = obj.get(part) {
                    if let Value::Object(ref o) = existing {
                        if o.contains_key("message") {
                            if let Value::Object(ref mut o) = obj.get_mut(part).unwrap() {
                                o.insert("message".to_string(), Value::String(value.to_string()));
                                return;
                            }
                        }
                    }
                }
                obj.insert(part.to_string(), Value::String(value.to_string()));
            }
        } else {
            // Navega
            if let Value::Object(obj) = current {
                current = obj.entry(part.to_string()).or_insert(Value::Object(Map::new()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn json_i18n_simple() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"greeting": "Hello", "farewell": "Goodbye"}}"#).unwrap();

        let mut units = open_json_i18n(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 2);
    }

    #[tokio::test]
    async fn json_i18n_chrome_ext() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"welcome": {{"message": "Welcome", "description": "Home page greeting"}}}}"#).unwrap();

        let mut units = open_json_i18n(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 1);
        assert_eq!(units_vec[0].as_ref().unwrap().metadata.get("description").unwrap().as_str().unwrap(), "Home page greeting");
    }
}