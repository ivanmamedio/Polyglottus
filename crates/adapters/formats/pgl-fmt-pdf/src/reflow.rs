//! PDF Reflow - Extrai texto e estrutura para exportar como EPUB/DOCX/HTML

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use zip::ZipArchive;
use pdfium_render::prelude::*;

/// Abre PDF no modo reflow
pub async fn open_pdf_reflow(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    let pdfium = Pdfium::default();
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| FormatError::ParseError(e.to_string()))?;

    let mut units = Vec::new();
    let mut section = SectionId::new();
    let mut global_idx = 0;

    let pages_count = document.pages().len();

    for (page_idx, page) in document.pages().iter().enumerate() {
        // Extrai texto da página
        let text_page = page.text().map_err(|e| FormatError::ParseError(e.to_string()))?;
        let text = text_page.text();

        if text.trim().is_empty() {
            continue;
        }

        // Divide em parágrafos heurísticos (quebras duplas de linha)
        let paragraphs: Vec<&str> = text.split("\n\n")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && s.len() > 10)
            .collect();

        for (para_idx, para) in paragraphs.iter().enumerate() {
            let unit_id = UnitId::new();
            let source_ref = SourceRef {
                internal_path: format!("page_{}.txt", page_idx + 1),
                selector: format!("paragraph[{}]", para_idx),
                offset: None,
                length: Some(para.len()),
            };

            let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(para), section);
            unit.id = unit_id;
            unit.metadata.insert("page".into(), serde_json::Value::Number((page_idx + 1).into()));
            unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
            units.push(unit);
            global_idx += 1;
        }

        // Nova seção a cada página (ou configurável)
        section = SectionId::new();
    }

    // Metadados do PDF
    let metadata = DocumentMetadata {
        title: document.metadata().title().map(|s| s.to_string()),
        author: document.metadata().author().map(|s| s.to_string()),
        subject: document.metadata().subject().map(|s| s.to_string()),
        description: document.metadata().keywords().map(|s| s.to_string()),
        created: document.metadata().creation_date().map(|d| d.to_string()),
        modified: document.metadata().modification_date().map(|d| d.to_string()),
        page_count: Some(pages_count),
        ..Default::default()
    };

    // Assets (imagens) - extrai imagens de cada página
    let mut assets = Vec::new();
    for (page_idx, page) in document.pages().iter().enumerate() {
        let images = page.images().map_err(|e| FormatError::ParseError(e.to_string()))?;
        for (img_idx, image) in images.iter().enumerate() {
            let bitmap = image.as_bitmap().map_err(|e| FormatError::ParseError(e.to_string()))?;
            let mut png_data = Vec::new();
            bitmap.write_png(&mut png_data).map_err(|e| FormatError::ParseError(e.to_string()))?;

            let hash = format!("{:x}", md5::compute(&png_data));
            assets.push(AssetRef {
                id: hash.clone(),
                internal_path: format!("page_{}/image_{}.png", page_idx + 1, img_idx),
                mime_type: "image/png".to_string(),
                size: png_data.len() as u64,
                hash,
            });
        }
    }

    let fidelity = default_zip_fidelity()
        .with_preserved("Texto em ordem de leitura")
        .with_preserved("Imagens extraídas")
        .with_preserved("Metadados do documento")
        .with_not_preserved("Layout original (colunas, tabelas, formatação)")
        .with_not_preserved("Fontes e estilos")
        .with_warning("Modo reflow: layout original NÃO é preservado. Use para conversão para EPUB/DOCX/HTML.")
        .with_warning("Tabelas e colunas podem ficar linearizadas");

    let empty_archive = ZipArchive::new(std::io::Cursor::new(Vec::new()))?;

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, assets, fidelity, metadata
    )))
}

/// Exporta PDF reflow para outros formatos (usa writers correspondentes)
pub async fn export_reflow_to_format(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
    target_format: OutputFormat,
) -> Result<ExportReport, FormatError> {
    // Abre PDF reflow
    let doc = open_pdf_reflow(source).await?;

    // Cria unidades traduzidas
    let mut translated_units = Vec::new();
    for unit_result in doc.units() {
        let unit = unit_result?;
        let unit_id = unit.id;

        let translated_text = if let Some(translated) = translations.get(&unit_id) {
            if options.bilingual {
                format!("{}\n\n{}", unit.text.raw, translated.raw)
            } else {
                translated.raw.clone()
            }
        } else {
            unit.text.raw.clone()
        };

        let mut translated_unit = unit.clone();
        translated_unit.apply_translation(MarkedText::new(&translated_text))?;
        translated_units.push(translated_unit);
    }

    // Reconstrói documento no formato alvo
    match target_format {
        OutputFormat::EPUB => export_to_epub(&translated_units, out, &doc.metadata()).await,
        OutputFormat::DOCX => export_to_docx(&translated_units, out, &doc.metadata()).await,
        OutputFormat::HTML => export_to_html(&translated_units, out, &doc.metadata()).await,
        OutputFormat::TXT => export_to_txt(&translated_units, out).await,
        OutputFormat::MD => export_to_md(&translated_units, out).await,
        _ => Err(FormatError::WriteError(format!("Unsupported output format for reflow: {:?}", target_format))),
    }
}

async fn export_to_epub(units: &[Unit], out: &Path, metadata: &DocumentMetadata) -> Result<ExportReport, FormatError> {
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let mut zip = ZipWriter::new(std::fs::File::create(out)?);
    let options_zip = FileOptions::default().compression_method(zip::CompressionMethod::Deflated).compression_level(Some(6));

    // mimetype
    zip.start_file("mimetype", FileOptions::default().compression_method(zip::CompressionMethod::Stored))?;
    zip.write_all(b"application/epub+zip")?;

    // container.xml
    zip.start_file("META-INF/container.xml", options_zip)?;
    zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#)?;

    // content.opf
    zip.start_file("OEBPS/content.opf", options_zip)?;
    let title = metadata.title.as_deref().unwrap_or("Translated Document");
    let author = metadata.author.as_deref().unwrap_or("Unknown");
    let lang = "pt-BR";

    let opf = format!(r#"<?xml version="1.0"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-id="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">polyglottus-{}</dc:identifier>
    <dc:title>{}</dc:title>
    <dc:creator>{}</dc:creator>
    <dc:language>{}</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="content" href="content.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="content"/>
  </spine>
</package>"#, uuid::Uuid::new_v4(), title, author, lang);

    zip.write_all(opf.as_bytes())?;

    // nav.xhtml
    zip.start_file("OEBPS/nav.xhtml", options_zip)?;
    zip.write_all(br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Navigation</title></head>
<body><nav epub:type="toc"><ol><li><a href="content.xhtml">Content</a></li></ol></nav></body>
</html>"#)?;

    // content.xhtml
    zip.start_file("OEBPS/content.xhtml", options_zip)?;
    let mut content = String::from(r#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>"#);
    content.push_str(title);
    content.push_str("</title></head><body>\n");

    for unit in units {
        content.push_str("<p>");
        content.push_str(&escape_xml(&unit.display_text()));
        content.push_str("</p>\n");
    }

    content.push_str("</body></html>");
    zip.write_all(content.as_bytes())?;

    zip.finish()?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::EPUB,
        units_processed: units.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["Generated from PDF reflow - layout not preserved".into()],
    })
}

async fn export_to_docx(units: &[Unit], out: &Path, metadata: &DocumentMetadata) -> Result<ExportReport, FormatError> {
    use docx_rs::{Docx, Paragraph, Run, Text};

    let mut doc = Docx::new();

    if let Some(title) = &metadata.title {
        doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text(title)).heading(1));
    }

    for unit in units {
        doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text(&unit.display_text())));
    }

    let bytes = docx_rs::to_bytes(&doc).map_err(|e| FormatError::WriteError(e.to_string()))?;
    tokio::fs::write(out, bytes).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::DOCX,
        units_processed: units.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["Generated from PDF reflow - layout not preserved".into()],
    })
}

async fn export_to_html(units: &[Unit], out: &Path, metadata: &DocumentMetadata) -> Result<ExportReport, FormatError> {
    let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<title>"#);
    html.push_str(metadata.title.as_deref().unwrap_or("Translated Document"));
    html.push_str("</title></head><body>\n");

    for unit in units {
        html.push_str("<p>");
        html.push_str(&escape_xml(&unit.display_text()));
        html.push_str("</p>\n");
    }

    html.push_str("</body></html>");

    tokio::fs::write(out, html).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::HTML,
        units_processed: units.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec!["Generated from PDF reflow".into()],
    })
}

async fn export_to_txt(units: &[Unit], out: &Path) -> Result<ExportReport, FormatError> {
    let mut text = String::new();
    for unit in units {
        text.push_str(&unit.display_text());
        text.push_str("\n\n");
    }
    tokio::fs::write(out, text).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::TXT,
        units_processed: units.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

async fn export_to_md(units: &[Unit], out: &Path) -> Result<ExportReport, FormatError> {
    let mut md = String::new();
    for unit in units {
        md.push_str(&unit.display_text());
        md.push_str("\n\n");
    }
    tokio::fs::write(out, md).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::MD,
        units_processed: units.len(),
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

    #[test]
    fn escape_xml_test() {
        assert_eq!(escape_xml("<test>"), "<test>");
        assert_eq!(escape_xml("hello"), "hello");
    }
}
