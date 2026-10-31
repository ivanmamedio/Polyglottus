# ADR-0008: Pandoc como Pacote Opcional

**Status:** Aceito

## Contexto

Formatos de cauda longa (MOBI, AZW3, RTF, DOC, LaTeX, DJVU, etc.) não justificam leitores nativos em Rust.

## Decisão

**Pandoc como pacote opcional** (não no instalador base).

### Fluxo

```
Arquivo MOBI/AZW3/RTF/DOC/LaTeX
        |
        v
   Pandoc (ebook-convert / --to epub/docx)
        |
        v
   Pivô EPUB/DOCX (formato nativo)
        |
        v
   Pipeline normal (unidades, patch, export)
        |
        v
   Saída: EPUB/DOCX/HTML (ou converte de volta via Pandoc)
```

### Distribuição

| Plataforma | Método |
|------------|--------|
| Flatpak | Extensão separada `org.polyglottus.Polyglottus.Pandoc` |
| Windows | Download opcional no app (menu Modelos/Pacotes) |
| Linux (não-Flatpak) | Dependência do sistema (`sudo apt install pandoc`) |

### Licença

Pandoc é **GPL-2.0-or-later**. Como é:
- Chamado como **processo separado** (não linked)
- Distribuído como **pacote opcional** (não bundlado no instalador base)
- Usuario opta por instalar

Isso mantém o núcleo do Polyglottus em MIT/Apache-2.0.

### Alternativas

- **Calibre `ebook-convert`**: GPL-3.0, mais pesado, mas melhor para alguns formatos
- **LibreOffice headless**: MPL-2.0, bom para DOC/XLS/PPT legados
- **Leitores nativos Rust**: Esforço enorme, baixa prioridade

## Consequências

**Positivas:**
- Núcleo leve e licença permissiva
- Cobertura massiva de formatos via Pandoc (30+)
- Usuario só baixa se precisar
- Atualizações de Pandoc independentes

**Negativas:**
- Dependência externa para cauda longa
- Conversão de volta pode perder fidelidade
- GPL requer separação clara de processo