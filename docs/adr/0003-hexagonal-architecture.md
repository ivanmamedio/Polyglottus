# ADR-0003: Arquitetura Hexagonal (Portas e Adaptadores)

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

O Polyglottus precisa:
- Testar regras de negócio (domínio) sem dependências externas (GPU, rede, arquivos, UI)
- Trocar implementações de formato, provedor, OCR, storage sem tocar no domínio
- Suportar múltiplas interfaces (CLI, Tauri UI, futura API)
- Isolar código `unsafe` (FFI) em adaptadores pequenos e revisados

## Decisão

**Arquitetura Hexagonal (Ports & Adapters)** com três camadas:

```
┌───────────────────────────── Adaptadores de Entrada ─────────────────────────────┐
│   UI Tauri (comandos + eventos)            CLI (pgl-cli)            (futuro: API) │
└───────────────────────────────────────┬──────────────────────────────────────────┘
                                         ▼
┌───────────────────────── Aplicação (pgl-app): casos de uso ──────────────────────┐
│ ImportarLivro · PlanejarTraducao · ExecutarJob · RevisarUnidade · Exportar · ... │
│ Define as PORTAS (traits) que o mundo externo deve implementar                    │
└───────────────────────────────────────┬──────────────────────────────────────────┘
                                         ▼
┌─────────────────────── Domínio (pgl-domain): sem I/O, sem async ─────────────────┐
│ Unit, MarkedText, Glossary, StyleGuide, Project, JobState, regras de QA,        │
│ regras de segmentação, proteção de marcação                                       │
└───────────────────────────────────────▲──────────────────────────────────────────┘
                                         │ (dependências apontam sempre para dentro)
┌──────────────────────────── Adaptadores de Saída (infra) ────────────────────────┐
│ Formatos (epub, docx, pdf, …) · Provedores (OpenAI-compat, Ollama, embutido)     │
│ OCR (ocrs, tesseract) · Storage (SQLite) · Segredos (keyring) · Modelos · Logs   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

## Regras Obrigatórias

1. **`pgl-domain`**: Zero dependências externas. `#![forbid(unsafe_code)]`. Sem `async`. Testável com `cargo test` puro.
2. **`pgl-app`**: Conhece apenas `pgl-domain` + **traits das portas**. Zero dependências de implementação concreta.
3. **Adaptadores**: Implementam as traits das portas. **Nunca** importados por domínio ou aplicação.
4. **Montagem (DI)**: Acontece **apenas** nos binários (`pgl-cli`, `apps/desktop/src-tauri`).
5. **CI valida grafo de dependências**: `cargo deny` + verificação customizada impedem violações.

## Portas Principais (Traits em `pgl-app`)

| Porta | Propósito | Implementações |
|-------|-----------|----------------|
| `FormatReader` | Extração streaming de unidades | `EpubReader`, `DocxReader`, `PdfReflowReader`, `TxtReader`, `ConvertReader` |
| `FormatWriter` | Patch no lugar / regeneração | `EpubWriter`, `DocxWriter`, `PdfOverlayWriter`, `TxtWriter`, `ConvertWriter` |
| `TranslationProvider` | Inferência (local/remoto) | `OpenAICompatProvider`, `OllamaProvider`, `EmbeddedProvider` |
| `OcrEngine` | OCR de imagens | `OcrsEngine`, `TesseractEngine` (feature gate) |
| `ProjectRepository` | Persistência | `SqliteRepository` |
| `SecretStore` | Cofre de chaves | `SystemSecretStore` (keyring) |
| `ModelCatalog` | Modelos, hardware, download | `ModelCatalogImpl` |
| `EventSink` | Progresso, logs | `TauriEventSink`, `CliEventSink`, `TestEventSink` |

## Consequências

- ✅ **Testabilidade total**: Domínio + App testados com `FakeProvider`, `MemorySecretStore`, `InMemoryRepository`
- ✅ **Extensibilidade**: Novo formato = 1 crate implementando `FormatReader`/`FormatWriter` + registro em 1 lugar
- ✅ **Troca de provedor**: Ollama ↔ OpenAI-compat ↔ Embedded sem tocar no pipeline
- ✅ **Isolamento `unsafe`**: FFI (`pdfium-render`, `llama-cpp-2`, `ocrs` ONNX) confinado a adaptadores
- ✅ **Múltiplas UIs**: CLI e Tauri usam mesmos casos de uso (`ImportBook`, `ExecuteJob`, etc.)
- ⚠️ Mais boilerplate inicial (traits, DI manual) — compensado por clareza e manutenibilidade
- ⚠️ `async_trait` necessário para traits assíncronas (limitação Rust atual)

## Verificação no CI

```yaml
# .github/workflows/ci.yml - job deny
- name: Run cargo-deny
  run: cargo deny check
# Verifica: licenças, duplicatas, fontes, grafo de dependências
```

Regra de deny: `pgl-domain` não pode depender de `sqlx`, `reqwest`, `tauri`, `tokio`, `async-trait`, `zip`, `quick-xml`, etc.

## Implementação Atual

- `crates/domain/pgl-domain` — domínio puro ✅
- `crates/app/pgl-app` — casos de uso + traits ✅
- `crates/adapters/*` — implementações concretas ✅
- `crates/bins/pgl-cli` — montagem CLI ✅
- `apps/desktop/src-tauri` — montagem Tauri ✅