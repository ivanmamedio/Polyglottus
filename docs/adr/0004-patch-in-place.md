# ADR-0004: Unidades de Tradução + Patch no Lugar

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

Traduzir documentos preservando estrutura, formatação e layout é o maior desafio. Duas abordagens principais:

1. **IR Completa (Intermediate Representation)**: Parseia documento inteiro para modelo próprio, traduz, regenera saída (estilo BabelDOC/PDFMathTranslate)
2. **Unidades de Tradução + Patch no Lugar**: Extrai trechos traduzíveis (unidades), traduz, aplica de volta ao arquivo original modificando apenas o texto

## Decisão

**Unidades de Tradução + Patch no Lugar** como estratégia primária para formatos baseados em ZIP+XML (EPUB, OOXML, ODF, HTML, FB2).

## Modelo de Dados: Unidade de Tradução

```rust
pub struct Unit {
    pub id: UnitId,              // BLAKE3 hash(source_ref + texto original) — estável
    pub source_ref: SourceRef,   // Aponta para nó original (arquivo interno + XPath/seletor)
    pub kind: UnitKind,          // Heading, Paragraph, ListItem, TableCell, Caption, AltText, Footnote...
    pub text: MarkedText,        // Texto com placeholders ⟦1⟧...⟦/1⟧ protegendo markup inline
    pub section: SectionId,      // Capítulo/seção (contexto e progresso)
    pub state: UnitState,        // Pending → Translated → Flagged → Reviewed (ou Failed)
}
```

## Motor de Patch Genérico (`pgl-fmt-patch`)

Um único motor `PatchEngine` atende família inteira de formatos ZIP+XML:

| Família | Formatos | Seletor de Texto | Contexto |
|---------|----------|------------------|----------|
| OOXML (OPC) | DOCX, PPTX, XLSX | `w:t`, `a:t`, `si>t` | `w:p`, `p:sp`, `sheetData` |
| ODF | ODT, ODP, ODS | `text:p`, `text:h`, `text:span` | `text:h`, `text:list-item`, `table:table-cell` |
| E-books | EPUB 2/3, FB2 | `p`, `h1-h6`, `li`, `td`, `figcaption` | `h1-h6`, `li`, `td` |
| HTML/XHTML | HTML, XHTML | Mesmo EPUB | Mesmo EPUB |
| Legendas | SRT, VTT | Texto entre timestamps | — |
| Localização | PO, XLIFF, TMX, JSON | `msgstr`, `target`, `tuv` | — |

**Fluxo:**
1. `FormatReader.open()` → `PatchEngine::from_file()` → escaneia package → produz `Vec<UnitLocation>`
2. `UnitLocation` mapeia `UnitId` → (arquivo, XPath, índice nó texto, texto original, kind, section)
3. Tradução produz `UnitId` → `MarkedText` traduzido
4. `FormatWriter.write()` → `PatchEngine.apply_and_save()` → atualiza nós de texto no XML → re-zipa **sem alterar nada mais**

## Proteção de Marcação Inline (`MarkedText`)

```rust
// Original: "Ele disse <b>não</b> e saiu."
// MarkedText.raw: "Ele disse ⟦1⟧não⟦/1⟧ e saiu."
// placeholders: { "1": "<b>", "/1": "</b>" }
```

**Validação na volta (obrigatória):**
- Contagem de placeholders idêntica
- Ordem preservada
- Aninhamento correto (fechamento em ordem inversa)
- Tags correspondentes (`<b>` ↔ `</b>`)

Falha → QA error → reprocessa com prompt alternativo → se persistir, `UnitState::Flagged` para revisão humana.

## Vantagens vs IR Completa

| Critério | IR Completa | Patch no Lugar (Escolhido) |
|----------|-------------|----------------------------|
| Fidelidade estrutura | Alta (reconstrói) | **Máxima (arquivo original intacto)** |
| Memória | Documento inteiro | **Streaming (unidade por unidade)** |
| Complexidade | Muito alta (modelo docs) | **Baixa (1 motor genérico)** |
| Imagens/assets | Requer re-embutir | **Preservados byte-a-byte** |
| Fontes/CSS/estilos | Perde ou reconstrói | **Intactos** |
| Sumário/links internos | Frágil | **Preservados** |
| Round-trip | Difícil validar | **Trivial (diff original vs saída)** |
| Testabilidade | Complexa | **Snapshot tests por arquivo** |

## Formatos Fora do Escopo (Camada C — Conversores)

PDF, MOBI, AZW3, RTF, DOC, LaTeX, DJVU, CBZ → **Pandoc/Calibre/LibreOffice** convertem para pivô (EPUB/DOCX) → Patch Engine → (opcional) convertem de volta.

## Consequências

- ✅ **Fidelidade máxima** para 90% dos formatos (EPUB, DOCX, ODT, HTML, etc.)
- ✅ **Streaming natural** — livro nunca carregado inteiro na RAM
- ✅ **Checkpoint por unidade** — retomada idempotente por construção
- ✅ **Testes snapshot** — `insta` valida saída byte-a-byte contra test-corpus
- ✅ **Extensibilidade** — novo formato ZIP+XML = 1 scanner (seletores) + registro
- ⚠️ **Expansão/contração de texto** entre idiomas — crítico em tabelas, PDF, legendas (mitigado: QA length ratio + aviso)
- ⚠️ **RTL/CJK/índico** — fontes via `fontdb` + tipografia `rustybuzz`/`cosmic-text` (Fase 2+)
- ⚠️ **PDF** — não tem "parágrafos", usa estratégia própria (ADR separado)

## Verificação

- `pgl-fmt-patch` implementa `PatchEngine` + `FormatScanner` trait
- Scanners: `DocxScanner`, `OdfScanner`, `EpubScanner`, `HtmlScanner`, `PptxScanner`, `XlsxScanner`
- `pgl-fmt-epub` (legacy, usa `epub` crate) — **será migrado para PatchEngine**
- Testes property-based: `proteger → restaurar` = identidade
- EPUBCheck passa em EPUBs de saída