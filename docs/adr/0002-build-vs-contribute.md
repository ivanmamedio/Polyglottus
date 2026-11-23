# ADR-0002: Construir do Zero vs. Contribuir com TBL (TranslateBooksWithLLMs)

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

O [TranslateBooksWithLLMs (TBL)](https://github.com/TranslateBooksWithLLMs/TranslateBooksWithLLMs) é o projeto open source mais próximo do Polyglottus:
- Traduz livros (EPUB, DOCX, TXT, SRT)
- Suporta Ollama, servidores OpenAI-compat, APIs
- Checkpoints, retomada, sem limite de tamanho
- App desktop, web UI, CLI
- Licença AGPL-3.0
- Núcleo em **Python**

O Polyglottus tem princípios inegociáveis que divergem:
1. **Rust-first** (TBL é Python)
2. **Leve, sem Python/Electron no núcleo** (TBL usa Python + Electron/tauri-python)
3. **Maior cobertura de formatos** (PDF, ODF, OOXML, localização, cauda longa via Pandoc)
4. **OCR de imagens do livro** com legenda traduzida
5. **Revisão manual integrada** com memória de tradução
6. **Gerenciador de modelos** com detecção de hardware
7. **Instalador de verdade** (Flatpak + MSI Windows)

## Decisão

**Construir do zero** (não forkar, não contribuir upstream como base)

## Justificativa

| Critério | TBL (Python) | Polyglottus (Rust) | Decisão |
|----------|--------------|-------------------|---------|
| Linguagem núcleo | Python | **Rust** | Incompatível |
| Empacotamento | PyInstaller (~350MB sidecar) | **Binário nativo ~10-20MB** | Diferencial |
| Arquitetura | Monolítica | **Hexagonal (ports/adapters)** | Diferencial |
| Testabilidade | Requer modelo/GPT | **FakeProvider determinístico** | Diferencial |
| Formatos | EPUB, DOCX, TXT, SRT | **EPUB, DOCX, PPTX, XLSX, ODF, PDF, FB2, PO, XLIFF, TMX, SRT, VTT, HTML, MD, TXT + cauda longa** | Diferencial |
| PDF | Não suportado | **Reflow + Overlay (planejado)** | Diferencial |
| OCR | Não | **ocrs + Tesseract opcional** | Diferencial |
| Revisão | Básica | **Lado a lado + TM + glossário** | Diferencial |
| Modelos | Manual | **Detecção hw + catálogo + download** | Diferencial |
| Licença | AGPL-3.0 | **MIT OR Apache-2.0** | Estratégia distinta |

**Spike realizado (2 semanas):** Tentativa de reaproveitar lógica de extração de EPUB/DOCX do TBL resultou em:
- Código Python não reutilizável diretamente (precisaria reescrever em Rust)
- Arquitetura monolítica difícil de extrair módulos isolados
- Dependências Python (pandoc, ebooklib, python-docx) conflitam com princípio "sem Python no núcleo"

**Conclusão:** O esforço de adaptar TBL > esforço de construir do zero com arquitetura correta desde o início.

## Consequências

- ✅ Arquitetura hexagonal limpa desde o dia 1
- ✅ Zero Python no núcleo — build offline Flathub trivial
- ✅ Testes rápidos sem GPU/modelo (FakeProvider)
- ✅ Extensibilidade: adicionar formato = 1 crate `pgl-fmt-xxx`
- ⚠️ Reinventar extração EPUB/DOCX (mitigado: `epub` crate, `docx-rs`, `PatchEngine` genérico)
- ⚠️ Menos "battle-tested" inicialmente (mitigado: test-corpus + CI rigoroso)

## Alternativas Consideradas

| Alternativa | Por que não |
|-------------|-------------|
| Fork TBL + reescrever núcleo em Rust | Quase reescrita total; histórico de commits polui |
| Contribuir upstream com Rust bindings | TBL não aceita Rust no núcleo (decisão de arquitetura deles) |
| Usar TBL como backend (sidecar Python) | Viola princípio "sem Python", instalação pesada, sandbox Flatpak complexo |

## Verificação

- Repositório criado do zero com arquitetura hexagonal
- `pgl-domain` puro (sem I/O, sem async) — testável isoladamente
- `pgl-testkit/FakeProvider` valida pipeline completo sem modelo
- CI roda em Windows + Linux sem Python