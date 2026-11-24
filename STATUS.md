# Polyglottus — Status do Projeto (atualizado autonomamente)

## Resumo

O projeto está em **Fase 0 concluída / Fase 1 em andamento**. O núcleo Rust (domain, app, adaptadores de formato, provedores, storage) está implementado. A UI front-end (Svelte) foi construída. O pipeline de exportação end-to-end foi conectado.

## O que foi feito (commits autônomos)

### Commit 1: `63ad5c8` — Pipeline de exportação conectado
- `pgl-app`: adicionado `all_with_refs()` ao trait `TranslationLookup`; implementado em `JobTranslationLookup`; `ExportBook` carrega unidades do SQLite antes do export
- `pgl-cli`: `CliTranslationLookup` lê unidades do banco de fato; comando `export` passa `project.source_file` ao writer
- `tauri`: `export_book` carrega unidades, passa `source_path` correto, importa `JobTranslationLookup` do `pgl-app`
- `pgl-fmt-epub`: `translate_html_content` implementado (substitui texto em elementos de bloco XHTML via `source_ref.selector`)

### Commit 2: `b53cb58` — HTML reader/writer habilitado
- `pgl-fmt-text`: `HtmlTextReader`/`HtmlTextWriter` descomentados e registrados no `FormatRegistry`
- CLI: HTML reader/writer registrados
- Tauri: já tinha HTML registrado

### Commit 3: `24d380c` — UI front-end (Svelte + TypeScript + Tailwind)
- `Header`, `Sidebar`, `Toast`, `ProjectList`, `ProjectDetail`, `Settings`, `Import` components
- `App.svelte` com roteamento entre list/detail/settings/import
- Integração com comandos Tauri (import_book, list_projects, start_translation, etc.)

### Commit 4: `0e2fff4` — DOCX XML translation real
- `pgl-fmt-ooxml`: `translate_docx_xml` substitui texto em elementos `w:t` usando `all_with_refs` (antes era stub que retornava XML original)

### Commit 5: `9c6c1a0` — Testes de integração
- `pgl-testkit/tests/integration.rs`: pipeline completo import→translate→export com FakeProvider
- Testes de modos FakeProvider (ReverseWords, PlaceholderError)
- Teste de StyleGuide prompt para pt-BR

### Commit 6: `b23296d` (pré-existente) — Fase 0 completa
- Core pipeline funcional via CLI

## O que falta para Fase 1 (MVP)

### Crítico (bloqueia validação)
1. **Ambiente de build Windows**: falta `dlltool.exe` (MinGW-w64 ou Visual Studio Build Tools). `cargo check` falha em `windows-sys`/`getrandom`.
   - Solução: instalar Visual Studio Build Tools ou MinGW-w64 com `dlltool` no PATH
   - Depois rodar: `cargo check --workspace`

2. **html5ever 0.26 API**: `html_text.rs` existe mas precisa de adaptação para a API 0.26 (mudança em `parse_document` signature). Está marcado com TODO.

3. **ODT/ODP/ODS**: são placeholders (não lêem nem escrevem conteúdo real).

4. **PPTX/XLSX writers**: verificar se `translate_ooxml_xml` está implementado ou é stub.

### Importante (para uso real)
5. **test-corpus**: arquivos de teste criados para TXT, MD, SRT, VTT, HTML, EPUB (simples)

6. **UI front-end**: componentes criados mas precisam de validação com o build funcionando

7. **pgl-engine**: placeholder (apenas mantém processo vivo, sem JSON-RPC)

## Roadmap restante

| Fase | Status | Principais itens |
|------|--------|------------------|
| 0 Fundação | ✅ Concluída | Repo, CI, ADRs, domain, app, storage, providers, CLI, pipeline funcional |
| 1 MVP | 🚧 Em andamento | EPUB/HTML/TXT/MD/SRT/VTT + OpenAI-compat/Ollama + UI + CLI |
| 2 Escritório | ⬜ | DOCX/PPTX/XLSX/ODF/FB2/PO/XLIFF/TMX + revisão lado a lado + TM + OCR |
| 3 PDF | ⬜ | PDF reflow/overlay + Pandoc + motor embutido |
| 4 Avançado | ⬜ | PDF escaneado, re-diagramação, CBZ, inpainting |

## Como continuar quando voltar

1. Instalar build tools Windows (`dlltool.exe`)
2. Rodar `cargo check --workspace` para validar compilação
3. Rodar `./test-files/test_e2e_pipeline.sh` para validar E2E com FakeProvider
4. Adaptrar `html_text.rs` para html5ever 0.26
5. Implementar ODT/ODP/ODS readers/writers reais
6. Validar UI front-end com o build funcionando

## Comandos úteis

```bash
# Build e check
cargo check --workspace
cargo build --release -p pgl-cli

# Testes
cargo nextest run --workspace

# E2E (precisa do CLI compilado)
./test-files/test_e2e_pipeline.sh

# UI dev
cd apps/desktop && cargo tauri dev
```
