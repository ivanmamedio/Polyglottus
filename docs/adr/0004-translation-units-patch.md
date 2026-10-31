# ADR-0004: Unidades de Tradução + Patch no Lugar

**Status:** Aceito

## Contexto

Como representar documentos para tradução preservando fidelidade?

Opções:
1. **Modelo de documento completo** (estilo BabelDOC) - IR complexa, difícil de manter
2. **Unidades de tradução + Patch no lugar** - Extrai unidades, traduz, aplica no original
3. **Conversão para pivô + regeneração** - Perde fidelidade

## Decisão

**Unidades de tradução + Patch no lugar** como estratégia principal.

### Modelo de Dados

```rust
Unit {
    id: UnitId (hash estável: source_ref + texto original),
    source_ref: SourceRef (caminho interno + XPath/seletor),
    kind: UnitKind (Heading, Paragraph, ListItem, TableCell, Caption, AltText, Footnote, Subtitle, Metadata, CodeBlock),
    text: MarkedText (placeholders ⟦1⟧...⟦/1⟧ para marcação inline),
    section: SectionId,
    state: UnitState (Pending → Translated → Flagged/Reviewed → Failed),
    translation: Option<MarkedText>,
    qa_warnings: Vec<QaWarning>,
}
```

### Pipeline

1. **Importar** → Detecta formato → Extrai unidades (streaming, sem carregar tudo)
2. **Planejar** → Glossário, guia de estilo, estimativa
3. **Traduzir** → Lotes com contexto + memória de tradução
4. **QA** → Placeholders, idioma, tamanho, repetição, glossário, entidades
5. **Revisar** → Lado a lado, editar, retraduzir
6. **Exportar** → Patch no lugar (reescreve só o texto, mantém estrutura) OU regenera

### Patch no Lugar

Para formatos ZIP+XML (EPUB, DOCX, ODF, OOXML):
1. Abre ZIP original
2. Localiza nós via XPath/seletor
3. Substitui texto do nó pelo traduzido
4. Reescreve ZIP mantendo tudo o mais igual

### Vantagens

- Streaming natural (unidades no SQLite, não na RAM)
- Checkpoint por unidade = retomada automática
- Mesmo modelo serve EPUB, DOCX, legendas, PDF, HTML
- Fidelidade máxima (não toca no que não é texto traduzível)
- Testável com property-based testing (proteger → restaurar = identidade)

## Consequências

**Positivas:**
- Memória constante independente do tamanho do livro
- Retomada garantida após crash/fechar app
- Placeholders validados na volta (conta, ordem, aninhamento)
- QA granular por unidade

**Negativas:**
- Complexidade no mapeamento source_ref → nó XML
- Expansão/contração de texto problemática em PDF/tabelas
- Requer validação rigorosa de placeholders

## Exceções

- **PDF reflow**: Extrai texto → exporta como EPUB/DOCX (não patch)
- **PDF overlay**: Desenha tradução sobre caixas originais (aproximação)
- **Formatos cauda longa (MOBI, RTF, etc.)**: Converte para pivô via Pandoc