# ADR-0008: Pandoc como Pacote Opcional (Camada C — Cauda Longa)

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

Formatos "cauda longa" que não valem leitor próprio em Rust:
- **E-books**: MOBI, AZW3 (Kindle), EPUB antigo
- **Office legado**: RTF, DOC (Word 97-2003), WPD
- **Markup**: LaTeX, AsciiDoc, Org, RST avançado
- **Outros**: DJVU, CBZ/CBR (quadrinhos), FictionBook 2 (parcial)

Pandoc é o conversor universal:
- Suporta 40+ formatos de entrada/saída
- Haskell nativo, binário único (~150MB)
- Qualidade de conversão superior a alternativas
- Licença **GPL-2.0-or-later** (implicações de distribuição)

## Decisão

**Pandoc como pacote opcional**, não dependência do núcleo.

1. **Não no instalador base** — adiciona ~150MB
2. **Distribuído como**:
   - Flatpak: **Extension** (`org.polyglottus.Polyglottus.Pandoc`) — instalação opcional
   - Windows: **Download sob demanda** no app (Settings → Formatos → "Instalar conversores")
   - Linux (não-Flatpak): `pacman -S pandoc` / `apt install pandoc` — documentado
3. **Chamado via `pgl-fmt-convert`** — `FormatReader`/`FormatWriter` que invoca `pandoc` subprocesso
4. **Selo de fidelidade** avisa: "Convertido via Pandoc — possível perda de formatação complexa"

## Arquitetura

```
┌─────────────────────────────────────────────────────────────────┐
│                      Núcleo (Rust)                              │
│  ImportarLivro → FormatRegistry.find_reader()                   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ ConvertReader (pgl-fmt-convert)                         │   │
│  │   - Detecta formato via extensão/magic bytes            │   │
│  │   - Chama: pandoc -t epub -o /tmp/pivot.epub input.xxx  │   │
│  │   - Delega para EpubReader (Camada A)                   │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Fluxo de Conversão

```
Entrada (MOBI/AZW3/RTF/DOC/LaTeX/DJVU)
         │
         ▼
ConvertReader.sniff() → Confidence::high()
         │
         ▼
pandoc --to=epub --output=/tmp/pivot.epub input.mobi
         │
         ▼
EpubReader.open(/tmp/pivot.epub) → Unidades (Camada A)
         │
         ▼
Pipeline normal: Traduzir → QA → Revisão
         │
         ▼
Exportar: EpubWriter.write() → saida.epub
         │
         ▼ (opcional)
pandoc --to=mobi --output=saida.mobi saida.epub
```

## Licença GPL — Implicações

| Cenário | Obrigação |
|---------|-----------|
| Pandoc **não distribuído** com o app (download usuário) | **Nenhuma** — app apenas chama `pandoc` CLI se presente |
| Pandoc como **Flatpak Extension** | Extension é pacote separado; app não linka Pandoc |
| Pandoc **embutido no instalador Windows** | ❌ **NÃO FAZER** — forçaria GPL no app inteiro |

**Política:** App **nunca** binda Pandoc. Sempre subprocesso. Usuário instala Pandoc separadamente ou via extension opcional.

## Alternativas Consideradas

| Alternativa | Por que não |
|-------------|-------------|
| `calibre` (`ebook-convert`) | Python sidecar ~350MB; mais lento; GPL também |
| `libreoffice --headless` | ~500MB+; lento; GPL/LGPL complexo |
| Escrever leitores Rust para MOBI/AZW3/RTF/DOC | Esforço enorme; formatos legados/proprietários; baixo ROI |
| `pdfium` para tudo | PDFium não lê MOBI/AZW3/RTF/DOC nativamente |

## Consequências

- ✅ **Núcleo leve**: Zero dependência Pandoc no build/instalador base
- ✅ **Licença segura**: App MIT/Apache, Pandoc GPL isolado em subprocesso
- ✅ **Cobertura máxima**: 40+ formatos via 1 ferramenta
- ✅ **Atualização independente**: Pandoc atualiza sem rebuild do app
- ⚠️ **Experiência do usuário**: Precisa instalar Pandoc (mitigado: auto-detect + botão "Instalar" no Settings)
- ⚠️ **Fidelidade variável**: Conversão MOBI→EPUB→MOBI perde metadados Kindle — selo avisa
- ⚠️ **Sandbox Flatpak**: Extension precisa permissão `--filesystem=host` ou portal — testar cedo

## Verificação

- `pgl-fmt-convert/Cargo.toml`: `optional = true` features, sem `pandoc` em `dependencies` obrigatórias
- `FormatRegistry` registra `ConvertReader`/`ConvertWriter` apenas se feature `convert` ativa
- CI `package-flatpak`: Manifesto base **não** inclui Pandoc; extension separada
- `cargo deny check licenses`: Pandoc GPL não aparece no grafo de dependências do app