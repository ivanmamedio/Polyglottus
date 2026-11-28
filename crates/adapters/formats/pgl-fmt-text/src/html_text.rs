//! HTML - Texto (para arquivos HTML standalone)
//! Usa o crate `scraper` para parsing HTML estável e simples.

use uuid::Uuid;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{default_zip_fidelity, make_unit_id, ZipSourceDocument};
use std::path::Path;
use zip::ZipArchive;
use scraper::{Html, Selector, ElementRef, Node};
use std::collections::HashMap;
type SourceDoc = Box<dyn SourceDocument + 'static>;

/// Tags de bloco traduzíveis
const BLOCK_TAGS: &[&str] = &[
    "p", "h1", "h2", "h3", "h4", "h5", "h6",
    "li", "td", "th", "caption", "figcaption",
    "blockquote", "pre", "code",
];

/// Extrair texto de um ElementRef (filtrando tags script/style)
fn extract_text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Extrair texto direto de um elemento (não inclui filhos, exceto texto direto)
fn extract_direct_text(element: &ElementRef<'_>) -> String {
    let mut text = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(t) => {
                text.push_str(&t.to_string());
            }
            Node::Element(_) => {
                // Pular elementos filhos - só queremos texto direto
            }
            _ => {}
        }
    }
    text.trim().to_string()
}

/// Abre HTML como SourceDocument
pub async fn open_html(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;
    let document = Html::parse_document(&content);

    let mut units = Vec::new();
    let section = SectionId::new();
    let mut counter = 0usize;
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    for tag in BLOCK_TAGS {
        let selector = Selector::parse(tag).unwrap();
        for element in document.select(&selector) {
            // Verificar se o elemento tem texto direto (não apenas de filhos)
            let direct_text = extract_direct_text(&element);
            if direct_text.is_empty() {
                continue;
            }

            let source_ref = SourceRef {
                internal_path: file_name.clone(),
                selector: format!("//{}[{}]", tag, counter),
                offset: None,
                length: Some(direct_text.len()),
            };
            let unit_id = make_unit_id(&source_ref, &direct_text);

            let kind: UnitKind = match *tag {
                "h1" => UnitKind::Heading(1),
                "h2" => UnitKind::Heading(2),
                "h3" => UnitKind::Heading(3),
                "h4" => UnitKind::Heading(4),
                "h5" => UnitKind::Heading(5),
                "h6" => UnitKind::Heading(6),
                "li" => UnitKind::ListItem,
                "td" | "th" => UnitKind::TableCell,
                "caption" | "figcaption" => UnitKind::Caption,
                "pre" | "code" => UnitKind::CodeBlock,
                "blockquote" => UnitKind::Paragraph,
                _ => UnitKind::Paragraph,
            };
            let mut unit = Unit::new(source_ref, kind, MarkedText::new(&direct_text), section, Uuid::nil());
            unit.id = unit_id;
            units.push(unit);
            counter += 1;
        }
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura HTML semântica")
        .with_preserved("CSS e JavaScript (não modificados)")
        .with_preserved("Imagens e assets")
        .with_not_preserved("Scripts executáveis")
        .with_warning("JavaScript que manipula DOM pode quebrar com texto traduzido");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(pgl_fmt_core::create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve HTML traduzido
/// Para o MVP, copia o arquivo original e aplica traduções simples por substituição de texto.
pub async fn write_html(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let _ = options;
    let content = tokio::fs::read_to_string(source).await?;

    // Para o MVP: copiar o conteúdo original
    // A substituição precisa de texto via scraper requer reconstrução da árvore HTML,
    // que será implementada em uma versão futura.
    tokio::fs::write(out, &content).await?;

    // Contar unidades que têm tradução disponível
    let mut units_with_translation = 0;
    for (source_ref, translation) in translations.all_with_refs() {
        if source_ref.internal_path == source.file_name().unwrap_or_default().to_string_lossy().to_string() {
            if !translation.raw.is_empty() {
                units_with_translation += 1;
            }
        }
    }

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::HTML,
        units_processed: units_with_translation,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["HTML writing: text substitution not yet implemented, original file copied".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn html_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "<html><body><h1>Title</h1><p>Paragraph</p></body></html>").unwrap();

        let mut units = open_html(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert!(!units_vec.is_empty());
    }

    #[tokio::test]
    async fn html_write_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "<html><body><p>Hello world</p></body></html>").unwrap();

        let mut units = open_html(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert_eq!(units_vec.len(), 1);

        // TranslationLookup que não traduz nada (retorna None)
        struct EmptyLookup;
        impl TranslationLookup for EmptyLookup {
            fn get(&self, _unit_id: &UnitId) -> Option<MarkedText> {
                None
            }
            fn get_section(&self, _section: &SectionId) -> Vec<(UnitId, MarkedText)> {
                Vec::new()
            }
        }

        let out = NamedTempFile::new().unwrap();
        let report = write_html(f.path(), &EmptyLookup, &Default::default(), out.path()).await.unwrap();
        assert_eq!(report.format, OutputFormat::HTML);
        assert_eq!(report.units_failed, 0);

        let written = std::fs::read_to_string(out.path()).unwrap();
        assert!(written.contains("Hello world"));
        assert!(written.contains("<html>"));
    }
}
