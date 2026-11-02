//! PDF Reflow - Extrai texto e estrutura para exportar como EPUB/DOCX/HTML
use uuid::Uuid;

use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef};
use pgl_app::{SourceDocument, AssetRef, FidelityReport, DocumentMetadata, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::{ZipSourceDocument, default_zip_fidelity};
use std::path::Path;
use lopdf::{Document as LoppdfDocument, Object, ObjectId, Stream};
use std::collections::HashMap;
use zip::ZipWriter;
use std::io::{Cursor, Write};
use tokio::fs;

pub fn create_empty_zip() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        zip.finish().unwrap();
    }
    buf
}

/// Abre PDF no modo reflow (extrai texto real usando lopdf)
pub async fn open_pdf_reflow(path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
    // Usa tokio::task::spawn_blocking para CPU-intensive PDF parsing
    let path = path.to_path_buf();
    let doc = tokio::task::spawn_blocking(move || {
        LoppdfDocument::load(&path)
    }).await
    .map_err(|e| FormatError::ParseError(e.to_string()))?
    .map_err(|e| FormatError::ParseError(e.to_string()))?;

    let mut units = Vec::new();
    let mut section = SectionId::new();
    let mut global_idx = 0;

    // Extrai texto de cada página
    let pages: Vec<_> = doc.get_pages().into_iter().collect();
    
    for (page_num, (_, page_id)) in pages.iter().enumerate() {
        let text = extract_page_text(&doc, *page_id);
        
        if text.trim().is_empty() {
            continue;
        }

        // Divide em parágrafos
        let paragraphs: Vec<&str> = text.split("\n\n")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && s.len() > 10)
            .collect();

        for (para_idx, para) in paragraphs.iter().copied().enumerate() {
            let unit_id = UnitId::new();
            let source_ref = SourceRef {
                internal_path: format!("page_{}.txt", page_num + 1),
                selector: format!("paragraph[{}]", para_idx),
                offset: None,
                length: Some(para.len()),
            };

            let mut unit = Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(para), section, Uuid::nil());
            unit.id = unit_id;
            unit.metadata.insert("page".into(), serde_json::Value::Number((page_num + 1).into()));
            unit.metadata.insert("global_index".into(), serde_json::Value::Number(global_idx.into()));
            units.push(unit);
            global_idx += 1;
        }
    }

    // Metadados do documento
    let title = extract_pdf_title(&doc);

    let metadata = DocumentMetadata {
        title,
        ..Default::default()
    };

    let fidelity = default_zip_fidelity()
        .with_preserved("Texto em ordem de leitura")
        .with_preserved("Estrutura de páginas")
        .with_not_preserved("Layout original (colunas, tabelas, formatação)")
        .with_not_preserved("Imagens, gráficos, fontes")
        .with_warning("Modo reflow: layout complexo pode não ser preservado");

    let empty_archive = zip::ZipArchive::new(std::io::Cursor::new(create_empty_zip())).unwrap();

    Ok(Box::new(ZipSourceDocument::new(
        empty_archive, units, vec![], fidelity, metadata
    )))
}

/// Exporta PDF reflow para outros formatos
pub async fn export_reflow_to_format(
    source: &Path,
    translations: &dyn TranslationLookup,
    options: &ExportOptions,
    out: &Path,
    target_format: OutputFormat,
) -> Result<ExportReport, FormatError> {
    let mut doc = open_pdf_reflow(source).await?;

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
        translated_unit.apply_translation(MarkedText::new(&translated_text))
            .map_err(|e| FormatError::ValidationError(e.to_string()))?;
        translated_units.push(translated_unit);
    }

    match target_format {
        OutputFormat::TXT => export_to_txt(&translated_units, out).await,
        OutputFormat::MD => export_to_md(&translated_units, out).await,
        OutputFormat::EPUB => export_to_epub(&translated_units, out).await,
        OutputFormat::DOCX => export_to_docx(&translated_units, out).await,
        _ => Err(FormatError::WriteError(format!("Unsupported output format for reflow: {:?}", target_format))),
    }
}

async fn export_to_txt(units: &[Unit], out: &Path) -> Result<ExportReport, FormatError> {
    let mut text = String::new();
    for unit in units {
        text.push_str(&unit.display_text());
        text.push_str("\n\n");
    }
    fs::write(out, text).await?;

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
    fs::write(out, md).await?;

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

async fn export_to_epub(units: &[Unit], out: &Path) -> Result<ExportReport, FormatError> {
    let mut text = String::new();
    for unit in units {
        text.push_str(&unit.display_text());
        text.push_str("\n\n");
    }
    
    // Cria EPUB simples
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        zip.start_file("mimetype", options)?;
        zip.write_all(b"application/epub+zip")?;

        zip.start_file("META-INF/container.xml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#)?;

        let escaped = text.replace("&", "&").replace("<", "<").replace(">", ">");
        let content = format!(r#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Document</title></head>
<body>{}</body>
</html>"#, escaped.replace("\n\n", "</p><p>").replace("\n", "<br/>"));

        zip.start_file("OEBPS/content.opf", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-id="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">pdf-reflow</dc:identifier>
    <dc:title>PDF Reflow Export</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#)?;

        zip.start_file("OEBPS/ch1.xhtml", options)?;
        zip.write_all(content.as_bytes())?;
        zip.finish()?;
    }

    fs::write(out, buf).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::EPUB,
        units_processed: units.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

async fn export_to_docx(units: &[Unit], out: &Path) -> Result<ExportReport, FormatError> {
    let mut text = String::new();
    for unit in units {
        text.push_str(&unit.display_text());
        text.push_str("\n\n");
    }

    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(6));

        zip.start_file("[Content_Types].xml", options)?;
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#)?;

        zip.start_file("_rels/.rels", options)?;
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#)?;

        let escaped = text.replace("&", "&").replace("<", "<").replace(">", ">")
            .replace("\n\n", "</w:p><w:p><w:r><w:t>").replace("\n", "<w:br/>");
        let content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>{}</w:t></w:r></w:p>
  </w:body>
</w:document>"#, escaped);

        zip.start_file("word/document.xml", options)?;
        zip.write_all(content.as_bytes())?;
        zip.finish()?;
    }

    fs::write(out, buf).await?;

    Ok(ExportReport {
        output_path: out.display().to_string(),
        format: OutputFormat::DOCX,
        units_processed: units.len(),
        units_with_warnings: 0,
        units_failed: 0,
        output_size: std::fs::metadata(out)?.len(),
        export_time_ms: 0,
        warnings: vec![],
    })
}

/// Extrai texto de uma página PDF
fn extract_page_text(doc: &LoppdfDocument, page_id: ObjectId) -> String {
    let mut text = String::new();
    
    // Get page content stream
    if let Ok(content) = doc.get_page_content(page_id) {
        // The content is already the raw stream bytes, parse it directly
        text = extract_text_from_stream(&content);
    }
    
    text
}

/// Extrai texto bruto de stream
fn extract_text_from_stream(data: &[u8]) -> String {
    let mut text = String::new();
    let mut in_string = false;
    let mut escape = false;
    let mut buffer = Vec::new();
    
    for &b in data {
        if !in_string {
            if b == b'(' {
                in_string = true;
                buffer.clear();
            }
        } else {
            if escape {
                buffer.push(b);
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b')' {
                in_string = false;
                if let Ok(s) = std::str::from_utf8(&buffer) {
                    text.push_str(s);
                    text.push(' ');
                }
                buffer.clear();
            } else {
                buffer.push(b);
            }
        }
    }
    text
}

/// Extrai título do PDF
fn extract_pdf_title(doc: &LoppdfDocument) -> Option<String> {
    if let Ok(info) = doc.trailer.get(b"Info") {
        if let Object::Dictionary(dict) = info {
            if let Ok(Object::String(ref title, _)) = dict.get(b"Title") {
                return std::str::from_utf8(title).ok().map(|s| s.to_string());
            }
        }
    }
    None
}

/// Reader PDF Reflow
pub struct PdfReflowReader;

impl PdfReflowReader {
    pub fn new() -> Self { Self }
}

#[async_trait::async_trait]
impl pgl_app::FormatReader for PdfReflowReader {
    fn id(&self) -> pgl_app::FormatId { pgl_app::FormatId::PDF }
    fn extensions(&self) -> &'static [&'static str] { &[".pdf"] }
    fn sniff(&self, name: &str, _head: &[u8]) -> pgl_app::Confidence {
        if name.to_lowercase().ends_with(".pdf") { pgl_app::Confidence::high() } else { pgl_app::Confidence::none() }
    }
    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument + 'static>, pgl_app::FormatError> {
        open_pdf_reflow(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflow_placeholder() {
        // Teste de compilação apenas
    }
}