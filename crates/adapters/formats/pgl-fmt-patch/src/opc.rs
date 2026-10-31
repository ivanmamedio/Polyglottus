//! Manipulação de pacotes OPC (Open Packaging Conventions)
//! Base para DOCX, PPTX, XLSX, EPUB, ODF

use zip::ZipArchive;
use zip::read::ZipFile;
use zip::write::FileOptions;
use zip::ZipWriter;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write, Seek, Cursor};
use std::path::Path;
use pgl_fmt_core::zip_safe::{open_zip_safe, create_zip_safe, add_entry_safe, extract_entry_bytes, list_entries_safe};
use pgl_app::FormatError;

/// Pacote OPC genérico
pub struct OpcPackage {
    /// Arquivos do pacote (path -> conteúdo)
    pub parts: HashMap<String, Vec<u8>>,
    /// Partes de conteúdo XML (para tradução)
    pub content_parts: HashMap<String, Vec<u8>>,
    /// Relacionamentos
    pub relationships: HashMap<String, Vec<Relationship>>,
    /// Content types
    pub content_types: ContentTypes,
}

/// Relacionamento OPC
#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub target_mode: Option<String>, // "External" para links externos
}

/// Content Types
#[derive(Debug, Clone, Default)]
pub struct ContentTypes {
    pub defaults: HashMap<String, String>,      // extensão -> content type
    pub overrides: HashMap<String, String>,     // path -> content type
}

impl OpcPackage {
    /// Abre pacote OPC de arquivo
    pub fn open(path: &Path) -> Result<Self, FormatError> {
        let mut archive = open_zip_safe(path)?;

        // Lê [Content_Types].xml
        let content_types = Self::read_content_types(&mut archive)?;

        // Lê relacionamentos
        let relationships = Self::read_relationships(&mut archive)?;

        // Separa partes de conteúdo (XML) das demais
        let mut parts = HashMap::new();
        let mut content_parts = HashMap::new();

        for entry_info in list_entries_safe(&archive) {
            if entry_info.is_dir { continue; }

            let content = extract_entry_bytes(&mut archive, entry_info.name)?;
            let is_xml = Self::is_xml_content(&entry_info.name, &content, &content_types);

            parts.insert(entry_info.name.clone(), content.clone());
            if is_xml {
                content_parts.insert(entry_info.name, content);
            }
        }

        Ok(Self {
            parts,
            content_parts,
            relationships,
            content_types,
        })
    }

    /// Verifica se é conteúdo XML traduzível
    fn is_xml_content(name: &str, content: &[u8], content_types: &ContentTypes) -> bool {
        // Por content type
        if let Some(ct) = content_types.overrides.get(name) {
            return ct.contains("xml");
        }
        if let Some(ext) = std::path::Path::new(name).extension().and_then(|s| s.to_str()) {
            if let Some(ct) = content_types.defaults.get(ext) {
                return ct.contains("xml");
            }
        }

        // Fallback: verifica assinatura XML
        content.starts_with(b"<?xml") || content.starts_with(b"<")
    }

    fn read_content_types(archive: &mut ZipArchive<File>) -> Result<ContentTypes, FormatError> {
        let mut ct = ContentTypes::default();

        // Procura [Content_Types].xml
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.name() == "[Content_Types].xml" {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;

                // Parse simplificado
                for line in content.lines() {
                    if line.contains("<Default") {
                        if let Some(ext) = extract_attr(line, "Extension") {
                            if let Some(ct_val) = extract_attr(line, "ContentType") {
                                ct.defaults.insert(ext, ct_val);
                            }
                        }
                    } else if line.contains("<Override") {
                        if let Some(path) = extract_attr(line, "PartName") {
                            if let Some(ct_val) = extract_attr(line, "ContentType") {
                                ct.overrides.insert(path.trim_start_matches('/').to_string(), ct_val);
                            }
                        }
                    }
                }
                break;
            }
        }

        Ok(ct)
    }

    fn read_relationships(archive: &mut ZipArchive<File>) -> Result<HashMap<String, Vec<Relationship>>, FormatError> {
        let mut rels = HashMap::new();

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.name().ends_with(".rels") {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;

                let mut file_rels = Vec::new();
                // Parse simplificado de Relationships
                for line in content.lines() {
                    if line.contains("<Relationship") {
                        if let (Some(id), Some(rtype), Some(target)) = (
                            extract_attr(line, "Id"),
                            extract_attr(line, "Type"),
                            extract_attr(line, "Target"),
                        ) {
                            let target_mode = extract_attr(line, "TargetMode");
                            file_rels.push(Relationship {
                                id,
                                rel_type: rtype,
                                target,
                                target_mode,
                            });
                        }
                    }
                }

                // Mapeia para o arquivo correspondente
                let base_path = entry.name().replace(".rels", "").replace("_rels/", "");
                rels.insert(base_path, file_rels);
            }
        }

        Ok(rels)
    }

    /// Atualiza nó de texto no XML
    pub fn update_text_node(&mut self, file_path: &str, xpath: &str, text_node_index: usize, new_text: &str) -> Result<(), FormatError> {
        let content = self.content_parts.get_mut(file_path)
            .ok_or_else(|| FormatError::WriteError(format!("File not found: {}", file_path)))?;

        // Parse HTML/XML e atualiza nó
        let mut doc = html5ever::parse_document(
            html5ever::rcdom::RcDom::default(),
            html5ever::ParseOpts::default(),
        ).from_utf8()
        .read_from(&mut content.as_slice())?;

        // Encontra nó por XPath simplificado e atualiza
        Self::update_node_text(&mut doc.document, xpath, text_node_index, new_text)?;

        // Serializa de volta
        let mut serialized = Vec::new();
        html5ever::serialize(&mut serialized, &doc.document, html5ever::SerializeOpts::default())?;
        *content = serialized;

        Ok(())
    }

    fn update_node_text(
        node: &html5ever::rcdom::Handle,
        xpath: &str,
        text_index: usize,
        new_text: &str,
    ) -> Result<bool, FormatError> {
        // Implementação simplificada - em produção usar XPath real
        // Por enquanto retorna erro para forçar implementação específica
        Err(FormatError::WriteError("Update node not implemented generically".into()))
    }

    /// Salva pacote
    pub fn save(&self, path: &Path) -> Result<(), FormatError> {
        let mut zip = create_zip_safe(path)?;

        for (name, content) in &self.parts {
            add_entry_safe(&mut zip, name, content)?;
        }

        zip.finish()?;
        Ok(())
    }

    /// Retorna iterador sobre partes de conteúdo
    pub fn content_parts(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.content_parts.iter()
    }
}

/// Extrai atributo de string XML simples
fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = line.find(&pattern)? + pattern.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn opc_package_roundtrip() {
        let mut f = NamedTempFile::new().unwrap();
        write_minimal_opc(&mut f).unwrap();

        let pkg = OpcPackage::open(f.path()).unwrap();
        assert!(!pkg.parts.is_empty());
        assert!(!pkg.content_types.defaults.is_empty());

        // Salva em outro arquivo
        let mut out = NamedTempFile::new().unwrap();
        pkg.save(out.path()).unwrap();

        // Reabre
        let pkg2 = OpcPackage::open(out.path()).unwrap();
        assert_eq!(pkg.parts.len(), pkg2.parts.len());
    }

    fn write_minimal_opc(f: &mut NamedTempFile) -> std::io::Result<()> {
        let mut zip = ZipWriter::new(f.as_file_mut());
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Override PartName="/test.xml" ContentType="application/xml"/>
</Types>"#)?;

        zip.start_file("_rels/.rels", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="test.xml"/>
</Relationships>"#)?;

        zip.start_file("test.xml", options)?;
        zip.write_all(b"<root>Hello</root>")?;

        zip.start_file("test.xml.rels", options)?;
        zip.write_all(br#"<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#)?;

        zip.finish()?;
        Ok(())
    }
}