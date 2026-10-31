# ADR-0007: Sem Python no Núcleo

**Status:** Aceito

## Contexto

Muitos projetos de tradução usam Python (TBL, Tolmach, BabelDOC, PDFMathTranslate). Python traz dependências pesadas (~350 MB sidecar conforme `mdkit`).

## Decisão

**Python fica fora do núcleo.** Zero dependência Python no runtime.

### Estratégia

1. **Formato nativo em Rust** (EPUB, DOCX, ODF, HTML, legendas, PO/XLIFF/TMX)
2. **PDF via pdfium-render** (C++ binding, ~5 MB)
3. **OCR via ocrs** (Rust puro, ONNX via RTen)
4. **Cauda longa via conversores externos** (Pandoc, Calibre, LibreOffice) como **pacote opcional**
5. **Inferência via llama.cpp** (C++ binding) ou servidores HTTP

### Conversores Externos (Pacote Opcional)

- **Pandoc**: MOBI, AZW3, RTF, DOC, LaTeX, DJVU, etc. (~150 MB)
- **Calibre `ebook-convert`**: Formatos de e-book legados
- **LibreOffice headless**: DOC, XLS, PPT legados

Distribuídos como:
- **Flatpak**: Extensões separadas (a confirmar)
- **Windows**: Download opcional dentro do app ou instalador separado
- **Não no instalador base**

## Consequências

**Positivas:**
- Instalador base pequeno (< 20 MB)
- Build offline do Flathub viável (fontes Cargo + Node apenas)
- Sem conflitos de versão Python
- Binário único no Windows (sem sidecar)

**Negativas:**
- Formatos de cauda longa requerem conversor instalado
- Pandoc GPL requer cuidado na distribuição (processo separado, pacote opcional)
- Menos formatos "out of the box" sem pacote opcional