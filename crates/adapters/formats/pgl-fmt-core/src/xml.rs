//! Utilitários XML seguros para parsing de formatos baseados em XML

use quick_xml::events::{Event, BytesStart};
use quick_xml::Reader;
use quick_xml::Writer;
use std::io::{Cursor, BufReader, Read};
use pgl_app::FormatError;

/// Parser XML seguro (sem resolução de entidades externas, limites de profundidade)
pub struct SafeXmlParser {
    reader: Reader<BufReader<Box<dyn Read>>>,
    depth: usize,
    max_depth: usize,
    max_entity_expansion: usize,
    buf: Vec<u8>,
    buf2: Vec<u8>,
}

impl SafeXmlParser {
    pub fn new(input: impl Read + 'static) -> Result<Self, FormatError> {
        let reader: Reader<BufReader<Box<dyn Read>>> = Reader::from_reader(BufReader::new(Box::new(input)));
        Ok(Self {
            reader,
            depth: 0,
            max_depth: 1000,
            max_entity_expansion: 10_000,
            buf: Vec::new(),
            buf2: Vec::new(),
        })
    }

    pub fn with_limits(mut self, max_depth: usize, max_entity_expansion: usize) -> Self {
        self.max_depth = max_depth;
        self.max_entity_expansion = max_entity_expansion;
        self
    }

    pub fn next(&mut self) -> Result<Event<'static>, FormatError> {
        self.buf.clear();
        match self.reader.read_event_into(&mut self.buf) {
            Ok(Event::Start(e)) => {
                self.depth += 1;
                if self.depth > self.max_depth {
                    return Err(FormatError::ParseError("XML depth limit exceeded".into()));
                }
                Ok(Event::Start(e.into_owned()))
            }
            Ok(Event::End(_)) => {
                self.depth = self.depth.saturating_sub(1);
                self.buf2.clear();
                self.reader.read_event_into(&mut self.buf2)?;
                let name = String::from_utf8_lossy(&self.buf2).into_owned();
                Ok(Event::End(quick_xml::events::BytesEnd::new(name)))
            }
            Ok(e) => Ok(e.into_owned()),
            Err(e) => Err(FormatError::ParseError(e.to_string())),
        }
    }

    /// Extrai texto de um elemento (concatena todo conteúdo de texto)
    pub fn extract_text(&mut self) -> Result<String, FormatError> {
        let mut text = String::new();

        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Text(e)) => text.push_str(&e.unescape()?),
                Ok(Event::End(_)) => break,
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => return Err(FormatError::ParseError(e.to_string())),
            }
        }

        Ok(text)
    }

    /// Extrai atributo de um elemento de início
    pub fn get_attribute(start: &BytesStart, name: &str) -> Option<String> {
        start.attributes()
            .find_map(|attr| attr.ok())
            .filter(|attr| attr.key.as_ref() == name.as_bytes())
            .map(|attr| String::from_utf8_lossy(&attr.value).into_owned())
    }
}

/// Serializador XML
pub struct XmlSerializer {
    writer: Writer<Cursor<Vec<u8>>>,
}

impl XmlSerializer {
    pub fn new() -> Self {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None))).ok();
        Self { writer }
    }

    pub fn start_element(&mut self, name: &str, attrs: &[(&str, &str)]) -> Result<(), FormatError> {
        let mut elem = BytesStart::new(name);
        for (k, v) in attrs {
            elem.push_attribute((k.as_bytes(), v.as_bytes()));
        }
        self.writer.write_event(Event::Start(elem))?;
        Ok(())
    }

    pub fn end_element(&mut self, name: &str) -> Result<(), FormatError> {
        self.writer.write_event(Event::End(quick_xml::events::BytesEnd::new(name)))?;
        Ok(())
    }

    pub fn write_text(&mut self, text: &str) -> Result<(), FormatError> {
        self.writer.write_event(Event::Text(quick_xml::events::BytesText::new(text)))?;
        Ok(())
    }

    pub fn write_cdata(&mut self, text: &str) -> Result<(), FormatError> {
        self.writer.write_event(Event::CData(quick_xml::events::BytesCData::new(text)))?;
        Ok(())
    }

    pub fn finish(self) -> Result<String, FormatError> {
        let inner = self.writer.into_inner();
        Ok(String::from_utf8(inner.into_inner())?)
    }
}

impl Default for XmlSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper para encontrar elemento por XPath simples
pub fn find_element<'a>(reader: &mut SafeXmlParser, xpath: &str) -> Result<Option<String>, FormatError> {
    // Implementação simplificada - apenas tag name
    let target_tag = xpath.trim_start_matches("//").trim_start_matches('/');

    let mut buf = Vec::new();
    loop {
        match reader.reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == target_tag.as_bytes() => {
                return Ok(Some(reader.extract_text()?));
            }
            Ok(Event::Eof) => return Ok(None),
            Ok(_) => {}
            Err(e) => return Err(FormatError::ParseError(e.to_string())),
        }
        buf.clear();
    }
}

/// Escapa XML
pub fn escape_xml(text: &str) -> String {
    text.replace('&', "&")
        .replace('<', "<")
        .replace('>', ">")
        .replace('"', "\"")
        .replace('\'', "&apos;")
}

/// Desescapa XML
pub fn unescape_xml(text: &str) -> String {
    text.replace("&apos;", "\u{27}")
        .replace("\"", "\"")
        .replace(">", ">")
        .replace("<", "<")
        .replace("&", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_unescape() {
        let original = r#"<tag attr="value">Hello & "world"</tag>"#;
        let escaped = escape_xml(original);
        let unescaped = unescape_xml(&escaped);
        assert_eq!(original, unescaped);
    }

    #[test]
    fn xml_serializer() {
        let mut ser = XmlSerializer::new();
        ser.start_element("root", &[("attr", "value")]).unwrap();
        ser.write_text("Hello world").unwrap();
        ser.end_element("root").unwrap();

        let xml = ser.finish().unwrap();
        assert!(xml.contains("<root attr=\"value\">"));
        assert!(xml.contains("Hello world"));
        assert!(xml.contains("</root>"));
    }
}