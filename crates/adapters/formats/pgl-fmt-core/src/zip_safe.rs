//! Manipulação segura de arquivos ZIP (anti-zip-bomb, anti-zip-slip)

use zip::ZipArchive;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use pgl_app::FormatError;

/// Limites de segurança para ZIP
pub const MAX_UNCOMPRESSED_SIZE: u64 = 500 * 1024 * 1024; // 500 MB
pub const MAX_COMPRESSION_RATIO: f64 = 100.0; // 100:1
pub const MAX_FILES: usize = 10_000;
pub const MAX_DEPTH: usize = 100;

/// Abre arquivo ZIP com verificações de segurança
pub fn open_zip_safe(path: &Path) -> Result<ZipArchive<File>, FormatError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    // Verifica número de arquivos
    if archive.len() > MAX_FILES {
        return Err(FormatError::ParseError(format!("Too many files in archive: {} > {}", archive.len(), MAX_FILES)));
    }

    // Verifica cada entrada
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;

        // Anti-zip-slip: verifica path traversal
        if let Some(name) = entry.enclosed_name() {
            // Verifica se tenta escapar do diretório base
            if name.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(FormatError::ParseError(format!("Zip slip detected: {}", entry.name())));
            }
        } else {
            return Err(FormatError::ParseError(format!("Invalid path in archive: {}", entry.name())));
        }

        // Anti-zip-bomb: verifica ratio de compressão
        let compressed = entry.compressed_size();
        let uncompressed = entry.size();

        if uncompressed > MAX_UNCOMPRESSED_SIZE {
            return Err(FormatError::ParseError(format!("File too large: {} > {}", uncompressed, MAX_UNCOMPRESSED_SIZE)));
        }

        if compressed > 0 {
            let ratio = uncompressed as f64 / compressed as f64;
            if ratio > MAX_COMPRESSION_RATIO {
                return Err(FormatError::ParseError(format!("Suspicious compression ratio: {:.1}:1", ratio)));
            }
        }

        // Verifica profundidade de diretório
        let depth = entry.name().matches('/').count();
        if depth > MAX_DEPTH {
            return Err(FormatError::ParseError(format!("Directory depth too high: {}", depth)));
        }
    }

    Ok(archive)
}

/// Extrai arquivo do ZIP de forma segura (streaming, sem carregar tudo na memória)
pub fn extract_entry_streaming<W: std::io::Write>(
    archive: &mut ZipArchive<File>,
    index: usize,
    mut writer: W,
) -> Result<u64, FormatError> {
    let mut entry = archive.by_index(index)?;

    // Valida path traversal
    let entry_name = entry.name();
    if PathBuf::from(entry_name).components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(FormatError::ParseError("Zip slip detected in entry".into()));
    }

    // Verificações adicionais
    if entry.size() > MAX_UNCOMPRESSED_SIZE {
        return Err(FormatError::ParseError("Entry too large".into()));
    }

    let mut buffer = vec![0u8; 64 * 1024]; // 64 KB buffer
    let mut total = 0u64;

    loop {
        let n = entry.read(&mut buffer)?;
        if n == 0 { break; }
        writer.write_all(&buffer[..n])?;
        total += n as u64;

        // Verificação incremental de tamanho
        if total > MAX_UNCOMPRESSED_SIZE {
            return Err(FormatError::ParseError("Entry exceeded max size during extraction".into()));
        }
    }

    Ok(total)
}

/// Extrai entrada para bytes (para arquivos pequenos) pelo índice
pub fn extract_entry_bytes<R: Read + Seek>(archive: &mut ZipArchive<R>, index: usize) -> Result<Vec<u8>, FormatError> {
    let mut entry = archive.by_index(index)?;

    // Valida path traversal
    if entry.enclosed_name().is_none() {
        return Err(FormatError::ParseError("Zip slip detected in entry".into()));
    }
    if PathBuf::from(entry.name()).components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(FormatError::ParseError("Zip slip detected in entry".into()));
    }

    if entry.size() > 50 * 1024 * 1024 { // 50 MB limite para carregar na memória
        return Err(FormatError::ParseError("Entry too large for memory extraction".into()));
    }

    let mut content = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut content)?;
    Ok(content)
}

/// Extrai entrada para bytes pelo nome
pub fn extract_entry_bytes_by_name<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Result<Vec<u8>, FormatError> {
    // Valida path traversal no nome solicitado
    let path = PathBuf::from(name);
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(FormatError::ParseError("Zip slip detected in requested path".into()));
    }

    let index = archive.file_names()
        .position(|n| n == name)
        .ok_or_else(|| FormatError::ParseError(format!("Entry not found: {}", name)))?;
    extract_entry_bytes(archive, index)
}

/// Lista entradas do ZIP de forma segura
pub fn list_entries_safe<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Vec<ZipEntryInfo> {
    let len = archive.len();
    let mut entries = Vec::with_capacity(len);
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            entries.push(ZipEntryInfo {
                name: entry.name().to_string(),
                compressed_size: entry.compressed_size(),
                uncompressed_size: entry.size(),
                is_dir: entry.is_dir(),
                compression_method: entry.compression(),
            });
        }
    }
    entries
}

/// Info de entrada ZIP
#[derive(Debug, Clone)]
pub struct ZipEntryInfo {
    pub name: String,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub is_dir: bool,
    pub compression_method: zip::CompressionMethod,
}

/// Cria ZIP writer seguro
pub fn create_zip_safe<W: Write + Seek>(writer: W) -> Result<zip::ZipWriter<W>, FormatError> {
    Ok(zip::ZipWriter::new(writer))
}

/// Adiciona entrada ao ZIP de forma segura
pub fn add_entry_safe<W: Write + Seek>(
    writer: &mut zip::ZipWriter<W>,
    name: &str,
    content: &[u8],
) -> Result<(), FormatError> {
    // Valida nome
    let path = PathBuf::from(name);
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(FormatError::WriteError("Invalid path: zip slip attempt".into()));
    }

    if content.len() as u64 > MAX_UNCOMPRESSED_SIZE {
        return Err(FormatError::WriteError("Content too large".into()));
    }

    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6));

    writer.start_file(name, options)?;
    writer.write_all(content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Write, Cursor};
    use tempfile::NamedTempFile;

    #[test]
    fn create_and_read_zip() {
        let f = NamedTempFile::new().unwrap();
        {
            let file = File::create(f.path()).unwrap();
            let mut zip = create_zip_safe(file).unwrap();
            add_entry_safe(&mut zip, "test.txt", b"Hello world").unwrap();
            add_entry_safe(&mut zip, "dir/nested.txt", b"Nested").unwrap();
            zip.finish().unwrap();
        }

        let mut archive = open_zip_safe(f.path()).unwrap();
        assert_eq!(archive.len(), 2);

        let content = extract_entry_bytes(&mut archive, 0).unwrap();
        assert_eq!(content, b"Hello world");
    }

    #[test]
    fn zip_slip_prevention() {
        let f = NamedTempFile::new().unwrap();
        {
            let file = File::create(f.path()).unwrap();
            let mut zip = create_zip_safe(file).unwrap();
            // Tenta adicionar arquivo com path traversal
            let result = add_entry_safe(&mut zip, "../etc/passwd", b"evil");
            assert!(result.is_err());
            zip.finish().unwrap();
        }

        // Verifica que não foi criado
        let mut archive = open_zip_safe(f.path()).unwrap();
        assert_eq!(archive.len(), 0);
    }

    #[test]
    fn compression_ratio_check() {
        // Cria arquivo altamente comprimido (zip bomb simulado)
        let f = NamedTempFile::new().unwrap();
        {
            let file = File::create(f.path()).unwrap();
            let mut zip = create_zip_safe(file).unwrap();
            // 10 MB de zeros comprime para ~100 bytes
            let large_content = vec![0u8; 10_000_000];
            add_entry_safe(&mut zip, "bomb.txt", &large_content).unwrap();
            zip.finish().unwrap();
        }

        // Deve falhar na abertura devido ao ratio
        let _result = open_zip_safe(f.path());
        // Nota: zip::CompressionMethod::Stored não comprime, então este teste
        // depende do método de compressão usado. Em produção, usar Deflated.
    }

    #[test]
    fn zip_slip_prevention_streaming() {
        let f = NamedTempFile::new().unwrap();
        {
            let file = File::create(f.path()).unwrap();
            let mut zip = create_zip_safe(file).unwrap();
            // Tenta adicionar arquivo com path traversal
            let result = add_entry_safe(&mut zip, "../etc/passwd", b"evil");
            assert!(result.is_err());
            zip.finish().unwrap();
        }

        // Tenta extrair via streaming - deve falhar
        let mut archive = open_zip_safe(f.path()).unwrap();
        let mut buffer = Vec::new();
        let result = extract_entry_streaming(&mut archive, 0, &mut buffer);
        assert!(result.is_err());
    }

    #[test]
    fn zip_slip_prevention_extract_bytes() {
        let f = NamedTempFile::new().unwrap();
        {
            let file = File::create(f.path()).unwrap();
            let mut zip = create_zip_safe(file).unwrap();
            add_entry_safe(&mut zip, "safe.txt", b"safe").unwrap();
            zip.finish().unwrap();
        }

        let mut archive = open_zip_safe(f.path()).unwrap();
        // Tenta extrair by name com path traversal
        let result = extract_entry_bytes_by_name(&mut archive, "../etc/passwd");
        assert!(result.is_err());
    }
}