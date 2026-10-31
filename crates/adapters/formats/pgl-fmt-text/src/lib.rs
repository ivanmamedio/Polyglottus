//! Adaptador para formatos de texto simples

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod txt;
pub mod md;
pub mod srt;
pub mod vtt;
pub mod html_text;
pub mod po;
pub mod xliff;
pub mod tmx;
pub mod json_i18n;

use std::path::Path;
use std::sync::Arc;
use pgl_app::{FormatReader, FormatWriter, FormatId, Confidence, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use pgl_fmt_core::sniff_zip_format;

/// Leitor TXT
pub struct TxtReader;

impl TxtReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for TxtReader {
    fn id(&self) -> FormatId {
        FormatId::TXT
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".txt") || name.ends_with(".text");
        let text_match = head.iter().all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace() || b == b'\n' || b == b'\r');

        match (ext_match, text_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        txt::open_txt(path).await
    }
}

/// Writer TXT
pub struct TxtWriter;

impl TxtWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for TxtWriter {
    fn id(&self) -> FormatId {
        FormatId::TXT
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        txt::write_txt(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::TXT]
    }
}

/// Leitor Markdown
pub struct MdReader;

impl MdReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for MdReader {
    fn id(&self) -> FormatId {
        FormatId::MD
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".md") || name.ends_with(".markdown") || name.ends_with(".mdown") || name.ends_with(".mkd");
        let md_match = head.windows(2).any(|w| w == b"# ") || head.windows(3).any(|w| w == b"## ") || head.windows(2).any(|w| w == b"- ") || head.windows(2).any(|w| w == b"* ");

        match (ext_match, md_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        md::open_md(path).await
    }
}

/// Writer Markdown
pub struct MdWriter;

impl MdWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for MdWriter {
    fn id(&self) -> FormatId {
        FormatId::MD
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        md::write_md(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::MD]
    }
}

/// Leitor SRT
pub struct SrtReader;

impl SrtReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for SrtReader {
    fn id(&self) -> FormatId {
        FormatId::SRT
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".srt");
        let srt_match = head.windows(3).any(|w| w == b"1\n") || head.windows(4).any(|w| w == b"1\r\n");

        match (ext_match, srt_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        srt::open_srt(path).await
    }
}

/// Writer SRT
pub struct SrtWriter;

impl SrtWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for SrtWriter {
    fn id(&self) -> FormatId {
        FormatId::SRT
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        srt::write_srt(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::SRT]
    }
}

/// Leitor VTT
pub struct VttReader;

impl VttReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for VttReader {
    fn id(&self) -> FormatId {
        FormatId::VTT
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".vtt");
        let vtt_match = head.starts_with(b"WEBVTT");

        match (ext_match, vtt_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        vtt::open_vtt(path).await
    }
}

/// Writer VTT
pub struct VttWriter;

impl VttWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for VttWriter {
    fn id(&self) -> FormatId {
        FormatId::VTT
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        vtt::write_vtt(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::VTT]
    }
}

/// Leitor HTML (texto)
pub struct HtmlTextReader;

impl HtmlTextReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for HtmlTextReader {
    fn id(&self) -> FormatId {
        FormatId::HTML
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".html") || name.ends_with(".htm") || name.ends_with(".xhtml");
        let html_match = head.windows(7).any(|w| w == b"<html") || head.windows(9).any(|w| w == b"<!DOCTYPE");

        match (ext_match, html_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        html_text::open_html(path).await
    }
}

/// Writer HTML
pub struct HtmlTextWriter;

impl HtmlTextWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for HtmlTextWriter {
    fn id(&self) -> FormatId {
        FormatId::HTML
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        html_text::write_html(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::HTML]
    }
}

/// Leitor PO (gettext)
pub struct PoReader;

impl PoReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for PoReader {
    fn id(&self) -> FormatId {
        FormatId::PO
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".po") || name.ends_with(".pot");
        let po_match = head.windows(7).any(|w| w == b"msgid \"") || head.windows(8).any(|w| w == b"msgstr \"");

        match (ext_match, po_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        po::open_po(path).await
    }
}

/// Writer PO
pub struct PoWriter;

impl PoWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for PoWriter {
    fn id(&self) -> FormatId {
        FormatId::PO
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        po::write_po(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        false
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::PO]
    }
}

/// Leitor XLIFF
pub struct XliffReader;

impl XliffReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for XliffReader {
    fn id(&self) -> FormatId {
        FormatId::XLIFF
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".xliff") || name.ends_with(".xlf");
        let xliff_match = head.windows(14).any(|w| w == b"<xliff version") || head.windows(20).any(|w| w == b"<xliff xmlns=\"urn:oasis:names:tc:xliff:document:");

        match (ext_match, xliff_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        xliff::open_xliff(path).await
    }
}

/// Writer XLIFF
pub struct XliffWriter;

impl XliffWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for XliffWriter {
    fn id(&self) -> FormatId {
        FormatId::XLIFF
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        xliff::write_xliff(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true // XLIFF é nativamente bilíngue
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::XLIFF]
    }
}

/// Leitor TMX
pub struct TmxReader;

impl TmxReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for TmxReader {
    fn id(&self) -> FormatId {
        FormatId::TMX
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".tmx");
        let tmx_match = head.windows(13).any(|w| w == b"<tmx version=");

        match (ext_match, tmx_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        tmx::open_tmx(path).await
    }
}

/// Writer TMX
pub struct TmxWriter;

impl TmxWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for TmxWriter {
    fn id(&self) -> FormatId {
        FormatId::TMX
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        tmx::write_tmx(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true // TMX é nativamente bilíngue
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::TMX]
    }
}

/// Leitor JSON i18n
pub struct JsonI18nReader;

impl JsonI18nReader {
    pub fn new() -> Self {
        Self
    }
}

impl FormatReader for JsonI18nReader {
    fn id(&self) -> FormatId {
        FormatId::JSON_I18N
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        let ext_match = name.ends_with(".json");
        let json_match = head.starts_with(b"{") && (head.windows(8).any(|w| w == b"\"locale\"") || head.windows(9).any(|w| w == b"\"messages\""));

        match (ext_match, json_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::medium(),
            (false, true) => Confidence::low(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn pgl_app::SourceDocument>, FormatError> {
        json_i18n::open_json_i18n(path).await
    }
}

/// Writer JSON i18n
pub struct JsonI18nWriter;

impl JsonI18nWriter {
    pub fn new() -> Self {
        Self
    }
}

impl FormatWriter for JsonI18nWriter {
    fn id(&self) -> FormatId {
        FormatId::JSON_I18N
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        json_i18n::write_json_i18n(source, translations, options, out).await
    }

    fn supports_bilingual(&self) -> bool {
        true
    }

    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::JSON_I18N]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn readers_creation() {
        let _ = TxtReader::new();
        let _ = MdReader::new();
        let _ = SrtReader::new();
        let _ = VttReader::new();
        let _ = HtmlTextReader::new();
        let _ = PoReader::new();
        let _ = XliffReader::new();
        let _ = TmxReader::new();
        let _ = JsonI18nReader::new();
    }

    #[test]
    fn writers_creation() {
        let _ = TxtWriter::new();
        let _ = MdWriter::new();
        let _ = SrtWriter::new();
        let _ = VttWriter::new();
        let _ = HtmlTextWriter::new();
        let _ = PoWriter::new();
        let _ = XliffWriter::new();
        let _ = TmxWriter::new();
        let _ = JsonI18nWriter::new();
    }
}