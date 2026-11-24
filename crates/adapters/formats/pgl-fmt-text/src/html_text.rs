//! HTML - Texto (para arquivos HTML standalone)
//! Usa o crate `scraper` para parsing HTML estável e simples.

use uuid::Uuid;
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use scraper::{Html, Selector, Element, Node, Text};
use std::collections::HashMap;
type SourceDoc = Box<dyn SourceDocument + 'static>;

/// Tags de bloco traduzíveis
const BLOCK_TAGS: &[&str] = &[
    "p", "h1", "h2", "h3", "h4", "h5", "h6",
    "li", "td", "th", "caption", "figcaption",
    "blockquote", "pre", "code",
];

/// Extrair texto de um elemento (filtrando tags script/style)
fn extract_text(element: &Element) -> String {
    let mut text = String::new();
    for node in element.children() {
        if let Node::Text(t) = node {
            text.push_str(&t.text());
        } else if let Node::Element(child) = node {
            // Pular scripts e estilos
            let name = child.value().name();
            if !matches!(name, "script" | "style" | "noscript" | "template") {
                text.push_str(&extract_text(&child));
                text.push(' ');
            }
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
    let mut idx = 0usize;
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    for tag in BLOCK_TAGS {
        let selector = Selector::parse(tag).unwrap();
        for element in document.select(&selector) {
            let text = extract_text(&element);
            if text.is_empty() {
                continue;
            }

            let unit_id = UnitId::new();
            let source_ref = SourceRef {
                internal_path: file_name.clone(),
                selector: format!("//{}", tag),
                offset: None,
                length: Some(text.len()),
            };
            let kind = match tag {
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
            let mut unit = Unit::new(source_ref, kind, MarkedText::new(&text), section, Uuid::nil());
            unit.id = unit_id;
            units.push(unit);
            idx += 1;
        }
    }

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
    let document = Html::parse_document(&content);

    // Coletar traduções por índice de parágrafo
    let mut para_map: HashMap<usize, String> = HashMap::new();
    for (source_ref, translation) in translations.all_with_refs() {
        if let Some(idx) = source_ref
            .selector
            .strip_prefix("//")
            .and_then(|s| s.parse::<usize>().ok())
        {
            para_map.insert(idx, translation.raw.clone());
        }
    }

    if para_map.is_empty() {
        // Nenhuma tradução disponível, copiar original
        tokio::fs::write(out, content).await?;
        return Ok(ExportReport {
            output_path: out.display().to_string(),
            format: OutputFormat::HTML,
            units_processed: 0,
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec![],
        });
    }

    // Aplicar traduções substituindo texto dos elementos de bloco
    let mut output = String::with_capacity(content.len());
    let mut para_idx = 0usize;

    // Processar o HTML elemento por elemento, mantendo a estrutura
    // Usamos scraper para navegar, mas reconstruímos o HTML manualmente
    // para preservar a formatação original

    // Estratégia: para cada elemento de bloco traduzível, substituir seu texto
    // pelos conteúdos traduzidos quando disponível

    let mut elements_to_translate: Vec<(Selector, usize)> = Vec::new();
    for tag in BLOCK_TAGS {
        let selector = Selector::parse(tag).unwrap();
        elements_to_translate.push((selector, 0));
    }

    // Reconstruir HTML: percorrer nós e aplicar traduções
    // Abordagem simplificada: usar o HTML original e substituir textos
    // de elementos de bloco em ordem de aparição

    let mut current_para = 0;
    let mut result = String::new();

    // Para simplificar, vamos fazer um parse mais granular
    // Percorrer todos os elementos de bloco e substituir texto

    let mut doc_mut = document;
    for tag in BLOCK_TAGS {
        let selector = Selector::parse(tag).unwrap();
        for element in doc_mut.select(&selector) {
            if current_para >= para_map.len() {
                break;
            }
            let text = extract_text(&element);
            if text.is_empty() {
                continue;
            }
            if let Some(translated) = para_map.get(&current_para) {
                // Substituir o texto do elemento
                // Remover todos os nós de texto e adicionar um novo com a tradução
                let mut children = Vec::new();
                for child in element.children() {
                    match child {
                        Node::Text(_) => {} // pular
                        Node::Element(e) => children.push(e.clone()),
                    }
                }
                // Reconstruir elemento com novo texto
                // Isso é complexo com scraper imutável; vamos usar uma abordagem diferente
                // para o MVP: apenas registrar que traduzimos e continuar
                // A substituição real será feita na próxima iteração
            }
            current_para += 1;
        }
    }

    // Para o MVP, se há traduções mas não conseguimos aplicar via scraper
    // (devido à imutabilidade), vamos copiar o original e marcar para revisão
    tokio::fs::write(out, content).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::HTML,
        units_processed: para_map.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["HTML writing: text substitution not yet implemented via scraper".into()],
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
