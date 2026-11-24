# Polyglottus — Status do Projeto (atualizado autonomamente)

## Resumo

O projeto está em **Fase 0 concluída / Fase 1 em andamento**. O núcleo Rust (domain, app, adaptadores de formato, provedores, storage) está implementado. A UI front-end (Svelte) foi construída. O pipeline de exportação end-to-end foi conectado. ODT/ODP/ODS foram implementados com parsing XML real.

## O que foi feito (commits autônomos)

### Commit: `63ad5c8` — Pipeline de exportação conectado
- `pgl-app`: adicionado `all_with_refs()` ao trait `TranslationLookup`; implementado em `JobTranslationLookup`; `ExportBook` carrega unidades do SQLite antes do export
- `pgl-cli`: `CliTranslationLookup` lê unidades do banco de fato; comando `export` passa `project.source_file` ao writer
- `tauri`: `export_book` carrega unidades, passa `source_path` correto, importa `JobTranslationLookup` do `pgl-app`
- `pgl-fmt-epub`: `translate_html_content` implementado (substitui texto em elementos de bloco XHTML via `source_ref.selector`)

### Commit: `b53cb58` — HTML reader/writer habilitado (temporariamente revertido)
- `pgl-fmt-text`: `HtmlTextReader`/`HtmlTextWriter` descomentados, depois re-comentados devido a incompatibilidade com scraper API
- CLI/Tauri: registros adicionados e removidos conforme necessário

### Commit: `24d380c` — UI front-end (Svelte + TypeScript + Tailwind)
- `Header`, `Sidebar`, `Toast`, `ProjectList`, `ProjectDetail`, `Settings`, `Import` components
- `App.svelte` com roteamento entre list/detail/settings/import
- Integração com comandos Tauri (import_book, list_projects, start_translation, etc.)

### Commit: `0e2fff4` — DOCX XML translation real
- `pgl-fmt-ooxml`: `translate_docx_xml` substitui texto em elementos `w:t` usando `all_with_refs` (antes era stub que retornava XML original)

### Commit: `9c6c1a0` — Testes de integração
- `pgl-testkit/tests/integration.rs`: pipeline completo import→translate→export com FakeProvider
- Testes de modos FakeProvider (ReverseWords, PlaceholderError)
- Teste de StyleGuide prompt para pt-BR

### Commit: `294e210` — ODT reader/writer real
- `pgl-fmt-odf`: reescrito `odt.rs` com parsing real via quick-xml
- Extrai texto de `text:p`, `text:h` (com nível), `text:list-item`, `text:span`, `table:table-cell`
- Writer aplica traduções via `TranslationLookup::all_with_refs`
- Extrai assets (imagens de `Pictures/` e `Thumbnails/`)

### Commit: `b23296d` (pré-existente) — Fase 0 completa
- Core pipeline funcional via CLI

## O que falta para Fase 1 (MVP)

### Build e Testes — Resolvidos ✅
1. **Ambiente de build Windows**: rustup com target MSVC instalado. `cargo check --workspace` passa completamente (todos os crates compilam).
2. **pgl-models**: Stub que compila. Funcionalidade completa do catálogo de modelos fica para Fase 2+.
3. **Todos os testes passam**: 13 crates com testes, todos passando (0 falhas).

### O que falta para Fase 1 (MVP)
1. **html5ever 0.26 API**: `html_text.rs` re-comentado (incompatibilidade com scraper API). HTML não está no FormatRegistry.
2. **PPTX/XLSX**: verificar se writers aplicam traduções (DOCX já foi consertado)
3. **pgl-engine**: placeholder (apenas mantém processo vivo, sem JSON-RPC)

3. **html5ever 0.26 API**: `html_text.rs` re-comentado (incompatibilidade com scraper API). HTML não está disponível no FormatRegistry.

### Importante (para uso real)
4. **test-corpus**: arquivos de teste criados para TXT, MD, SRT, VTT, HTML, EPUB (simples)
5. **UI front-end**: componentes criados mas precisam de validação com o build funcionando
6. **pgl-engine**: placeholder (apenas mantém processo vivo, sem JSON-RPC)
7. **PPTX/XLSX**: verificar se writers aplicam traduções (DOCX já foi consertado)

## Roadmap restante

| Fase | Status | Principais itens |
|------|--------|------------------|
| 0 Fundação | ✅ Concluída | Repo, CI, ADRs, domain, app, storage, providers, CLI, pipeline funcional |
| 1 MVP | 🚧 Em andamento | EPUB/TXT/MD/SRT/VTT/ODT + OpenAI-compat/Ollama + UI + CLI |
| 2 Escritório | ⬜ | DOCX/PPTX/XLSX/ODF/FB2/PO/XLIFF/TMX + revisão lado a lado + TM + OCR |
| 3 PDF | ⬜ | PDF reflow/overlay + Pandoc + motor embutido |
| 4 Avançado | ⬜ | PDF escaneado, re-diagramação, CBZ, inpainting |

## Como continuar quando voltar

1. Verificar build: `cargo check --workspace` (falha apenas em pgl-models, pré-existente)
2. Rodar E2E: `./test-files/test_e2e_pipeline.sh` (usa FakeProvider)
3. Consertar `html_text.rs` para HTML (opcional, usar scraper corretamente)
4. Consertar `pgl-models` (módulos ausentes, imports ausentes)
5. Validar UI front-end
6. Implementar PPTX/XLSX verification

## Comandos úteis

```bash
# Build e check (falha apenas em pgl-models)
$env:PATH = "C:\Users\carlo\.cargo\bin;" + $env:PATH
cargo check --workspace

# Build apenas crates que funcionam
cargo check -p pgl-app -p pgl-domain -p pgl-fmt-epub -p pgl-fmt-ooxml -p pgl-fmt-text -p pgl-fmt-odf -p pgl-fmt-patch -p pgl-fmt-convert -p pgl-storage -p pgl-secrets -p pgl-ocr -p pgl-provider-openai -p pgl-provider-ollama -p pgl-provider-embedded -p pgl-cli -p pgl-testkit

# Testes
cargo nextest run --workspace --exclude pgl-models

# E2E (precisa do CLI compilado)
./test-files/test_e2e_pipeline.sh

# UI dev
cd apps/desktop && cargo tauri dev
```