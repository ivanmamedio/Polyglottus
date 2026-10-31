# ADR-0002: Build vs Contribute vs Fork

**Status:** Proposto

## Contexto

Existem projetos similares (TBL, Tolmach, epub-tr, etc.). A decisão de construir do zero, contribuir ou forkar afeta todo o roadmap.

## Decisão

**Construir do zero** com arquitetura hexagonal, Rust-first, e foco em:
- Instalador leve (sem Python, sem Electron)
- Maior cobertura de formatos nativa
- OCR integrado
- Revisão manual integrada
- Gerenciador de modelos com detecção de hardware

## Consequências

**Positivas:**
- Controle total da arquitetura
- Zero dependência Python (crítico para Flatpak)
- Código limpo e testável desde o início
- Alinhado com princípios declarados

**Negativas:**
- Mais trabalho inicial
- Reinventa algumas rodas (parsers EPUB, etc.)
- Tempo até MVP maior

## Spike Recomendado

2 semanas comparando:
1. Esforço para reaproveitar TBL (Python → Rust, arquitetura)
2. Esforço para construir parsers EPUB/DOCX nativos
3. Comparação de cobertura de formatos

## Critérios de Decisão

Se o spike mostrar que reaproveitar TBL economiza >50% do trabalho de formato E mantém arquitetura limpa → reconsiderar.
Caso contrário → confirmar build from scratch.