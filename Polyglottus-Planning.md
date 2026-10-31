# Polyglottus — Documento do Projeto (v3)

> Tradutor de livros e documentos, gratuito, open source e *local-first*.
> Plataformas: **Windows** e **Linux (Flatpak)**.
> Atualizado em 30/10/2026.
>
> Convenções deste documento:
> - **[verificar]** = hipótese ou informação que precisa de checagem/protótipo antes de virar decisão.
> - **[spike]** = pequeno experimento técnico recomendado antes de se comprometer.
> - Números de tamanho/desempenho são **metas a validar**, não medições.

---

## 1. Resumo executivo

O **Polyglottus** traduz livros e documentos inteiros, no maior número possível de formatos (EPUB, PDF, DOCX e muitos outros), preservando estrutura, imagens e o máximo possível do layout. Roda localmente, é leve, e usa modelos locais como caminho principal. API de terceiros é opcional e usa a chave do próprio usuário.

**Princípios inegociáveis**

1. **Gratuito e open source**, sem taxa, sem conta, sem telemetria obrigatória.
2. **Local-first:** o livro nunca sai da máquina, a menos que o usuário escolha uma API externa (o app avisa de forma explícita).
3. **Rust-first:** a maior parte do código é Rust. Onde Rust sozinho não basta, usamos bibliotecas nativas e binários auxiliares **isolados atrás de interfaces** (seção 4).
4. **Leve:** instalador pequeno, pouca RAM, abertura rápida, sem Python e sem Electron no núcleo.
5. **Código limpo e fácil de manter:** arquitetura hexagonal, crates pequenos com fronteiras claras, testes sem dependência de modelo, CI rigoroso (seções 6, 7 e 10).
6. **Honestidade sobre fidelidade:** cada formato tem um nível de fidelidade declarado e visível ao usuário.

---

## 2. Panorama competitivo (pesquisa)

O espaço já tem projetos bons. O Polyglottus não é pioneiro em "traduzir livro com LLM local"; o diferencial precisa estar em outro lugar (seção 3).

| Projeto | Foco | Formatos | Local / API | Destaques | Limitações observadas |
|---|---|---|---|---|---|
| **TranslateBooksWithLLMs (TBL)** | Livros, legendas, documentos | EPUB, DOCX, TXT, SRT | Ollama, servidores compatíveis com OpenAI (llama.cpp, LM Studio, vLLM), OpenRouter, OpenAI, Gemini e outros | App desktop, web UI e CLI; checkpoints para retomar; sem limite de tamanho; AGPL-3.0 | Sem OCR de imagens nem PDF nos formatos listados; núcleo em Python **[verificar]** |
| **Tolmach** | Tradução literária | TXT, EPUB, PDF, DOCX | Ollama local | Glossário por documento; mesa de revisão lado a lado (original / rascunho / final editável) | Interface web local (Flask); exige vários modelos locais pesados |
| **epub-tr** | EPUB literário | EPUB | Ollama, LLMs gratuitos via CLI, MT | Glossário próprio, passe de polimento, edição bilíngue, validação com EPUBCheck, cache | Só EPUB; CLI |
| **bilingual_book_maker** | Edições bilíngues | EPUB, TXT, SRT | Principalmente APIs | MIT; gera EPUB bilíngue | CLI, foco em API |
| **Ebook Translator (plugin Calibre)** | Tradução dentro do Calibre | Formatos do Calibre | Muitos motores | Aproveita a conversão do Calibre; modo bilíngue; GPL-3.0 | Depende do Calibre; pode ser lento |
| **BreviaBook** | Condensar e traduzir | EPUB, PDF → EPUB/PDF/MD | Ollama + APIs | Representação intermediária, glossário por capítulo, `--dry-run` com estimativa, `--resume` | CLI; foco em resumo |
| **PDFMathTranslate / BabelDOC** | PDF científico com layout | PDF | Vários, incl. Ollama | BabelDOC separa layout e semântica numa IR bidirecional, com glossário e contexto entre páginas; PDFMathTranslate usa detecção de layout por modelo | Foco em artigos; implementação em Python |
| **comic-translate / manga-image-translator / Koharu** | Texto em imagens de quadrinhos | Imagens | OCR + MT/LLM, alguns offline | Detectar → OCR → remover texto → traduzir → reescrever; **Koharu é em Rust** | Voltados a quadrinhos, não a livros |

**Fontes consultadas:** repositórios e páginas dos projetos acima, artigo ACL do BabelDOC, relatório técnico do TranslateGemma, documentação do Tauri (Flatpak), docs.rs/crates.io (docx-rs, lopdf, pdfium-render, mdkit, ocrs, llama-cpp-2).

---

## 3. Diferencial e lacunas

Nenhum projeto encontrado combina **tudo** isto:

- App desktop com **instalador de verdade** (sem Python, terminal ou navegador manual) e **leve**.
- **Maior cobertura de formatos** numa só ferramenta (livros, documentos de escritório, legendas, PDF, formatos de localização).
- **OCR de texto em imagens do livro** com legenda traduzida.
- **Revisão manual** integrada, com memória de tradução.
- **Gerenciador de modelos** com detecção de hardware.
- **Estimativa de tempo/custo** antes de começar, e retomada automática.
- Núcleo em **Rust**, testável sem modelo e fácil de contribuir.

**Decisão estratégica (ADR-0002):** construir do zero, contribuir com o TBL ou forkar? Este documento assume **construir do zero** pelas razões acima (leveza, Rust, cobertura de formatos), mas a decisão deve ser formalizada após um **[spike]** de 1–2 semanas comparando o esforço de reaproveitar o TBL.

---

## 4. Rust-first: onde Rust basta e onde não

Rust faz a maior parte do trabalho. Mas "Rust sozinho não faz tudo", e a estratégia é admitir isso e **isolar o que não é Rust atrás de portas (traits)**, para poder trocar sem tocar no domínio.

| Necessidade | Solução | Rust puro? | Observação |
|---|---|---|---|
| Domínio, pipeline, jobs, QA, glossário, segmentação | Rust | Sim | O coração do projeto |
| EPUB, HTML, XHTML, FB2, ODF, OOXML (DOCX/PPTX/XLSX), SRT/VTT, TXT/MD | Rust (zip + quick-xml + html5ever/scraper) | Sim | Estratégia de "patch no lugar" (seção 8) |
| DOCX (leitura/escrita avançada) | `docx-rs` (MIT) ou patch direto no XML | Sim | Patch direto tende a preservar mais fidelidade **[spike]** |
| Estrutura de PDF | `lopdf` (Rust) | Sim | Manipulação de objetos |
| Texto e render de PDF | `pdfium-render` (binding do PDFium, C++) | Não | PDFium em processo; peso moderado (~5 MB segundo a documentação do mdkit) |
| OCR (alfabeto latino) | `ocrs` (Rust, modelos ONNX via RTen; Apache-2.0/MIT) | Sim | Segundo o README, em *early preview* e **só alfabeto latino** hoje |
| OCR (outros alfabetos, mais precisão) | Tesseract (C++) ou modelos ONNX via `ort` | Não | Entregar como "pacote de OCR" opcional |
| Inferência local | `llama.cpp` via `llama-cpp-2` (Apache-2.0) ou servidor externo (Ollama/llama-server) | Não (C++) | O crate declara que **não segue semver**: fixar versão |
| Modelos de MT dedicados (NLLB/MADLAD) | ONNX via `ort` ou CTranslate2 | Não | Fase 3 **[verificar crates e licenças]** |
| Formatos de cauda longa (MOBI, AZW3, RTF, DOC, LaTeX, DJVU, CBZ…) | **Pandoc** e/ou conversores externos como pacote opcional | Não | Padrão "despachar para a melhor ferramenta por formato", como no `mdkit` |
| Interface | Tauri (Rust) + front-end web leve | Parcial | Front-end em TypeScript |
| Banco | SQLite (`rusqlite`/`sqlx`) | Quase | C embutido, trivial de empacotar |

**Python fica fora do núcleo.** Evita o peso de empacotamento (a doc do `mdkit` cita um sidecar Python de ~350 MB como o problema típico) e simplifica o build offline do Flathub.

---

## 5. Decisões de arquitetura (ADRs iniciais)

Registrar cada uma em `docs/adr/NNNN-titulo.md` (formato curto: contexto, decisão, consequências).

| ADR | Decisão proposta | Alternativa considerada |
|---|---|---|
| 0001 | Licença do projeto (ver seção 14) | MIT/Apache vs. GPL/AGPL |
| 0002 | Construir do zero vs. contribuir com TBL | Fork / contribuição upstream |
| 0003 | **Arquitetura hexagonal** (domínio puro, portas, adaptadores) | MVC/camadas tradicionais |
| 0004 | **Unidades de tradução + patch no lugar** em vez de um modelo de documento completo (seção 8) | IR completa de documento (estilo BabelDOC) |
| 0005 | UI: Tauri + front-end leve (Svelte ou Solid) | Slint/Iced (UI 100% Rust); React |
| 0006 | Inferência atrás de uma porta `TranslationProvider`, com processo separado para o motor embutido | Inferência dentro do processo da UI |
| 0007 | Sem Python no núcleo | Sidecar Python para OCR |
| 0008 | Pandoc como "pacote de formatos" opcional | Escrever todos os leitores em Rust |
| 0009 | SQLite como armazenamento de projeto (unidades, estado, glossário, memória de tradução) | Arquivos JSON |
| 0010 | Distribuição: instalador Windows + Flatpak (Flathub) | AppImage/deb/rpm adicionais |

### 5.1 UI: por que Tauri (e a alternativa 100% Rust)

| Opção | Prós | Contras |
|---|---|---|
| **Tauri + Svelte/Solid + TypeScript** (proposta) | Leve (usa a webview do sistema), UI rica (edição lado a lado, listas virtualizadas, acessibilidade), ecossistema grande | Duas linguagens; webview difere entre Windows (WebView2) e Linux (WebKitGTK) |
| Tauri + React | Mais conhecido pela comunidade | Bundle e runtime maiores que Svelte/Solid |
| Slint / Iced / egui (UI em Rust) | Uma só linguagem, binário único, sem webview | Edição de texto rica, acessibilidade e virtualização de listas enormes são mais difíceis **[verificar maturidade]** |

**Regra de ouro:** o núcleo é uma **biblioteca headless** (`pgl-app`) com uma CLI (`pgl-cli`). A UI é só mais um adaptador. Isso permite trocar a UI no futuro sem reescrever nada e facilita testar o produto inteiro sem interface.

---

## 6. Arquitetura (hexagonal / portas e adaptadores)

```
┌───────────────────────────── Adaptadores de entrada ─────────────────────────────┐
│   UI Tauri (comandos + eventos)            CLI (pgl-cli)            (futuro: API) │
└───────────────────────────────────────┬──────────────────────────────────────────┘
                                        ▼
┌───────────────────────── Aplicação (pgl-app): casos de uso ──────────────────────┐
│ ImportarLivro · PlanejarTraducao · ExecutarJob · RevisarUnidade · Exportar · ... │
│ Define as PORTAS (traits) que o mundo externo deve implementar                    │
└───────────────────────────────────────┬──────────────────────────────────────────┘
                                        ▼
┌─────────────────────── Domínio (pgl-domain): sem I/O, sem async ─────────────────┐
│ Unit, MarkedText, Glossary, StyleGuide, Project, JobState, regras de QA,         │
│ regras de segmentação, proteção de marcação                                       │
└───────────────────────────────────────▲──────────────────────────────────────────┘
                                        │ (dependências apontam sempre para dentro)
┌──────────────────────────── Adaptadores de saída (infra) ────────────────────────┐
│ Formatos (epub, docx, pdf, …) · Provedores (OpenAI-compat, Ollama, embutido)     │
│ OCR (ocrs, tesseract) · Storage (SQLite) · Segredos (keyring) · Modelos · Logs   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Regras:**
- `pgl-domain` não conhece SQLite, HTTP, arquivos nem Tauri. É testável com testes unitários puros.
- `pgl-app` só conhece o domínio e as **traits** das portas.
- Adaptadores implementam as portas e **nunca** são importados pelo domínio ou pela aplicação. A montagem (injeção de dependências) acontece só nos binários (`pgl-cli`, `apps/desktop/src-tauri`).
- Em CI, validar o grafo de dependências entre crates (por exemplo com `cargo-deny` + verificação do grafo) para impedir violações **[verificar ferramenta]**.

### 6.1 Portas principais (esboço)

```rust
// Leitura de formatos: produz unidades de forma incremental (streaming)
pub trait FormatReader: Send + Sync {
    fn id(&self) -> FormatId;
    fn sniff(&self, name: &str, head: &[u8]) -> Confidence;
    fn open(&self, path: &Path) -> Result<Box<dyn SourceDocument>, FormatError>;
}

pub trait SourceDocument {
    fn units(&mut self) -> Box<dyn Iterator<Item = Result<Unit, FormatError>> + '_>;
    fn assets(&self) -> Vec<AssetRef>;
    fn fidelity(&self) -> FidelityReport;   // o que será preservado e o que não
}

// Escrita: aplica traduções ao original (patch no lugar) ou regenera
pub trait FormatWriter: Send + Sync {
    fn write(
        &self,
        source: &Path,
        translations: &dyn TranslationLookup,
        options: &ExportOptions,   // bilíngue, formato de saída, etc.
        out: &Path,
    ) -> Result<ExportReport, FormatError>;
}

pub trait TranslationProvider: Send + Sync {
    fn capabilities(&self) -> Capabilities; // contexto máx., idiomas, local/remoto
    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError>;
}

pub trait OcrEngine, ProjectRepository, SecretStore, ModelCatalog, EventSink { /* ... */ }
```

> `async fn` em traits é estável, mas não é compatível com `dyn`; usar `async-trait` ou despacho por enum nos pontos de montagem.

---

## 7. Modelo de dados: unidades de tradução

Em vez de modelar o "documento inteiro" (caro e frágil), o domínio trabalha com **unidades de tradução**. O leitor extrai unidades; o escritor devolve as traduções ao arquivo original.

```rust
pub struct Unit {
    pub id: UnitId,             // estável: hash(source_ref + texto original)
    pub source_ref: SourceRef,  // aponta para o nó original (arquivo interno + caminho/offset)
    pub kind: UnitKind,         // Heading(level), Paragraph, ListItem, TableCell,
                                // Caption, AltText, Footnote, Subtitle, Metadata…
    pub text: MarkedText,       // texto com marcação inline protegida
    pub section: SectionId,     // capítulo/seção (contexto e progresso)
}

// MarkedText: "Ele disse ⟦1⟧não⟦/1⟧ e saiu." — as tags inline viram placeholders
// validados na volta (contagem, ordem, aninhamento).
```

**Estado por unidade (persistido no SQLite):** `Pending → Translated → Flagged → Reviewed` (ou `Failed`). Isso torna o job **idempotente e retomável** por construção.

**Memória de tradução (TM):** cache por hash do texto + par de idiomas + glossário, reaproveitando frases repetidas e permitindo importar/exportar **TMX** (formato padrão de localização).

**Vantagens:** menos código, streaming natural (nunca carrega o livro inteiro na RAM), e o mesmo modelo serve a EPUB, DOCX, legendas e PDF.

---

## 8. Formatos: cobertura máxima em três camadas

Objetivo: aceitar **qualquer formato razoável**. A estratégia é em camadas, sempre com um **selo de fidelidade** exibido ao usuário antes de traduzir.

### 8.1 Camada A — Nativa em Rust (leitura e escrita, alta fidelidade)

**Motor de "patch no lugar":** abre o contêiner original (zip/XML), localiza os nós de texto traduzíveis, substitui pelo texto traduzido e regrava tudo o mais **sem alterar**. Um único motor atende uma família inteira de formatos:

| Família | Formatos | Observação |
|---|---|---|
| Pacotes OOXML (OPC) | **DOCX, PPTX, XLSX** | Texto em `w:t`, `a:t`, strings compartilhadas; atenção à fragmentação em "runs" |
| ODF | ODT, ODP, ODS | Mesma técnica (XML em zip) |
| E-books | **EPUB** (2 e 3), FB2 | XHTML + CSS + OPF; reescrever sumário (NCX/nav) e metadados |
| Web/Texto | HTML/XHTML, TXT, Markdown, reStructuredText (básico) | Marcação inline protegida |
| Legendas | SRT, VTT, ASS (texto dos diálogos) | Preservar tempos e estilos |
| Localização | PO/POT, XLIFF, TMX, JSON/YAML de i18n | Uso para desenvolvedores; baixo custo e alto valor |

### 8.2 Camada B — PDF (layout fixo, estratégia própria)

PDF não tem "parágrafos"; tem caracteres posicionados. Três modos, do mais confiável ao mais ambicioso:

| Modo | O que faz | Fidelidade | Dificuldade |
|---|---|---|---|
| **1. Reflow** | Extrai o texto em ordem de leitura e **exporta como EPUB/DOCX/HTML** traduzido | Estrutura boa, layout original **não** preservado | Média |
| **2. Sobreposição (overlay)** | Cobre cada caixa de texto original e desenha o texto traduzido na mesma caixa, ajustando o tamanho da fonte | Boa em PDFs simples; ruim em colunas densas, fórmulas e tabelas | Alta |
| **3. Re-diagramação com análise de layout** | Detecta blocos (colunas, tabelas, fórmulas) com modelo ONNX, traduz e re-tipografa — a abordagem do BabelDOC/PDFMathTranslate, reimplementada em Rust | Melhor resultado possível | Muito alta |

Base técnica em Rust: `pdfium-render` (texto com posições, renderização de páginas, edição de objetos **[spike]**) e `lopdf` (estrutura). Tipografia: `rustybuzz`/`cosmic-text` para escrita complexa (árabe, índico) e `fontdb` para fontes. **PDF escaneado:** renderizar páginas com PDFium e passar pelo OCR.

**Compromisso realista:** MVP+1 entrega o modo 1 (reflow). O modo 2 vem na sequência. O modo 3 é pesquisa de longo prazo.

### 8.3 Camada C — Cauda longa via conversores (pacote opcional)

Formatos que não valem um leitor próprio em Rust: **MOBI, AZW3, RTF, DOC (legado), LaTeX, DJVU, CBZ/CBR, ODT antigo, etc.**

Fluxo: `converter para pivô (EPUB/DOCX) → traduzir pela Camada A → (opcional) converter de volta`.

- **Pandoc** como conversor principal (DOCX, EPUB, RTF, ODT, LaTeX, HTML e muitos outros); pesa na casa de ~150 MB, por isso é um **pacote opcional**, não parte do instalador base.
- Outros conversores (Calibre `ebook-convert`, LibreOffice headless) só onde o empacotamento permitir **[verificar]**, sobretudo dentro do Flatpak.
- A conversão de volta pode perder fidelidade; o selo de fidelidade deve avisar.
- **DRM:** o Polyglottus **não remove DRM**.

### 8.4 Matriz de formatos

| Formato | Entrada | Saída | Camada | Fidelidade esperada | Fase |
|---|---|---|---|---|---|
| EPUB | ✔ | ✔ | A | Alta | 1 |
| HTML/XHTML | ✔ | ✔ | A | Alta | 1 |
| TXT / Markdown | ✔ | ✔ | A | Alta | 1 |
| SRT / VTT | ✔ | ✔ | A | Alta | 1 |
| DOCX | ✔ | ✔ | A | Alta (patch no lugar) | 2 |
| PPTX / XLSX | ✔ | ✔ | A | Alta | 2 |
| ODT / ODP / ODS | ✔ | ✔ | A | Alta | 2 |
| FB2 | ✔ | ✔ | A | Alta | 2 |
| PO / XLIFF / TMX / JSON i18n | ✔ | ✔ | A | Alta | 2 |
| Imagens soltas (PNG/JPG/WebP) | ✔ | legenda/overlay | A + OCR | Média | 2 |
| PDF com texto | ✔ | EPUB/DOCX/HTML (reflow) | B1 | Média (estrutura) | 3 |
| PDF com texto | ✔ | PDF (overlay) | B2 | Média/Variável | 3 |
| PDF escaneado | ✔ | EPUB/DOCX (OCR) | B + OCR | Média/Baixa | 4 |
| MOBI / AZW3 / RTF / DOC / LaTeX / DJVU | ✔ | via pivô | C | Variável | 3 |
| CBZ/CBR (quadrinhos) | ✔ | CBZ | C + OCR | Baixa/Experimental | 4 |

### 8.5 Problemas recorrentes (precisam de solução desenhada)

- **Marcação inline:** placeholders + validação (contagem, ordem, aninhamento). Testes baseados em propriedades.
- **Expansão/contração do texto** entre idiomas (crítico em PDF, tabelas, legendas).
- **Escrita da direita para a esquerda (RTL)** e **fontes** para o idioma de destino (CJK, árabe, índico): usar fontes do sistema via `fontdb` e/ou pacote de fontes opcional (famílias Noto são livres, mas CJK é pesado).
- **Sumário, links internos, notas de rodapé** devem permanecer consistentes.
- **Convenções tipográficas** por idioma (aspas, travessões e diálogos).
- **Hifenização e quebras** em PDF (reunir palavras partidas antes de traduzir).

---

## 9. Pipeline de processamento

```
Importar → Detectar formato/idioma → Extrair unidades (streaming)
   → Preparar (glossário, guia de estilo, estimativa)
   → Traduzir (lotes com contexto + memória de tradução)
   → QA automático → (reprocessar falhas) → Revisão manual
   → Exportar (formato original e/ou outros, bilíngue)
```

Cada etapa grava seu estado no SQLite. **Pausar, fechar o app, cair a energia ou trocar de modelo no meio não perde trabalho.**

- **Lotes com contexto:** janela deslizante com sobreposição, resumo rolante do capítulo, glossário e guia de estilo no prompt.
- **Consistência em livros longos:** glossário gerado numa passada de preparação (nomes, termos) e editável; reaplicável.
- **Estimativa antes de iniciar:** tokens, tempo por perfil de hardware e custo (se API), como o `--dry-run` do BreviaBook.
- **Concorrência controlada:** canais com limite (backpressure), nº de requisições paralelas configurável, cancelamento cooperativo.

### 9.1 QA automático por unidade

- Idioma da saída coincide com o idioma de destino?
- Razão de tamanho original/tradução dentro da faixa esperada para o par de idiomas?
- Todos os placeholders voltaram, na ordem e aninhamento corretos?
- Repetição anormal de n-gramas (sinal de alucinação em laço)?
- Termos do glossário respeitados? Números, datas e nomes próprios preservados?

Unidade reprovada → reprocessa (prompt alternativo, outro modelo, janela menor) → se falhar, vai para a fila de revisão.

---

## 10. Inferência e modelos

### 10.1 Uma porta, vários adaptadores

| Adaptador | Cobre | Observação |
|---|---|---|
| **OpenAI-compatível (HTTP)** | llama-server, LM Studio, vLLM, LocalAI, APIs do usuário | O mais genérico; **prioridade no MVP** |
| **Ollama (API nativa)** | Ollama | Experiência de modelos já instalados |
| **Motor embutido (llama.cpp)** | "Funciona sem instalar nada" | Processo separado (`pgl-engine`) para isolar falhas e memória de GPU; fixar versão do `llama-cpp-2` |
| **MT dedicada (NLLB/MADLAD)** | Idiomas de poucos recursos | Fase 3 |

### 10.2 Famílias de modelos

| Família | Exemplos | Pontos fortes | Pontos fracos |
|---|---|---|---|
| LLMs gerais ou ajustados p/ tradução | TranslateGemma (4B, 12B, 27B; 55 idiomas), Qwen, Gemma, Mistral | Seguem instruções (glossário, estilo, formato); melhor fluência | Qualidade cai em idiomas raros; podem repetir/alucinar |
| Modelos dedicados de MT | NLLB-200, MADLAD-400, Opus-MT | Cobertura muito ampla, rápidos, estáveis | Não seguem instruções complexas |

Notas da pesquisa:
- O relatório técnico do TranslateGemma indica que o modelo de 12B superou o Gemma 3 de 27B base em métricas automáticas, e o 12B foi pensado para notebooks comuns.
- Avaliações da comunidade relatam grande variação por idioma: alguns quase perfeitos, outros com inserções e repetições. Isso exige **QA automático e aviso de qualidade por idioma**.
- Guias comparativos apontam que modelos dedicados (como o NLLB) tendem a vencer LLMs gerais em idiomas de poucos recursos.

### 10.3 Perfis de hardware (valores aproximados, **[verificar]**)

| Perfil | Hardware | Sugestão |
|---|---|---|
| Leve | CPU, 8 GB de RAM | Modelo de ~4B quantizado (~3 GB) |
| Médio | 16 GB de RAM ou GPU 8–12 GB | Modelo de ~12B quantizado (~8 GB) |
| Alto | GPU 16+ GB | Modelo de ~27B quantizado (~17 GB) |

O app **detecta o hardware** no primeiro uso, recomenda um modelo, mostra tamanho de download e licença, e estima o tempo por livro. **O modo CPU deve sempre funcionar.**

---

## 11. OCR e imagens

**Fluxo (MVP+):**
1. Extrair as imagens do livro e **copiá-las sem recompressão** (bytes originais), armazenadas por hash (sem duplicar).
2. Heurística para decidir se vale rodar OCR (evitar fotos sem texto).
3. OCR → traduzir com contexto (legenda e parágrafos vizinhos).
4. Inserir **legenda traduzida abaixo da imagem**, marcada como gerada por OCR e editável.
5. Confiança baixa → sinalizar para revisão em vez de inserir texto errado.

**Camadas de OCR:**

| Camada | Motor | Quando |
|---|---|---|
| Padrão | `ocrs` (Rust puro, leve) | Alfabeto latino |
| Pacote de OCR | Tesseract e/ou modelos ONNX via `ort` | Outros alfabetos e maior precisão |
| Experimental | Modelo de visão local (VLM) | Casos difíceis; mais lento |

**Futuro:** remover o texto original da imagem e reescrever a tradução sobre ela (inpainting), como fazem projetos de quadrinhos (incluindo o Koharu, em Rust). Não é MVP.

---

## 12. Estrutura do repositório

Monorepo com **Cargo workspace** (Rust) e **pnpm workspace** (front-end). Pastas agrupadas por camada da arquitetura.

```
polyglottus/
├─ Cargo.toml                    # workspace (members = crates/**, apps/desktop/src-tauri)
├─ Cargo.lock
├─ rust-toolchain.toml           # versão fixa do Rust (MSRV documentada)
├─ rustfmt.toml  clippy.toml  deny.toml  .editorconfig
├─ justfile                      # atalhos: just check, just test, just run, just package
├─ README.md  LICENSE  CONTRIBUTING.md  CODE_OF_CONDUCT.md  SECURITY.md  CHANGELOG.md
│
├─ crates/
│  ├─ domain/
│  │  └─ pgl-domain/             # Unit, MarkedText, Glossary, StyleGuide, JobState,
│  │                             # regras de QA/segmentação/placeholders (sem I/O, sem async)
│  ├─ app/
│  │  └─ pgl-app/                # casos de uso + PORTAS (traits) + eventos de progresso
│  ├─ adapters/
│  │  ├─ formats/
│  │  │  ├─ pgl-fmt-core/        # utilitários comuns: zip seguro, XML, SourceRef, sniffing
│  │  │  ├─ pgl-fmt-patch/       # motor de "patch no lugar" (OPC/ODF/EPUB/FB2)
│  │  │  ├─ pgl-fmt-epub/
│  │  │  ├─ pgl-fmt-ooxml/       # DOCX, PPTX, XLSX
│  │  │  ├─ pgl-fmt-odf/         # ODT, ODP, ODS
│  │  │  ├─ pgl-fmt-text/        # TXT, Markdown, HTML, legendas, PO/XLIFF/TMX
│  │  │  ├─ pgl-fmt-pdf/         # PDF (reflow, overlay) sobre pdfium-render + lopdf
│  │  │  └─ pgl-fmt-convert/     # cauda longa via Pandoc (pacote opcional)
│  │  ├─ providers/
│  │  │  ├─ pgl-provider-openai/ # OpenAI-compatível (inclui servidores locais)
│  │  │  ├─ pgl-provider-ollama/
│  │  │  └─ pgl-provider-embedded/ # cliente do pgl-engine (llama.cpp)
│  │  ├─ pgl-ocr/                # ocrs (padrão) + Tesseract/ONNX (opcional)
│  │  ├─ pgl-storage/            # SQLite: unidades, estado, glossário, TM, histórico
│  │  ├─ pgl-secrets/            # cofre do sistema (keyring)
│  │  └─ pgl-models/             # catálogo, detecção de hardware, download e licenças
│  ├─ bins/
│  │  ├─ pgl-cli/                # CLI (também usada em testes de ponta a ponta)
│  │  └─ pgl-engine/             # processo isolado de inferência (llama.cpp)
│  └─ testing/
│     └─ pgl-testkit/            # FakeProvider determinístico, fixtures, helpers
│
├─ apps/
│  └─ desktop/
│     ├─ src-tauri/              # shell Tauri: comandos, eventos, montagem das dependências
│     └─ ui/                     # front-end (Svelte ou Solid + TypeScript)
│        └─ src/
│           ├─ app/              # bootstrap, rotas, tema
│           ├─ features/         # import/ setup/ progress/ review/ export/ models/ settings/
│           ├─ shared/           # components/ hooks/ ipc/ i18n/ styles/ (tokens de design)
│           └─ tests/
│
├─ packaging/
│  ├─ flatpak/                   # manifesto, metainfo.xml, .desktop, fontes cargo/node offline
│  └─ windows/                   # configuração do instalador, assinatura
│
├─ test-corpus/                  # livros de domínio público e casos difíceis (RTL, CJK, tabelas…)
├─ docs/
│  ├─ adr/                       # decisões de arquitetura (ADRs)
│  ├─ architecture/              # diagramas (Mermaid), guia dos crates
│  ├─ formats/                   # matriz de fidelidade por formato
│  └─ contributing/              # como adicionar um formato, um provedor, um idioma de UI
├─ tools/                        # scripts de dev, geração de tipos, auditoria de licenças
└─ .github/workflows/            # CI (ver seção 13)
```

**Princípios da organização**
- **Um crate = uma responsabilidade.** Compilação incremental rápida e testes isolados.
- **Adicionar um formato** = criar um crate `pgl-fmt-xxx` que implementa `FormatReader`/`FormatWriter` e registrá-lo em **um** lugar. Nada mais muda.
- **Adicionar um provedor** = idem com `TranslationProvider`.
- **Tipos compartilhados com o front-end** são gerados a partir do Rust (por exemplo com `specta`/`ts-rs` **[verificar]**), eliminando divergência entre o IPC e o TypeScript.
- **Recursos opcionais** (Pandoc, Tesseract, motor embutido) ficam atrás de *feature flags* e de pacotes separados, para manter o instalador base pequeno.

---

## 13. Engenharia: código limpo e fácil de manter

### 13.1 Convenções de código

- **Fronteiras:** módulos pequenos, funções curtas, tipos expressivos (*newtypes* para `UnitId`, `SectionId`, `Lang` etc.), sem `String` genérica onde existe um conceito de domínio.
- **Erros:** `thiserror` nas bibliotecas (erros tipados por camada); `anyhow` **somente** nos binários. Nada de `unwrap()`/`expect()` fora de testes.
- **Segurança de memória:** `#![forbid(unsafe_code)]` em todos os crates, **exceto** nos adaptadores que fazem FFI (e esses isolam o `unsafe` em módulos pequenos e revisados).
- **Observabilidade:** `tracing` com níveis e *spans* por job/unidade; logs locais com rotação; nada enviado para fora.
- **Configuração:** estruturas tipadas com `serde`, valores padrão sensatos, validação na carga.
- **Concorrência:** `tokio` apenas nas camadas de aplicação/infra; domínio síncrono e puro.
- **Dependências:** poucas e justificadas; `cargo-deny` (licenças, duplicatas, fontes) e `cargo-audit` (vulnerabilidades) no CI.
- **Documentação:** `///` em toda API pública; `docs/architecture` com diagramas; **ADR** para cada decisão relevante.

### 13.2 Estratégia de testes (pirâmide)

| Nível | O que testa | Ferramentas |
|---|---|---|
| Unitários | Domínio puro: placeholders, QA, segmentação, glossário | `cargo test`, `proptest` (propriedades: *proteger → restaurar* é identidade) |
| Snapshot/golden | Leitores e escritores de formato contra arquivos reais | `insta` + `test-corpus/` |
| Contrato das portas | Todo adaptador passa na mesma suíte de contrato | testes genéricos em `pgl-testkit` |
| Integração | Pipeline inteiro com **`FakeProvider`** (tradutor determinístico, ex.: inverte/marca o texto), sem modelo real | `pgl-testkit`, `pgl-cli` |
| Robustez | Parsers contra entradas hostis | `cargo-fuzz` nos leitores de zip/XML/PDF |
| Desempenho | Tempo e memória em livros grandes | `criterion`; teste de "livro de 1.000+ páginas" com limite de RAM |
| Validação de saída | EPUB válido, DOCX abre sem reparo | EPUBCheck; verificações estruturais de OOXML |
| UI | Componentes e fluxos principais | Vitest + testes de ponta a ponta **[verificar ferramenta com Tauri/WebKitGTK]** |
| Qualidade de tradução | Regressão em amostra com referência | chrF/COMET ou similar + revisão humana amostral |

> O **`FakeProvider`** é o que mantém o código testável e rápido: o pipeline é validado sem GPU, sem rede e sem modelo.

### 13.3 CI/CD (GitHub Actions)

| Job | Conteúdo |
|---|---|
| `lint` | `cargo fmt --check`, `cargo clippy -D warnings`, lint/format do front-end |
| `test` | `cargo test` + testes do front-end, em **Windows e Linux** |
| `supply-chain` | `cargo deny`, `cargo audit` |
| `docs` | `cargo doc` sem avisos; checagem de links |
| `corpus` | Roda o pipeline com `FakeProvider` sobre o `test-corpus/` e valida saídas |
| `package-flatpak` | Gera o Flatpak a partir do manifesto (build offline) |
| `package-windows` | Gera o instalador |
| `release` | Tags semânticas, changelog gerado, artefatos com checksums |

**Processo:** *Conventional Commits*, SemVer, PRs pequenos, `CODEOWNERS`, templates de issue/PR, rótulos *good first issue*, MSRV documentada.

---

## 14. Licenças, segurança e questões legais

### 14.1 Licenças (checar todas antes da ADR-0001)

| Componente | Licença observada/esperada | Nota |
|---|---|---|
| `lopdf`, `docx-rs`, `ocrs`, `llama-cpp-2` | MIT / Apache-2.0 (conforme páginas dos crates) | Permissivas |
| PDFium | BSD-3 / Apache-2.0 **[verificar]** | Binário distribuído com o app |
| llama.cpp | MIT **[verificar]** | |
| Pandoc | GPL **[verificar]** | Chamado como processo separado e distribuído como **pacote opcional**; confirmar obrigações de distribuição |
| Tesseract | Apache-2.0 **[verificar]** | |
| **PyMuPDF / MuPDF** | **AGPL** (ou comercial) **[verificar]** | **Evitar:** forçaria compatibilidade com AGPL; PDFium + lopdf dispensam |
| Modelos (Gemma etc.) | Termos próprios de cada modelo | O app **não distribui** modelos; mostrar a licença antes do download |

**Recomendação para a ADR-0001:** licença permissiva (Apache-2.0 ou MIT) para máxima adoção, **ou** GPL-3.0/AGPL-3.0 se o objetivo for impedir forks fechados (o TBL usa AGPL-3.0). Decidir após a auditoria de dependências.

### 14.2 Segurança e privacidade

- **Sem telemetria por padrão.** Diagnóstico opcional, *opt-in*, local por padrão.
- **Chaves de API** no cofre do sistema (Credential Manager no Windows; Secret Service no Linux), nunca em arquivo de texto **[verificar acesso dentro do Flatpak]**.
- **Entradas hostis:** limite de tamanho e taxa de descompressão (zip bomb), bloqueio de *zip slip* (caminhos `../`), XML sem resolução de entidades externas, nenhum script do livro é executado.
- **Webview endurecida:** CSP estrita, permissões mínimas do Tauri, sem carregar conteúdo remoto na interface.
- **Aviso explícito** quando uma API externa for escolhida: "o texto do livro será enviado ao provedor X".

### 14.3 Legal

- Não remove DRM. Direitos autorais do conteúdo traduzido são responsabilidade do usuário; incluir aviso claro.
- Mostrar a licença de cada modelo antes do download.

---

## 15. UI/UX

### 15.1 Princípios

- **Simples por padrão, poderoso sob demanda:** o usuário comum escolhe arquivo e idioma; o resto tem padrões sensatos e uma área "Avançado".
- **Nunca perder trabalho:** salvamento contínuo e retomada automática.
- **Transparência:** mostrar o que está sendo feito, o tempo estimado e quando algo vai para a internet.
- **Erros acionáveis:** "este trecho falhou por X; tentar com outro modelo?" em vez de mensagens técnicas.
- **Acessibilidade:** navegação por teclado completa, contraste adequado (WCAG AA), suporte a leitor de tela, escala de fonte, tema claro/escuro.
- **Internacionalização da interface:** pt-BR, en, es no mínimo, com arquivos de tradução simples para contribuição da comunidade.
- **Desempenho percebido:** listas virtualizadas (um livro tem dezenas de milhares de unidades), nenhuma operação longa na thread da UI, *skeletons* em vez de telas vazias.

### 15.2 Fluxo de telas

1. **Início:** arrastar e soltar arquivo; projetos recentes. Detecta formato e idioma, mostra título, nº de capítulos/palavras e **selo de fidelidade** do formato.
2. **Configuração:** idioma de destino; **Local (recomendado)** ou **API própria**; modelo recomendado para o hardware (com tamanho e licença); formalidade/estilo; glossário opcional; formato de saída (inclui bilíngue).
3. **Estimativa:** tempo previsto, RAM/disco, custo se for API. Botão **Iniciar**.
4. **Progresso:** barra geral, capítulo atual, velocidade/ETA, unidades sinalizadas; pausar/retomar; segue em segundo plano com notificação ao terminar.
5. **Revisão:** lado a lado (original | tradução), filtros "com alerta de QA", edição inline, "retraduzir este trecho", destaque de termos do glossário, desfazer/refazer, atalhos de teclado.
6. **Exportação:** escolher formatos de saída, prévia, relatório de qualidade.
7. **Modelos:** instalados, recomendados, licenças, tamanho, baixar/remover.
8. **Configurações:** idioma da interface, chaves de API, pasta de dados, diagnóstico.

**Primeiro uso (assistente):** detectar hardware → recomendar modelo → baixar → traduzir um parágrafo de teste. Em menos de um minuto o usuário vê o app funcionando.

### 15.3 Sistema de design

Tokens de design (cores, espaçamento, tipografia) em `shared/styles`, componentes pequenos e reutilizáveis, estados vazios/erro/carregando padronizados, e documentação visual curta em `docs/`.

---

## 16. Leveza (metas a validar)

| Item | Meta inicial |
|---|---|
| Instalador base (sem modelos, sem pacotes opcionais) | Menor possível; **acompanhar o tamanho a cada release** |
| RAM ociosa do app | Baixa (dezenas de MB, fora o modelo) |
| Abertura do app | < 1–2 s em hardware modesto |
| RAM durante tradução (sem contar o modelo) | Constante, independente do tamanho do livro (streaming + SQLite) |
| Front-end | Bundle pequeno (Svelte/Solid), listas virtualizadas, carregamento sob demanda |

**Como manter leve:**
- Webview do sistema (Tauri), não Chromium embutido.
- Núcleo em Rust com *streaming*: o livro **nunca** é carregado inteiro na memória; unidades vivem no SQLite (modo WAL); imagens ficam em armazenamento endereçado por hash.
- Recursos pesados (Pandoc, Tesseract, fontes CJK, motor embutido, modelos) são **pacotes opcionais**.
- Perfil de *release* otimizado (LTO, `codegen-units=1`, `strip`) e monitoramento de tamanho no CI.

---

## 17. Plataformas e empacotamento

Escopo: **Windows** e **Linux via Flatpak**.

### 17.1 Flatpak (Flathub)

Pontos confirmados na documentação do Tauri:
- O Flatpak **não é um alvo de build do Tauri**: gera-se o `.deb` primeiro e empacota-se com `flatpak-builder` a partir de um manifesto.
- O runtime **GNOME** já traz as dependências do Tauri/WebKitGTK nas versões corretas.
- O build do Flathub é **offline**: as fontes de Cargo e Node precisam ser geradas com as ferramentas do `flatpak-builder-tools` (`cargo-sources.json`, `node-sources.json`). **Isso é mais um motivo para não usar Python.**
- Exige arquivos AppStream (`metainfo.xml`) e `.desktop`.

Pontos que precisam de teste **[spike]**:

| Tema | Implicação |
|---|---|
| **Arquivos** | Usar o portal de arquivos (seletor do sistema); evitar `--filesystem=home` sem necessidade |
| **Ollama/servidor local** | Falar com um servidor no host exige rede local; alternativa: empacotar o `llama-server` no Flatpak ou como extensão |
| **GPU** | Aceleração (NVIDIA/AMD/Vulkan) depende de extensões; testar cedo; CPU sempre funciona |
| **Pacotes opcionais** | Pandoc, Tesseract e fontes como **extensões do Flatpak** (a confirmar) para manter o app base leve |
| **Cofre de chaves** | Acesso ao Secret Service dentro do sandbox |
| **Armazenamento de modelos** | Pasta de dados do app, com opção de apontar para outro disco com permissão explícita |
| **Fontes** | Confirmar acesso às fontes do sistema para idiomas de destino |

### 17.2 Windows

- Instalador gerado pelo Tauri; dependência do WebView2 (presente no Windows 10/11 atualizados).
- **Assinatura de código:** sem certificado, o SmartScreen mostra avisos; avaliar opções para projetos open source **[verificar]** ou documentar o aviso.
- GPU: CUDA/Vulkan variam por driver; manter CPU como padrão seguro.
- Caminhos longos e Unicode em nomes de arquivo.

### 17.3 CI de plataforma

Matriz com Windows 10/11 e pelo menos duas distros Linux com Flatpak (por exemplo Fedora e Ubuntu).

---

## 18. Roadmap

### Fase 0 — Fundação (2–3 semanas)
- Repositório, workspace, CI (lint, testes, deny/audit), ADRs 0001–0010.
- **[spike]** "Hello World" Tauri empacotado como **Flatpak e instalador Windows**, rodando no CI.
- **[spike]** Patch no lugar em EPUB e DOCX; leitura de texto com PDFium; `ocrs` em imagens de amostra; comparação com reaproveitar o TBL.
- `pgl-domain`, `pgl-testkit` (com `FakeProvider`) e `test-corpus` inicial.
- **Aceite:** pipeline "importar → traduzir com FakeProvider → exportar" funcionando via CLI em EPUB e TXT.

### Fase 1 — MVP (formatos de texto e e-book, modelo local)
- Formatos: **EPUB, HTML, TXT, Markdown, SRT/VTT** (leitura e escrita), incluindo **modo bilíngue**.
- Provedor OpenAI-compatível (cobre llama-server, LM Studio e APIs) + Ollama.
- Proteção de marcação, segmentação com contexto, glossário básico, QA mínimo, checkpoint e retomada.
- UI: importar → configurar → estimativa → progresso → exportar; assistente de primeiro uso; CLI equivalente.
- **Aceite:** traduzir um EPUB de 300+ páginas; passar no EPUBCheck; retomar após interrupção; ≥ 99% das unidades do corpus com marcação inline preservada; RAM estável.

### Fase 2 — Escritório, qualidade e revisão
- **DOCX, PPTX, XLSX, ODT/ODP/ODS, FB2, PO/XLIFF/TMX** pelo motor de patch.
- Tela de revisão lado a lado, memória de tradução, guia de estilo, resumo rolante, passe de polimento.
- Gerenciador de modelos + detecção de hardware; provedores de API com chave no cofre.
- OCR de imagens com legenda editável (`ocrs`).

### Fase 3 — PDF e cauda longa
- **PDF com texto:** reflow para EPUB/DOCX/HTML; depois overlay.
- Pacote de formatos (Pandoc) para MOBI/AZW3/RTF/DOC/LaTeX etc.
- Pacote de OCR (Tesseract/ONNX) para outros alfabetos.
- Motor embutido (llama.cpp) para "funciona sem instalar nada"; roteamento por idioma com MT dedicada.

### Fase 4 — Avançado
- PDF escaneado, PDF com re-diagramação por análise de layout, CBZ/CBR, inpainting de texto em imagens.
- Sistema de plugins para a comunidade (por exemplo adaptadores de formato isolados) **[avaliar]**.

---

## 19. Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Escopo de formatos explode | Projeto nunca fica pronto | Fases claras; **um motor de patch** cobre vários formatos; PDF e cauda longa só na Fase 3+ |
| PDF "preservando layout" | Fidelidade ruim e muito esforço | Entregar reflow primeiro; overlay depois; avisar fidelidade na UI |
| LLM quebra marcação | Arquivo corrompido | Placeholders + validação + reprocessamento + testes de propriedades |
| Qualidade ruim em idiomas raros | Usuário frustrado | Aviso por idioma, roteamento para MT dedicada, QA, revisão |
| Alucinação/repetição em modelos pequenos | Texto errado | QA automático, retradução, sugerir modelo maior |
| Hardware fraco | Livros demoram | Estimativa prévia, perfis, modelos leves, pausar/retomar |
| Dependências C++ (PDFium, llama.cpp) no Flatpak/Windows | Build frágil | Fixar versões; isolar em crates adaptadores; motor em processo separado |
| `llama-cpp-2` sem semver | Quebras em atualização | Fixar versão; porta `TranslationProvider` isola o impacto |
| OCR `ocrs` só latino e em preview | Cobertura limitada | Pacote de OCR (Tesseract/ONNX) para outros alfabetos |
| Licenças copyleft | Conflito de licença | Auditoria; evitar MuPDF/PyMuPDF; `cargo-deny` no CI |
| Direitos autorais / DRM | Risco legal | Não remover DRM; aviso e responsabilidade do usuário |
| Arquivos maliciosos | Segurança | Limites de descompressão, anti *zip slip*, XML seguro, fuzzing |
| Poucos mantenedores | Projeto estagna | Arquitetura modular, bom CONTRIBUTING, *good first issue*, ADRs |

---

## 20. Gratuito e open source de forma sustentável

- **Licença clara** desde o dia 1 (ADR-0001).
- **Sem servidor obrigatório:** tudo roda local; custo de infraestrutura ≈ zero (repositório, CI gratuito para open source, site estático).
- **Sem taxa e sem revenda de API:** o app só aceita a chave do usuário.
- **Sem telemetria por padrão.**
- **Sustentabilidade voluntária:** doações opcionais (GitHub Sponsors/Open Collective), sem bloquear recursos.
- **Comunidade:** documentação, guia de contribuição, código de conduta, issues rotuladas, traduções da interface por voluntários, guias "como adicionar um formato/provedor".
- **Distribuição:** Flathub e releases no GitHub com checksums.

---

## 21. Decisões em aberto

1. Licença do projeto (depende da auditoria de dependências).
2. Construir do zero ou contribuir com o TBL (após o spike).
3. Front-end: Svelte, Solid ou React (ou UI 100% Rust).
4. Motor embutido (llama.cpp) já no MVP ou só na Fase 3?
5. Patch direto no XML do DOCX ou `docx-rs` para escrita?
6. PDF: até onde ir na Fase 3 (apenas reflow ou também overlay)?
7. Pacotes opcionais no Flatpak: como distribuir (extensões ou download dentro do app)?
8. Quais idiomas serão "oficialmente suportados" no lançamento, com nível de qualidade declarado?
9. Meta de qualidade de tradução aceitável para o MVP e como medi-la.
10. Estratégia de assinatura de código no Windows.

---

## Apêndice A — Prompt revisado (v3) para pedir o plano a um modelo

````text
Atue como arquiteto de software sênior em Rust e líder técnico de um projeto open source.

PROJETO: Polyglottus — aplicativo desktop (Windows e Linux via Flatpak), gratuito e open source, que traduz livros e documentos completos entre idiomas, no maior número possível de formatos (PDF, EPUB, DOCX e muitos outros), preservando estrutura, imagens e o máximo possível do layout.

PRINCÍPIOS (inegociáveis)
- Gratuito, open source, sem cobrança, sem conta, sem telemetria obrigatória.
- Local-first: modelos locais como caminho principal; API de terceiros opcional, com chave do usuário; avisar quando dados saem da máquina.
- Rust-first: priorize Rust e admita onde Rust sozinho não basta, isolando o resto atrás de interfaces (traits).
- Leve de rodar: instalador pequeno, pouca RAM, streaming; sem Python e sem Electron no núcleo.
- Código limpo e fácil de manter: arquitetura hexagonal, crates pequenos, testes sem modelo (provedor falso), CI rigoroso.

RESTRIÇÕES
- Plataformas: Windows e Linux em Flatpak (considere sandbox, portais de arquivos, GPU, acesso a servidor local de modelos e build offline do Flathub).
- Orçamento zero; equipe pequena (declare suas suposições).
- Não remover DRM.

ANTES DE PLANEJAR
1. Compare projetos existentes (TranslateBooksWithLLMs, Tolmach, epub-tr, bilingual_book_maker, plugin Ebook Translator do Calibre, BreviaBook, PDFMathTranslate/BabelDOC, comic-translate, manga-image-translator, Koharu) e diga o que reutilizar, o que contribuir upstream e o que justifica construir.
2. Separe: fato conhecido, suposição sua e o que precisa ser verificado.

REQUISITOS
- Formatos: EPUB, HTML, TXT, Markdown, legendas, DOCX, PPTX, XLSX, ODF, FB2, PDF (com e sem texto), formatos de localização (PO, XLIFF, TMX) e, via conversores opcionais, MOBI, AZW3, RTF, DOC, LaTeX, DJVU, CBZ.
- Modelo de dados baseado em unidades de tradução com "patch no lugar" para preservar fidelidade; proteção de marcação inline com placeholders validados.
- Pipeline com checkpoint por unidade, retomada, memória de tradução, glossário, guia de estilo, QA automático e revisão manual lado a lado.
- OCR de imagens com legenda traduzida editável; sem apagar o texto original no MVP.
- Gerenciador de modelos com detecção de hardware, estimativa de tempo/custo e aviso de qualidade por idioma.
- UI leve, acessível e internacionalizada (pt-BR, en, es), simples para não técnicos; CLI equivalente.

ENTREGÁVEIS (Markdown, em português, com tabelas)
1. Análise competitiva e decisão build vs. contribute
2. Arquitetura hexagonal com portas (traits) e diagrama
3. Onde Rust basta e onde exige bibliotecas nativas ou binários externos
4. Estrutura do repositório (workspace Rust + front-end + packaging) e convenções
5. Matriz de formatos: camada, fidelidade, fase
6. Estratégia para PDF (reflow, overlay, re-diagramação)
7. Inferência local e API opcional; perfis de hardware
8. Qualidade: QA, testes (incluindo propriedades, snapshots, fuzzing), CI/CD
9. UX: telas, fluxo, acessibilidade, i18n
10. Empacotamento Windows e Flatpak
11. Segurança, privacidade e licenças das dependências
12. Roadmap com critérios de aceitação, MVP e riscos
13. Backlog inicial (épicos e tarefas) e decisões em aberto

Seja pragmático e realista: diga o que é viável hoje, o que exige muito esforço e o que fica para depois.
````
