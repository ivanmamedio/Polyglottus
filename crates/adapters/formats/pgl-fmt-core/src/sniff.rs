//! Detecção de formato (sniffing) por magic bytes e extensão

use std::path::Path;
use pgl_app::{FormatId, Confidence};

/// Sniffer de formato
pub struct FormatSniffer {
    rules: Vec<SniffRule>,
}

#[derive(Debug, Clone)]
struct SniffRule {
    format_id: FormatId,
    extensions: &'static [&'static str],
    magic_bytes: &'static [(&'static [u8], usize)], // (bytes, offset)
    mime_types: &'static [&'static str],
}

impl FormatSniffer {
    pub fn new() -> Self {
        Self {
            rules: vec![
                // EPUB
                SniffRule {
                    format_id: FormatId::EPUB,
                    extensions: &[".epub"],
                    magic_bytes: &[(b"PK\x03\x04", 0)], // ZIP
                    mime_types: &["application/epub+zip"],
                },
                // DOCX
                SniffRule {
                    format_id: FormatId::DOCX,
                    extensions: &[".docx"],
                    magic_bytes: &[(b"PK\x03\x04", 0)],
                    mime_types: &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"],
                },
                // PPTX
                SniffRule {
                    format_id: FormatId::PPTX,
                    extensions: &[".pptx"],
                    magic_bytes: &[(b"PK\x03\x04", 0)],
                    mime_types: &["application/vnd.openxmlformats-officedocument.presentationml.presentation"],
                },
                // XLSX
                SniffRule {
                    format_id: FormatId::XLSX,
                    extensions: &[".xlsx"],
                    magic_bytes: &[(b"PK\x03\x04", 0)],
                    mime_types: &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"],
                },
                // ODT
                SniffRule {
                    format_id: FormatId::ODT,
                    extensions: &[".odt"],
                    magic_bytes: &[(b"PK\x03\x04", 0)],
                    mime_types: &["application/vnd.oasis.opendocument.text"],
                },
                // PDF
                SniffRule {
                    format_id: FormatId::PDF,
                    extensions: &[".pdf"],
                    magic_bytes: &[(b"%PDF", 0)],
                    mime_types: &["application/pdf"],
                },
                // HTML
                SniffRule {
                    format_id: FormatId::HTML,
                    extensions: &[".html", ".htm", ".xhtml"],
                    magic_bytes: &[],
                    mime_types: &["text/html", "application/xhtml+xml"],
                },
                // TXT/MD
                SniffRule {
                    format_id: FormatId::TXT,
                    extensions: &[".txt", ".text"],
                    magic_bytes: &[],
                    mime_types: &["text/plain"],
                },
                SniffRule {
                    format_id: FormatId::MD,
                    extensions: &[".md", ".markdown", ".mdown", ".mkd"],
                    magic_bytes: &[],
                    mime_types: &["text/markdown"],
                },
                // SRT
                SniffRule {
                    format_id: FormatId::SRT,
                    extensions: &[".srt"],
                    magic_bytes: &[],
                    mime_types: &["application/x-subrip"],
                },
                // VTT
                SniffRule {
                    format_id: FormatId::VTT,
                    extensions: &[".vtt"],
                    magic_bytes: &[(b"WEBVTT", 0)],
                    mime_types: &["text/vtt"],
                },
                // FB2
                SniffRule {
                    format_id: FormatId::FB2,
                    extensions: &[".fb2"],
                    magic_bytes: &[],
                    mime_types: &["application/x-fictionbook+xml"],
                },
            ]
        }
    }

    /// Detecta formato por caminho e cabeçalho do arquivo
    pub fn sniff(&self, path: &Path, head: &[u8]) -> (FormatId, Confidence) {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let mut best = (FormatId::TXT, Confidence::none());

        for rule in &self.rules {
            let mut confidence = Confidence::none();

            // Match por extensão
            if rule.extensions.iter().any(|e| name.ends_with(e)) {
                confidence = Confidence::high();
            }

            // Match por magic bytes
            for (magic, offset) in rule.magic_bytes {
                if head.len() >= offset + magic.len() && &head[offset..offset + magic.len()] == *magic {
                    confidence = Confidence::certain();
                    break;
                }
            }

            // Match por MIME (se disponível via mime_guess)
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            if rule.mime_types.iter().any(|mt| mime.to_string() == *mt) {
                confidence = Confidence::certain();
            }

            if confidence > best.1 {
                best = (rule.format_id, confidence);
            }
        }

        best
    }

    /// Retorna todos os formatos conhecidos
    pub fn known_formats(&self) -> Vec<FormatId> {
        self.rules.iter().map(|r| r.format_id).collect()
    }
}

impl Default for FormatSniffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn sniff_pdf() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"%PDF-1.7\n").unwrap();
        let path = f.path().to_path_buf();

        let pdf_path = path.with_extension("pdf");
        std::fs::rename(&path, &pdf_path).unwrap();

        let sniffer = FormatSniffer::new();
        let (fmt, conf) = sniffer.sniff(&pdf_path, b"%PDF-1.7");

        assert_eq!(fmt, FormatId::PDF);
        assert_eq!(conf.value(), 100);

        std::fs::remove_file(pdf_path).ok();
    }

    #[test]
    fn sniff_epub() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"PK\x03\x04").unwrap();
        let path = f.path().to_path_buf();

        let epub_path = path.with_extension("epub");
        std::fs::rename(&path, &epub_path).unwrap();

        let sniffer = FormatSniffer::new();
        let (fmt, conf) = sniffer.sniff(&epub_path, b"PK\x03\x04");

        assert_eq!(fmt, FormatId::EPUB);
        assert_eq!(conf.value(), 100);

        std::fs::remove_file(epub_path).ok();
    }

    #[test]
    fn sniff_txt_by_extension() {
        let sniffer = FormatSniffer::new();
        let (fmt, conf) = sniffer.sniff(Path::new("test.txt"), b"plain text");

        assert_eq!(fmt, FormatId::TXT);
        assert_eq!(conf.value(), 75); // high
    }
}