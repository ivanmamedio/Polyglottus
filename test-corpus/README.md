# Test Corpus

Este diretório contém arquivos de teste para validação de formatos.

## Estrutura

```
test-corpus/
├── epub/
│   ├── simple.epub          # EPUB básico
│   ├── with-images.epub     # Com imagens
│   ├── complex-styles.epub  # CSS complexo
│   ├── rtl.epub             # Árabe/Hebraico
│   ├── cjk.epub             # Chinês/Japonês/Coreano
│   └── tables.epub          # Tabelas
├── docx/
│   ├── simple.docx
│   ├── with-tables.docx
│   ├── with-images.docx
│   ├── headers-footers.docx
│   └── comments.docx
├── pdf/
│   ├── text-only.pdf        # PDF com texto extraível
│   ├── scanned.pdf          # PDF escaneado (para OCR)
│   ├── columns.pdf          # Duas colunas
│   └── tables.pdf           # Tabelas complexas
├── odt/
├── html/
├── txt/
├── md/
├── srt/
├── vtt/
├── po/
├── xliff/
├── tmx/
└── json-i18n/
```

## Fontes

- **Domínio público**: Project Gutenberg, Standard Ebooks
- **Criados para teste**: Arquivos mínimos cobrindo edge cases
- **Licenças**: Apenas domínio público ou CC0

## Como Adicionar

1. Coloque arquivo em `test-corpus/<formato>/`
2. Nomeie descrevendo o caso: `simple.epub`, `rtl-hebrew.epub`, `nested-tables.docx`
3. Adicione teste de snapshot em `crates/adapters/formats/pgl-fmt-<formato>/tests/`
4. Execute `cargo insta test --accept` para gerar/atualizar snapshots

## Diretrizes

- **Tamanho**: Mantenha arquivos pequenos (< 1 MB cada)
- **Variedade**: Cubra casos normais + edge cases
- **Licença**: Apenas domínio público/CC0
- **Nomes**: Descritivos, sem espaços, lowercase