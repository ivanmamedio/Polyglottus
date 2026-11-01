//! HTML - Texto (para arquivos HTML standalone)

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use html5ever::parse_document;
use html5ever::rcdom::RcDom;
use html5ever::ParseOpts;
type SourceDoc = Box<dyn SourceDocument + 'static>;

/// Abre HTML como SourceDocument
pub async fn open_html(path: &Path) -> Result<SourceDoc, FormatError> {
    let content = tokio::fs::read_to_string(path).await?;

    let dom = parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(content.into_bytes()))?;

    let mut units = Vec::new();
    let section = SectionId::new();

    fn walk(node: &html5ever::rcdom::Handle, units: &mut Vec<Unit>, section: SectionId, file_name: &str, idx: &mut usize) {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, .. } => {
                let tag_name = name.local.to_string().to_lowercase();

                // Skip script, style, etc.
                if matches!(tag_name.as_str(), "script" | "style" | "noscript" | "template" | "head" | "meta" | "link" | "title") {
                    return;
                }

                let kind = match tag_name.as_str() {
                    "p" | "div" | "article" | "section" | "blockquote" => UnitKind::Paragraph,
                    "h1" => UnitKind::Heading(1),
                    "h2" => UnitKind::Heading(2),
                    "h3" => UnitKind::Heading(3),
                    "h4" => UnitKind::Heading(4),
                    "h5" => UnitKind::Heading(5),
                    "h6" => UnitKind::Heading(6),
                    "li" => UnitKind::ListItem,
                    "td" | "th" => UnitKind::TableCell,
                    "figcaption" => UnitKind::Caption,
                    "pre" | "code" => UnitKind::CodeBlock,
                    _ => UnitKind::Other,
                };

                let text = extract_text(node);
                if !text.trim().is_empty() && !matches!(kind, UnitKind::Other) {
                    let unit_id = UnitId::new();
                    let source_ref = SourceRef {
                        internal_path: file_name.to_string(),
                        selector: format!("//{}", tag_name),
                        offset: None,
                        length: Some(text.len()),
                    };
                    let mut unit = Unit::new(source_ref, kind, MarkedText::new(&text), section);
                    unit.id = unit_id;
                    units.push(unit);
                    *idx += 1;
                }

                for child in node.children.borrow().iter() {
                    walk(child, units, section, file_name, idx);
                }
            }
            _ => {}
        }
    }

    fn extract_text(node: &html5ever::rcdom::Handle) -> String {
        let mut text = String::new();
        fn collect(node: &html5ever::rcdom::Handle, text: &mut String) {
            let node_data = node.data.borrow();
            match &node_data.node_type {
                html5ever::rcdom::NodeData::Text { contents } => {
                    text.push_str(&contents.borrow());
                }
                html5ever::rcdom::NodeData::Element { .. } => {
                    for child in node.children.borrow().iter() {
                        collect(child, text);
                    }
                }
                _ => {}
            }
        }
        collect(node, &mut text);
        text.trim().to_string()
    }

    let mut idx = 0;
    walk(&dom.document, &mut units, section, &path.file_name().unwrap_or_default().to_string_lossy(), &mut idx);

    let fidelity = default_zip_fidelity()
        .with_preserved("Estrutura HTML semântica")
        .with_preserved("CSS e JavaScript (não modificados)")
        .with_preserved("Imagens e assets")
        .with_not_preserved("Scripts executáveis")
        .with_warning("JavaScript que manipula DOM pode quebrar com texto traduzido");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, Default::default()
    )))
}

/// Escreve HTML traduzido
pub async fn write_html(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    let content = tokio::fs::read_to_string(source).await?;

    let dom = parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(content.into_bytes()))?;

    // Aplica traduções no DOM
    let mut idx = 0;
    fn apply_translations(node: &html5ever::rcdom::Handle, translations: &dyn TranslationLookup, options: &ExportOptions, idx: &mut usize) {
        let node_data = node.data.borrow();

        match &node_data.node_type {
            html5ever::rcdom::NodeData::Element { name, .. } => {
                let tag_name = name.local.to_string().to_lowercase();

                if !matches!(tag_name.as_str(), "script" | "style" | "noscript" | "template" | "head" | "meta" | "link" | "title") {
                    let kind = match tag_name.as_str() {
                        "p" | "div" | "article" | "section" | "blockquote" => UnitKind::Paragraph,
                        "h1" => UnitKind::Heading(1),
                        "h2" => UnitKind::Heading(2),
                        "h3" => UnitKind::Heading(3),
                        "h4" => UnitKind::Heading(4),
                        "h5" => UnitKind::Heading(5),
                        "h6" => UnitKind::Heading(6),
                        "li" => UnitKind::ListItem,
                        "td" | "th" => UnitKind::TableCell,
                        "figcaption" => UnitKind::Caption,
                        "pre" | "code" => UnitKind::CodeBlock,
                        _ => UnitKind::Other,
                    };

                    if !matches!(kind, UnitKind::Other) {
                        let text = extract_text(node);
                        if !text.trim().is_empty() {
                            let unit_id = UnitId::from_hash(&{
                                use std::hash::{Hash, Hasher};
                                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                                tag_name.hash(&mut hasher);
                                idx.hash(&mut hasher);
                                hasher.finish().to_le_bytes()
                            });

                            if let Some(translated) = translations.get(&unit_id) {
                                let new_text = if options.bilingual {
                                    format!("{} ⟦ ⟧ {}", text, translated.raw)
                                } else {
                                    translated.raw.clone()
                                };
                                replace_text(node, &new_text);
                            }
                            *idx += 1;
                        }
                    }
                }

                for child in node.children.borrow().iter() {
                    apply_translations(child, translations, options, idx);
                }
            }
            _ => {}
        }
    }

    fn extract_text(node: &html5ever::rcdom::Handle) -> String {
        let mut text = String::new();
        fn collect(node: &html5ever::rcdom::Handle, text: &mut String) {
            let node_data = node.data.borrow();
            match &node_data.node_type {
                html5ever::rcdom::NodeData::Text { contents } => {
                    text.push_str(&contents.borrow());
                }
                html5ever::rcdom::NodeData::Element { .. } => {
                    for child in node.children.borrow().iter() {
                        collect(child, text);
                    }
                }
                _ => {}
            }
        }
        collect(node, &mut text);
        text.trim().to_string()
    }

    fn replace_text(node: &html5ever::rcdom::Handle, new_text: &str) {
        // Remove existing text children and add new one
        let mut children = node.children.borrow_mut();
        children.retain(|child| {
            let child_data = child.data.borrow();
            !matches!(child_data.node_type, html5ever::rcdom::NodeData::Text { .. })
        });
        children.push(html5ever::rcdom::Handle::new(html5ever::rcdom::NodeData::Text {
            contents: std::cell::RefCell::new(new_text.to_string()),
        }));
    }

    apply_translations(&dom.document, translations, options, &mut idx);

    // Serializa
    let mut output = Vec::new();
    html5ever::serialize(&mut output, &dom.document, html5ever::serialize::SerializeOpts::default())?;

    tokio::fs::write(out, output).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::HTML,
        units_processed: idx,
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
    async fn html_basic() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "<html><body><h1>Title</h1><p>Paragraph</p></body></html>").unwrap();

        let units = open_html(f.path()).await.unwrap();
        let units_vec: Vec<_> = units.units().collect();
        assert!(!units_vec.is_empty());
    }
}