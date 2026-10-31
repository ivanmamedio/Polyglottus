# Como Adicionar um Formato

> Este guia explica como adicionar suporte a um novo formato de arquivo no Polyglottus.

---

## Visão Geral

A arquitetura hexagonal torna a adição de formatos simples:

1. **Um crate = um formato** (ex: `pgl-fmt-mobi`)
2. **Implementa traits** `FormatReader` e/ou `FormatWriter` (definidos em `pgl-app`)
3. **Registra em um lugar** (`FormatRegistry` em `pgl-app`)
4. **Testes de snapshot** com `insta` + `test-corpus/`
5. **Atualiza documentação** (matriz, ADR se necessário)

---

## Passo a Passo

### 1. Crie o Crate

```bash
# No workspace root
mkdir -p crates/adapters/formats/pgl-fmt-meuformato/src
```

**`Cargo.toml`:**
```toml
[package]
name = "pgl-fmt-meuformato"
version = "0.1.0"
edition = "2021"
description = "Adaptador MeuFormato"
license = "MIT OR Apache-2.0"

[dependencies]
pgl-fmt-core = { path = "../pgl-fmt-core" }
pgl-app = { path = "../../../app/pgl-app" }
pgl-domain = { path = "../../../domain/pgl-domain" }
# Dependências específicas do formato
thiserror = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
tempfile = { workspace = true }
```

### 2. Implemente o Leitor (`FormatReader`)

```rust
// src/reader.rs
use pgl_app::{FormatReader, FormatId, Confidence, FormatError, SourceDocument, AssetRef, FidelityReport, DocumentMetadata};
use pgl_domain::{Unit, UnitId, UnitKind, SectionId, MarkedText, SourceRef, Language};
use async_trait::async_trait;
use std::path::Path;

pub struct MeuFormatoReader;

#[async_trait]
impl FormatReader for MeuFormatoReader {
    fn id(&self) -> FormatId {
        FormatId("meuformato")
    }

    fn sniff(&self, name: &str, head: &[u8]) -> Confidence {
        // Detecção por extensão + magic bytes
        let ext_match = name.ends_with(".meu");
        let magic_match = head.starts_with(b"MEU\x00");
        
        match (ext_match, magic_match) {
            (true, true) => Confidence::certain(),
            (true, false) => Confidence::high(),
            (false, true) => Confidence::medium(),
            (false, false) => Confidence::none(),
        }
    }

    async fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument>, FormatError> {
        // 1. Parse do arquivo
        // 2. Extrai unidades (streaming!)
        // 3. Extrai assets (imagens, etc.)
        // 4. Gera fidelity report
        // 5. Retorna SourceDocument
        
        let units = self.extract_units(path)?;
        let assets = self.extract_assets(path)?;
        let fidelity = self.fidelity_report();
        let metadata = self.extract_metadata(path)?;
        
        Ok(Box::new(pgl_fmt_core::ZipSourceDocument::new(
            archive, // ou archive vazio para arquivos não-ZIP
            units,
            assets,
            fidelity,
            metadata,
        )))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".meu", ".meuformato"]
    }
    
    fn mime_types(&self) -> &'static [&'static str] {
        &["application/x-meuformato"]
    }
}
```

### 3. Implemente o Escritor (`FormatWriter`)

```rust
// src/writer.rs
use pgl_app::{FormatWriter, FormatId, FormatError, ExportOptions, ExportReport, TranslationLookup, OutputFormat};
use async_trait::async_trait;
use std::path::Path;

pub struct MeuFormatoWriter;

#[async_trait]
impl FormatWriter for MeuFormatoWriter {
    fn id(&self) -> FormatId {
        FormatId("meuformato")
    }

    async fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,
        out: &Path,
    ) -> Result<ExportReport, FormatError> {
        // 1. Lê arquivo original
        // 2. Para cada unidade traduzida, aplica no original
        // 3. Se bilíngue, gera versão dual
        // 4. Escreve arquivo de saída
        
        Ok(ExportReport {
            output_path: out.display().to_string(),
            format: OutputFormat::from_str("meuformato").unwrap_or(OutputFormat::SameAsSource),
            units_processed: 0, // contar
            units_with_warnings: 0,
            units_failed: 0,
            output_size: std::fs::metadata(out)?.len(),
            export_time_ms: 0,
            warnings: vec![],
        })
    }

    fn supports_bilingual(&self) -> bool { true }
    
    fn output_formats(&self) -> &'static [OutputFormat] {
        &[OutputFormat::from_str("meuformato").unwrap_or(OutputFormat::SameAsSource)]
    }
}
```

### 4. Registre no Registry

Em `pgl-app/src/lib.rs` (módulo `registry`):

```rust
// No setup do app (binários)
let mut registry = FormatRegistry::new();
registry.register_reader(Arc::new(MeuFormatoReader));
registry.register_writer(Arc::new(MeuFormatoWriter));
```

### 5. Testes de Snapshot

```rust
// tests/integration.rs
use pgl_fmt_core::test_corpus_path;
use insta::assert_snapshot;

#[test]
fn read_meuformato() {
    let reader = MeuFormatoReader;
    let path = test_corpus_path("meuformato/simple.meu");
    let doc = block_on(reader.open(&path)).unwrap();
    
    let mut units = Vec::new();
    while let Some(unit) = doc.units().next() {
        units.push(unit.unwrap());
    }
    
    assert_snapshot!(units, @r###"
    [
        Unit { id: ..., kind: Paragraph, text: "Hello world", ... },
    ]
    "###);
}

#[test]
fn write_meuformato() {
    // Testa round-trip: ler -> traduzir (FakeProvider) -> escrever -> ler
}
```

**Adicione arquivo de teste em `test-corpus/meuformato/simple.meu`**

### 6. Atualize Documentação

- `docs/formats/matrix.md` - Adicione linha na tabela
- `docs/formats/meuformato.md` - Documentação detalhada
- `docs/adr/XXXX-meuformato.md` - ADR se for decisão arquitetural significativa

---

## Dicas Importantes

### Streaming (Obrigatório)

**NÃO carregue o arquivo inteiro na memória.** Use iteradores:

```rust
fn units(&mut self) -> Box<dyn Iterator<Item = Result<Unit, FormatError>> + '_> {
    Box::new(self.internal_iter()) // Lazy!
}
```

### Placeholders

Proteja marcação inline:

```rust
let marked = MarkedText::from_html("<b>Olá</b> mundo")?;
// marked.raw = "⟦1⟧Olá⟦/1⟧ mundo"
// marked.placeholders = {"1": "<b>", "/1": "</b>"}
```

### Fidelidade

Seja honesto no `FidelityReport`:

```rust
FidelityReport::new(4)
    .with_preserved("Estrutura de parágrafos")
    .with_preserved("Imagens")
    .with_not_preserved("Formatação avançada (tabelas complexas)")
    .with_warning("Tabelas aninhadas podem perder estrutura")
```

### Testes de Propriedade

```rust
proptest! {
    #[test]
    fn placeholder_roundtrip(html in html_with_inline_tags()) {
        let marked = MarkedText::from_html(&html).unwrap();
        let restored = marked.restore().unwrap();
        // Placeholders protegidos -> restaurados corretamente
        prop_assert_eq!(html, restored);
    }
}
```

---

## Checklist de PR

- [ ] Crate criado em `crates/adapters/formats/pgl-fmt-<nome>/`
- [ ] Implementa `FormatReader` (e `FormatWriter` se aplicável)
- [ ] Registrado no `FormatRegistry`
- [ ] Testes de snapshot passando (`cargo insta test --accept`)
- [ ] Testes de integração com `FakeProvider`
- [ ] Arquivo(s) em `test-corpus/`
- [ ] `docs/formats/matrix.md` atualizado
- [ ] `docs/formats/<nome>.md` criado
- [ ] `cargo deny check` passa (licenças)
- [ ] `cargo clippy` passa
- [ ] `cargo fmt --check` passa