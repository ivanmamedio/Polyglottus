# Matriz de Formatos - Polyglottus

> **Versão:** 1.0  
> **Atualizado:** 30/10/2026  
> **Fonte:** [Polyglottus-Planning.md](../Polyglottus-Planning.md#84-matriz-de-formatos)

---

## Visão Geral

| Formato | Entrada | Saída | Camada | Fidelidade | Fase | Leitor | Escritor |
|---------|---------|-------|--------|------------|------|--------|----------|
| **EPUB** | ✔ | ✔ | A | ★★★★★ | 1 | `pgl-fmt-epub` | `pgl-fmt-epub` |
| **HTML/XHTML** | ✔ | ✔ | A | ★★★★★ | 1 | `pgl-fmt-text` | `pgl-fmt-text` |
| **TXT** | ✔ | ✔ | A | ★★★★★ | 1 | `pgl-fmt-text` | `pgl-fmt-text` |
| **Markdown** | ✔ | ✔ | A | ★★★★★ | 1 | `pgl-fmt-text` | `pgl-fmt-text` |
| **SRT** | ✔ | ✔ | A | ★★★★★ | 1 | `pgl-fmt-text` | `pgl-fmt-text` |
| **VTT** | ✔ | ✔ | A | ★★★★★ | 1 | `pgl-fmt-text` | `pgl-fmt-text` |
| **DOCX** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-ooxml` | `pgl-fmt-ooxml` |
| **PPTX** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-ooxml` | `pgl-fmt-ooxml` |
| **XLSX** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-ooxml` | `pgl-fmt-ooxml` |
| **ODT** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-odf` | `pgl-fmt-odf` |
| **ODP** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-odf` | `pgl-fmt-odf` |
| **ODS** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-odf` | `pgl-fmt-odf` |
| **FB2** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-patch` | `pgl-fmt-patch` |
| **PO/POT** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-text` | `pgl-fmt-text` |
| **XLIFF** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-text` | `pgl-fmt-text` |
| **TMX** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-text` | `pgl-fmt-text` |
| **JSON i18n** | ✔ | ✔ | A | ★★★★★ | 2 | `pgl-fmt-text` | `pgl-fmt-text` |
| **Imagens** | ✔ | Legenda | A+OCR | ★★★☆☆ | 2 | `pgl-ocr` | — |
| **PDF (texto)** | ✔ | EPUB/DOCX/HTML | B1 | ★★★☆☆ | 3 | `pgl-fmt-pdf` (reflow) | — |
| **PDF (texto)** | ✔ | PDF (overlay) | B2 | ★★★☆☆ | 3 | `pgl-fmt-pdf` | `pgl-fmt-pdf` |
| **PDF (escaneado)** | ✔ | EPUB/DOCX | B+OCR | ★★☆☆☆ | 4 | `pgl-fmt-pdf` (OCR) | — |
| **MOBI/AZW3** | ✔ | Via pivô | C | ★★★☆☆ | 3 | `pgl-fmt-convert` (Pandoc) | `pgl-fmt-convert` |
| **RTF/DOC** | ✔ | Via pivô | C | ★★★☆☆ | 3 | `pgl-fmt-convert` (Pandoc/LibreOffice) | `pgl-fmt-convert` |
| **LaTeX** | ✔ | Via pivô | C | ★★★☆☆ | 3 | `pgl-fmt-convert` (Pandoc) | `pgl-fmt-convert` |
| **DJVU** | ✔ | Via pivô | C | ★★☆☆☆ | 3 | `pgl-fmt-convert` (Pandoc) | — |
| **CBZ/CBR** | ✔ | CBZ | C+OCR | ★★☆☆☆ | 4 | `pgl-fmt-convert` + OCR | — |

---

## Legenda de Camadas

| Camada | Descrição |
|--------|-----------|
| **A** | Nativo em Rust (patch no lugar), alta fidelidade |
| **B** | PDF - estratégia própria (reflow → overlay → re-diagramação) |
| **C** | Cauda longa via conversores externos (Pandoc, Calibre, LibreOffice) |

## Legenda de Fidelidade

| Estrelas | Significado |
|----------|-------------|
| ★★★★★ | Estrutura, estilos, imagens, metadados, links preservados |
| ★★★★☆ | Maioria preservada, pequenas perdas |
| ★★★☆☆ | Estrutura básica preservada, layout perdido |
| ★★☆☆☆ | Apenas texto, estrutura perdida |
| ★☆☆☆☆ | Experimental, perda significativa |

---

## Detalhes por Formato

### EPUB (Camada A - Fase 1)
- **Formatos**: EPUB 2, EPUB 3
- **Engine**: `pgl-fmt-epub` (baseado em `pgl-fmt-patch` + `epub` crate)
- **Preserva**: Spine (ordem de leitura), NAV/NCX, metadados OPF, imagens, CSS, fontes
- **Patch**: Reescreve XHTML mantendo estrutura
- **Bilíngue**: Gera EPUB com original + tradução
- **Validação**: EPUBCheck no CI

### HTML/XHTML (Camada A - Fase 1)
- **Engine**: `pgl-fmt-text` (html5ever + scraper)
- **Preserva**: Estrutura semântica, CSS/JS não modificados, imagens
- **Seletores**: `p, h1-h6, li, td, th, caption, figcaption, blockquote, pre, code`
- **Ignora**: `script, style, noscript, template, head, meta, link, title`

### TXT/Markdown (Camada A - Fase 1)
- **TXT**: Parágrafos por linha vazia
- **Markdown**: `comrak` para AST → unidades por heading/parágrafo/item/código
- **Preserva**: Estrutura de headings, listas, blocos de código

### Legendas SRT/VTT (Camada A - Fase 1)
- **SRT**: Parse por blocos (índice, tempo, texto)
- **VTT**: Parse WebVTT (cues com identificador, timing, settings)
- **Preserva**: Timing exato, numeração, formatação básica (`<i>`, `<b>`)
- **Aviso**: Expansão de texto pode dessincronizar

### DOCX/PPTX/XLSX (Camada A - Fase 2)
- **Engine**: `pgl-fmt-ooxml` (baseado em `docx-rs` + `pgl-fmt-patch`)
- **DOCX**: `w:t` runs, estilos, tabelas, cabeçalhos/rodapés, comentários
- **PPTX**: `a:t` em slides, masters, layouts, notas
- **XLSX**: `sharedStrings.xml` + sheets, fórmulas não traduzidas
- **Patch**: Reescreve XML no ZIP mantendo formatação

### ODT/ODP/ODS (Camada A - Fase 2)
- **Engine**: `pgl-fmt-odf` (baseado em `odf` crate + `pgl-fmt-patch`)
- **Content.xml**: `text:p, text:h, text:span, text:list-item, table:table-cell`
- **Styles.xml**: Textos em estilos automáticos

### PDF (Camada B - Fases 3-4)

#### Modo 1: Reflow (Fase 3)
- Extrai texto em ordem de leitura (pdfium)
- Exporta como EPUB/DOCX/HTML traduzido
- **Não preserva layout original**
- Útil para: leitura em e-reader, conversão

#### Modo 2: Overlay (Fase 3)
- Detecta caixas de texto (pdfium)
- Desenha tradução sobre caixa original
- Ajusta tamanho da fonte para caber
- **Preserva layout visual** (aproximado)

#### Modo 3: Re-diagramação (Fase 4+)
- Análise de layout (colunas, tabelas, fórmulas) via ONNX
- Re-tipografa com `rustybuzz`/`cosmic-text`
- **Pesquisa de longo prazo** (estilo BabelDOC)

#### PDF Escaneado (Fase 4)
- Renderiza páginas → imagens
- OCR (`ocrs` / Tesseract) → texto
- Pipeline normal + legenda traduzida

### Cauda Longa (Camada C - Fase 3)
| Formato | Conversor | Pivô | Notas |
|---------|-----------|------|-------|
| MOBI/AZW3 | Pandoc/Calibre | EPUB | DRM não removido |
| RTF | Pandoc | DOCX | |
| DOC (legado) | LibreOffice/Pandoc | DOCX | |
| LaTeX | Pandoc | EPUB/DOCX | MathML preservado |
| DJVU | Pandoc | EPUB | OCR necessário |
| CBZ/CBR | Custom + OCR | CBZ | Quadrinhos |

---

## Formatos de Localização (Camada A - Fase 2)

| Formato | Uso | Bilíngue Nativo |
|---------|-----|-----------------|
| **PO/POT** | gettext (software) | Não (msgid/msgstr) |
| **XLIFF** | Localização profissional | Sim (`<source>`/`<target>`) |
| **TMX** | Memória de tradução | Sim (`<tuv>` por idioma) |
| **JSON i18n** | Web/apps (i18next, Chrome Ext) | Sim (chave `_translated`) |

---

## OCR (Camada A - Fase 2)

| Engine | Idiomas | Precisão | Dependência |
|--------|---------|----------|-------------|
| **ocrs** (padrão) | Latim apenas | Média | Rust puro (ONNX via RTen) |
| **Tesseract** (opcional) | 100+ | Alta | C++ (binding) |
| **ONNX custom** (futuro) | Específicos | Muito alta | `ort` |

**Fluxo:**
1. Extrai imagens do livro (deduplicadas por hash)
2. Heurística: vale a pena OCR? (evita fotos sem texto)
3. OCR → traduz com contexto (legenda + parágrafos vizinhos)
4. Insere **legenda traduzida abaixo da imagem** (editável)
5. Confiança baixa → sinaliza para revisão

---

## Adicionando um Novo Formato

1. Crie crate em `crates/adapters/formats/pgl-fmt-<nome>/`
2. Implemente `FormatReader` e/ou `FormatWriter` (traits em `pgl-app`)
3. Registre no `FormatRegistry` (um único lugar em `pgl-app`)
4. Adicione testes de snapshot com `insta` usando `test-corpus/`
5. Atualize esta matriz
6. Documente em `docs/formats/<nome>.md`

---

## Testes de Fidelidade

| Teste | Ferramenta | Formatos |
|-------|------------|----------|
| EPUB válido | EPUBCheck | EPUB |
| DOCX abre sem reparo | `docx-rs` validação | DOCX |
| OOXML structure | Verificação custom | DOCX, PPTX, XLSX |
| ODF válido | `odf` crate | ODT, ODP, ODS |
| Round-trip | Import → Export → Compare | Todos |
| Property-based | `proptest` (proteger → restaurar = identidade) | MarkedText |