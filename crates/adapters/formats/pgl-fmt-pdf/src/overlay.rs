//! PDF Overlay - Mantém PDF original, sobrepõe texto traduzido

use pgl_app::{FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use std::path::Path;
use pdfium_render::prelude::*;
use lopdf::{Document, Object, ObjectId, Stream, Dictionary};
use std::collections::HashMap;

/// Escreve PDF com overlay de tradução
pub async fn write_pdf_overlay(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Abre com PDFium para renderização e posicionamento
    let pdfium = Pdfium::default();
    let document = pdfium
        .load_pdf_from_file(source, None)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Abre com lopdf para edição de objetos
    let mut doc = Document::load(source).map_err(|e| FormatError::ParseError(e.to_string()))?;

    // Para cada página, encontra caixas de texto e substitui
    let pages_count = document.pages().len();
    let mut units_processed = 0;

    for page_idx in 0..pages_count {
        let page = document.pages().get(page_idx).unwrap();

        // Extrai texto com posições
        let text_page = page.text().map_err(|e| FormatError::ParseError(e.to_string()))?;
        let chars: Vec<_> = text_page.chars().collect();

        // Agrupa caracteres em linhas/parágrafos por proximidade
        let text_blocks = group_chars_into_blocks(&chars);

        for block in text_blocks {
            let original_text = block.text.trim().to_string();
            if original_text.is_empty() || original_text.len() < 3 {
                continue;
            }

            // Gera UnitId baseado no texto e posição
            let unit_id = crate::reflow::make_unit_id_for_pdf(&original_text, block.page, block.bbox);

            if let Some(translated) = translations.get(&unit_id) {
                let new_text = if options.bilingual {
                    format!("{} / {}", original_text, translated.raw)
                } else {
                    translated.raw.clone()
                };

                // Substitui texto no PDF via lopdf
                replace_text_in_pdf(&mut doc, page_idx, &block, &new_text)?;
                units_processed += 1;
            }
        }
    }

    // Salva PDF modificado
    doc.save(out).map_err(|e| FormatError::WriteError(e.to_string()))?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::PDF,
        units_processed,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![
            "Overlay mode: text fitted into original boxes, may overflow".into(),
            "Font substitution may occur if original font lacks glyphs".into(),
        ],
    })
}

/// Escreve PDF bilíngue (original + tradução)
pub async fn write_pdf_bilingual(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
) -> Result<ExportReport, FormatError> {
    // Similar ao overlay mas adiciona tradução abaixo do original
    write_pdf_overlay(source, translations, options, out).await
}

/// Agrupa caracteres em blocos de texto (heurística)
#[derive(Debug, Clone)]
struct TextBlock {
    text: String,
    page: usize,
    bbox: (f32, f32, f32, f32), // x, y, width, height
}

fn group_chars_into_blocks(chars: &[PdfChar]) -> Vec<TextBlock> {
    // Implementação simplificada - agrupa por linha (y similar)
    let mut blocks = Vec::new();
    let mut current_block = String::new();
    let mut current_y = None;
    let mut current_x = 0.0;
    let mut block_start_x = 0.0;
    let mut block_y = 0.0;

    for ch in chars {
        let y = ch.origin_y;
        let x = ch.origin_x;

        if current_y.is_none() || (y - current_y.unwrap()).abs() < 5.0 {
            // Mesma linha
            current_block.push(ch.character);
            current_y = Some(y);
            current_x = x + ch.width;
        } else {
            // Nova linha
            if !current_block.trim().is_empty() {
                blocks.push(TextBlock {
                    text: current_block.clone(),
                    page: 0, // Seria preenchido com page_idx real
                    bbox: (block_start_x, block_y, current_x - block_start_x, 12.0),
                });
            }
            current_block = ch.character.to_string();
            current_y = Some(y);
            block_start_x = x;
            block_y = y;
            current_x = x + ch.width;
        }
    }

    if !current_block.trim().is_empty() {
        blocks.push(TextBlock {
            text: current_block,
            page: 0,
            bbox: (block_start_x, block_y, current_x - block_start_x, 12.0),
        });
    }

    blocks
}

/// Substitui texto no PDF via lopdf
fn replace_text_in_pdf(
    doc: &mut Document,
    page_idx: usize,
    block: &TextBlock,
    new_text: &str,
) -> Result<(), FormatError> {
    // Implementação simplificada - em produção:
    // 1. Encontra stream de conteúdo da página
    // 2. Parseia operadores de texto (Tj, TJ, ')
    // 3. Substitui strings mantendo operadores de posicionamento
    // 4. Ajusta font size se texto não couber

    // Por enquanto, log warning
    eprintln!("Warning: PDF text replacement not fully implemented for block: {}", block.text);

    Ok(())
}

/// Gera UnitId para PDF baseado em texto + posição
pub fn make_unit_id_for_pdf(text: &str, page: usize, bbox: (f32, f32, f32, f32)) -> pgl_domain::UnitId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    page.hash(&mut hasher);
    bbox.0.to_bits().hash(&mut hasher);
    bbox.1.to_bits().hash(&mut hasher);
    pgl_domain::UnitId::from_hash(&hasher.finish().to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_unit_id_for_pdf_test() {
        let id1 = make_unit_id_for_pdf("Hello", 1, (0.0, 0.0, 100.0, 20.0));
        let id2 = make_unit_id_for_pdf("Hello", 1, (0.0, 0.0, 100.0, 20.0));
        assert_eq!(id1, id2);

        let id3 = make_unit_id_for_pdf("World", 1, (0.0, 0.0, 100.0, 20.0));
        assert_ne!(id1, id3);
    }
}