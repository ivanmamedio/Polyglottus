# ADR-0007: Sem Python no Núcleo

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

Muitas ferramentas de processamento de documentos usam Python:
- `pandoc` (Haskell, mas wrapper Python comum)
- `ebook-convert` (Calibre, Python)
- `pdfplumber`, `PyMuPDF`, `pymupdf` (Python)
- `python-docx`, `openpyxl`, `odfpy` (Python)
- `Tesseract` via `pytesseract` (wrapper Python)

Problemas do Python no núcleo:
- **Empacotamento**: Sidecar Python ~350MB (conforme doc do `mdkit`)
- **Build offline Flathub**: `pip` requer rede; `flatpak-builder` exige fontes offline
- **Distribuição**: Interpretador + dependências em cada plataforma
- **Performance**: GIL, startup lento, memória alta
- **Segurança**: Superfície de ataque maior (execução de código)

## Decisão

**Zero Python no núcleo (crates Rust).** Python permitido apenas como:
1. **Ferramenta de desenvolvimento** (scripts de build, geração de código, testes)
2. **Conversor externo opcional** via `pgl-fmt-convert` (Pandoc, Calibre, LibreOffice) — chamado como subprocesso, não linkado

## Implementação Atual — Tudo em Rust

| Necessidade | Solução Rust | Crate |
|-------------|--------------|-------|
| EPUB parsing | `epub` crate (Rust) | `pgl-fmt-epub` |
| DOCX/OOXML | `docx-rs` + `quick-xml` + `zip` | `pgl-fmt-ooxml` via `PatchEngine` |
| ODF | `lo_odf` + `quick-xml` + `zip` | `pgl-fmt-odf` via `PatchEngine` |
| PDF texto/layout | `pdfium-render` (binding C++) + `lopdf` | `pgl-fmt-pdf` |
| HTML/XML | `quick-xml`, `html5ever`, `scraper` | `pgl-fmt-text`, `pgl-fmt-patch` |
| Markdown | `comrak` | `pgl-fmt-text` |
| Legendas | Parsing manual simples | `pgl-fmt-text` |
| Localização (PO/XLIFF/TMX) | `quick-xml` + serde | `pgl-fmt-text` |
| OCR | `ocrs` (Rust puro, ONNX via `RTen`) | `pgl-ocr` |
| ZIP handling | `zip` crate (Rust) | `pgl-fmt-core` |
| SQLite | `sqlx` + `rusqlite` (C embutido) | `pgl-storage` |
| Secrets | `keyring` (Rust, backend nativo por OS) | `pgl-secrets` |
| Hardware detection | `sysinfo` (Rust) | `pgl-models` |
| Download modelos | `reqwest` + `tokio` + `sha2` | `pgl-models` |

## Conversores Externos (Camada C — Opcional)

`pgl-fmt-convert` registra conversores como `FormatReader`/`FormatWriter` que chamam subprocessos:

```rust
pub enum ConverterType {
    Pandoc,      // pandoc --to=epub --from=docx ...
    Calibre,     // ebook-convert input.docx output.epub
    LibreOffice, // libreoffice --headless --convert-to epub ...
}
```

- **Não no instalador base** — pacote opcional (Flatpak extension / download no app)
- **Chamados via `tokio::process::Command`** — isolados, timeout, sandbox
- **Fidelidade variável** — selo de fidelidade avisa usuário

## Build Offline Flathub

```yaml
# packaging/flatpak/org.polyglottus.Polyglottus.yml
# flatpak-builder-tools gera:
# - cargo-sources.json (crates.io + git deps)
# - node-sources.json (npm deps)
# ZERO Python sources necessários
```

## Consequências

- ✅ **Instalador leve**: ~15-25MB (vs 350MB+ com Python sidecar)
- ✅ **Build Flathub offline**: Apenas Cargo + npm sources
- ✅ **Distribuição simples**: Binário único + assets, sem interpretador
- ✅ **Segurança**: Sem execução de código Python não confiável
- ✅ **Performance**: Rust nativo, sem GIL, startup instantâneo
- ⚠️ **Formatos de cauda longa**: Requerem conversores externos (Pandoc ~150MB) — aceitável como pacote opcional
- ⚠️ **PDF avançado**: `pdfium-render` é binding C++ — gerenciado como dependência nativa (não Python)

## Verificação

- `grep -r "python\|Python\|pip\|pyproject" crates/` → **zero resultados** nos crates do núcleo
- `Cargo.toml` workspace: nenhuma dependência Python
- CI `package-flatpak`: `flatpak-builder` usa apenas `cargo-sources.json` + `node-sources.json`
- `pgl-fmt-convert` isolado atrás de feature flags / pacote opcional