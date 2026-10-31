# Polyglottus — Documento do Projeto (v2)

> Tradutor de livros completos, gratuito, open source e *local-first*.
> Documento atualizado em 30/10/2026 (v2: nome Polyglottus; plataformas Windows e Linux/Flatpak; macOS fora do escopo).
> Status: rascunho para discussão. Itens marcados com **[verificar]** dependem de checagem antes de virar decisão.

---

## 1. Resumo executivo

O Polyglottus é uma aplicação desktop (Windows e Linux via Flatpak) que traduz livros inteiros entre quaisquer idiomas suportados pelos modelos escolhidos, preservando estrutura (capítulos, listas, títulos, parágrafos), imagens e, na medida do possível, o layout.

Princípios inegociáveis:

1. **Gratuito e open source**, sem taxa, sem conta, sem telemetria obrigatória.
2. **Modelo local é o caminho principal.** API de terceiros é opcional, com chave do próprio usuário.
3. **Os dados do livro nunca saem da máquina** a menos que o usuário escolha uma API externa (e o app avisa isso claramente).
4. **Honestidade sobre fidelidade:** EPUB/DOCX/HTML (texto reflowable) e PDF (layout fixo) são problemas diferentes e terão níveis de fidelidade diferentes.

---

## 2. Panorama competitivo (pesquisa)

Já existe bastante coisa. O projeto **não** é pioneiro em "traduzir livro com LLM local" — o espaço está ocupado. O diferencial precisa estar em outro lugar (ver seção 3).

| Projeto | Foco | Formatos | Local/API | Destaques | Limitações observadas |
|---|---|---|---|---|---|
| **TranslateBooksWithLLMs (TBL)** | Livros, legendas, documentos | EPUB, DOCX, TXT, SRT | Ollama, llama.cpp/LM Studio/vLLM (API compatível OpenAI), OpenRouter, OpenAI, Gemini e outros | App desktop (Windows/macOS), web UI e CLI; checkpoints para retomar; sem limite de tamanho; AGPL-3.0 | Sem OCR de imagens e sem PDF nos formatos listados; backend em Python **[verificar]** |
| **Tolmach (KazKozDev/book-translator)** | Tradução literária | TXT, EPUB, PDF, DOCX | Ollama local | Glossário por documento gerado ao "preparar" o livro; mesa de revisão lado a lado (original / rascunho / final editável); refinamento guardado | Interface web local (Flask); exige vários modelos locais pesados para o setup mínimo |
| **epub-tr** | EPUB literário (turco primeiro) | EPUB | Ollama, LLMs gratuitos via CLI, MT (Google/Bing) | Glossário próprio, passe de polimento, edição bilíngue, validação com EPUBCheck, cache | Só EPUB; CLI |
| **bilingual_book_maker** | Edições bilíngues | EPUB, TXT, SRT | Principalmente APIs | MIT; gera EPUB bilíngue | CLI, foco em API |
| **Plugin Ebook Translator (Calibre)** | Tradução dentro do Calibre | Todos os formatos do Calibre | Muitos motores (Google, DeepL, LLMs) | Aproveita a conversão do Calibre; modo bilíngue; GPL-3.0 | Depende do Calibre; pode ser lento e sofrer com limites de requisição |
| **BreviaBook** | Condensar/traduzir | EPUB, PDF → EPUB/PDF/MD | Ollama + APIs | Representação intermediária (IR), glossário por capítulo, `--dry-run` com estimativa de tokens/custo, `--resume` | CLI; foco em resumo |
| **PDFMathTranslate / BabelDOC** | PDF científico com layout preservado | PDF | Vários, incl. Ollama | BabelDOC separa layout e semântica numa **IR bidirecional**, com glossário, contexto entre páginas e re-diagramação adaptativa | Foco em artigos científicos, não livros; PDFMathTranslate lista problemas pendentes (listas, rotação de página) |
| **comic-translate / manga-image-translator / Koharu** | Texto em imagens de quadrinhos | Imagens | OCR + MT/LLM, alguns offline | Pipeline detectar → OCR → remover texto (inpainting) → traduzir → reescrever na imagem; Koharu é escrito em Rust | Voltados a quadrinhos, não a livros com figuras/diagramas |

**Fontes consultadas** (busca de 30/10/2026): repositórios GitHub dos projetos acima, PyPI (breviabook, epub-tr, bbook-maker), artigo ACL do BabelDOC, relatório técnico do TranslateGemma, guias de modelos locais para tradução.

---

## 3. Lacunas e diferencial possível

Nenhum projeto encontrado combina **tudo** isto:

- App desktop com **instalador de verdade** (sem exigir Python, terminal ou navegador manual).
- **Vários formatos** de entrada/saída (EPUB, DOCX, HTML, TXT e PDF) numa só ferramenta.
- **OCR de texto dentro de imagens do livro** com legenda traduzida (os projetos de OCR são de quadrinhos; os de livro ignoram imagens).
- **Revisão manual** integrada (só o Tolmach tem algo parecido).
- **Gerenciador de modelos** com detecção de hardware e recomendação ("seu PC roda este modelo").
- **Estimativa de tempo/custo antes de começar** e retomada automática.

**Recomendação estratégica:** antes de escrever código, decidir entre (a) construir do zero, (b) contribuir/forkar um projeto existente (TBL é AGPL e já faz boa parte do núcleo), ou (c) construir só as peças ausentes (OCR de imagens, PDF, revisão, instalador) reutilizando ideias. O caminho (a) é legítimo, mas o plano deve justificá-lo.

---

## 4. Melhorias sugeridas ao prompt original

O prompt original é bom como lista de desejos, mas tem pontos que fazem a resposta do modelo sair genérica ou otimista demais.

### 4.1 O que acrescentar

1. **Níveis de fidelidade por formato.** "Preservar layout" significa coisas diferentes em EPUB (reflowable, HTML/CSS), DOCX (estilos) e PDF (posições absolutas). Pedir uma tabela formato × o que é preservado × dificuldade.
2. **Representação Intermediária (IR).** Exigir que o plano separe *estrutura/layout* de *conteúdo textual*, como faz o BabelDOC. É a decisão arquitetural mais importante.
3. **Proteção de marcação inline.** Itálico, negrito, links, notas de rodapé e âncoras quebram fácil quando o LLM "reescreve" o HTML. Pedir estratégia de *placeholders* e validação de que todas as tags voltaram.
4. **Consistência em livros longos.** Glossário (nomes, termos), resumo rolante de capítulos, janela de contexto com sobreposição, "bíblia de estilo" por livro (tom, tratamento formal/informal, travessão vs. aspas em diálogos).
5. **Controle de qualidade automático.** Detectar trechos não traduzidos, repetições/alucinações, saída em idioma errado, razão de tamanho anormal entre original e tradução, perda de tags. Reprocessar só o trecho falho.
6. **Retomada e idempotência.** Checkpoint por bloco; reabrir após falha ou fechamento sem perder trabalho.
7. **Hardware e modelos.** Pedir requisitos mínimos, detecção de GPU/RAM, modelos recomendados por faixa de hardware, quantização, e estimativa de tempo por livro.
8. **Honestidade sobre "qualquer idioma".** A qualidade varia muito por par de idiomas. Pedir estratégia: LLM tradutor para idiomas principais, modelos dedicados (NLLB/MADLAD) para idiomas de poucos recursos, e aviso de qualidade na interface.
9. **Segurança e privacidade.** Chaves de API no cofre do sistema (Credential Manager no Windows; Secret Service/libsecret via portal no Linux/Flatpak), sem telemetria, sanitização de EPUB/HTML (arquivo é um zip com HTML e scripts), proteção contra zip bomb.
10. **Questões legais.** Não remover DRM; responsabilidade do usuário sobre direitos autorais; licenças das dependências (ver seção 6).
11. **Licença e governança.** Escolher licença cedo (MIT/Apache vs. GPL/AGPL) e documentar o porquê.
12. **Testes e métricas.** Corpus de livros de teste (domínio público, de vários formatos e idiomas), validação de EPUB (EPUBCheck), regressão visual, métricas de tradução (ex.: chrF/COMET) e revisão humana amostral.
13. **Internacionalização da própria interface** (pt-BR, en, es no mínimo) e acessibilidade.
14. **Saídas de modo bilíngue** (original + tradução lado a lado/alternado), muito pedido pelos usuários e comum nos concorrentes.
15. **Restrições de execução do plano:** tamanho da equipe, prazo, orçamento zero, e pedir que o modelo declare suas **suposições** e separe o que é *fato conhecido* do que é *palpite*.

### 4.2 O que ajustar

- **Corrigir "independemente"** → "independentemente".
- **Estrutura de pastas:** mistura convenções de web (`routes/`, `pages/`) com um app Tauri/Rust. Sugerir um *workspace* Rust com crates por módulo e um front-end separado (ver seção 5).
- **Stack em três linguagens** (Rust + TypeScript + Python) aumenta muito o custo de empacotamento. Pedir que o plano discuta alternativas e justifique cada linguagem (ver seção 6).
- **"Preservar o layout do livro"** como requisito único é irreal para PDF. Separar em "obrigatório no MVP" (EPUB/DOCX/TXT/HTML) e "melhor esforço" (PDF).
- **OCR + legenda:** deixar explícito que o MVP *não* apaga o texto original da imagem (inpainting). Isso é fase futura e já existe em projetos de quadrinhos.
- **Remover redundâncias** (requisitos repetidos em três seções) para o modelo não gastar espaço de resposta com isso.
- **Pedir formato de saída:** tabelas de decisão, diagramas em texto, critérios de aceitação por fase e um backlog inicial.

### 4.3 Prompt revisado (v2)

````text
Atue como arquiteto de software sênior e líder técnico de um projeto open source.

CONTEXTO
Quero planejar o Polyglottus: um aplicativo desktop (Windows e Linux via Flatpak), gratuito e open source, que traduz livros completos entre idiomas, preservando estrutura, imagens e o máximo possível do layout.

PRINCÍPIOS (inegociáveis)
- Gratuito, open source, sem cobrança, sem conta e sem telemetria obrigatória.
- Modelos locais são o caminho principal. API de terceiros é opcional, com chave do próprio usuário, e o app deixa claro quando dados saem da máquina.
- O app nunca deve depender de serviço pago para funcionar.

RESTRIÇÕES
- Orçamento zero; equipe pequena (informe suas suposições sobre tamanho de equipe e prazo).
- Plataformas: Windows e Linux distribuído como Flatpak (considere sandbox, portais de arquivos, acesso a GPU e ao Ollama dentro do Flatpak, e build offline do Flathub).
- Hardware do usuário varia de notebook sem GPU a PC com GPU dedicada.
- Não remover DRM. Direitos autorais são responsabilidade do usuário.

ANTES DE PLANEJAR
1. Considere projetos existentes (TranslateBooksWithLLMs, Tolmach, epub-tr, bilingual_book_maker, plugin Ebook Translator do Calibre, BreviaBook, PDFMathTranslate/BabelDOC, comic-translate, manga-image-translator). Compare-os e diga o que vale reutilizar, o que vale contribuir upstream e o que justifica construir do zero.
2. Separe claramente: o que é fato conhecido, o que é suposição sua e o que precisa ser verificado.

REQUISITOS FUNCIONAIS
- Entrada: EPUB, DOCX, HTML, TXT e PDF. Saída: EPUB, DOCX, HTML, TXT, modo bilíngue e, se viável, PDF.
- Livros longos sem estourar memória; processamento por blocos com checkpoint e retomada.
- Preservação de capítulos, títulos, listas, parágrafos, indentação, notas de rodapé, links e formatação inline.
- Imagens copiadas sem deformação e na posição original. Se houver texto na imagem: OCR, tradução e legenda traduzida abaixo (sem apagar o texto original no MVP). Legenda editável.
- Consistência: glossário por livro, resumo rolante de capítulos, guia de estilo (formalidade, diálogos), contexto com sobreposição.
- Controle de qualidade automático: trechos não traduzidos, repetições/alucinações, idioma errado, razão de tamanho anormal, perda de tags; reprocessar só o trecho falho.
- Revisão manual lado a lado (original / tradução), com edição e reprocessamento de trechos.
- Progresso em tempo real, estimativa de tempo antes de iniciar, salvamento automático.
- Gerenciador de modelos: detectar hardware, recomendar modelos, baixar/remover modelos, escolher local ou API.
- Interface em pt-BR, en e es, simples para não técnicos.

ARQUITETURA E STACK (discuta alternativas e justifique)
- Proponho: Rust (núcleo), Tauri + React + TypeScript (UI), Python (OCR/modelos) como processo auxiliar (sidecar), SQLite, Ollama/llama.cpp. Critique essa escolha: o custo de empacotar três linguagens compensa? Existe uma stack mais simples para o MVP?
- Proponha uma Representação Intermediária (IR) que separe estrutura/layout de conteúdo textual.
- Estratégia para proteger marcação inline (placeholders) e validar sua restauração.
- Níveis de fidelidade por formato, em tabela: formato × o que é preservado × dificuldade × entra no MVP?
- Idiomas: estratégia para "qualquer idioma" reconhecendo que a qualidade varia (LLMs tradutores para idiomas principais; modelos dedicados para idiomas de poucos recursos; aviso de qualidade na UI).
- Licenças: avalie as licenças das dependências principais e recomende a licença do projeto.
- Segurança e privacidade: chaves no cofre do sistema, sanitização de EPUB/HTML, proteção contra arquivos maliciosos.

ENTREGÁVEIS (formato: Markdown, em português, com tabelas onde ajudar)
1. Resumo executivo e análise competitiva (build vs. contribute vs. fork)
2. Visão geral da arquitetura e diagrama em texto
3. Stack recomendada com alternativas e trade-offs
4. Estrutura do repositório (workspace Rust, front-end, sidecar)
5. Pipeline de processamento, do arquivo ao exportado
6. Modelos locais e API opcional (requisitos de hardware, modelos por faixa, tempo estimado por livro)
7. Preservação de layout por formato
8. OCR e legendas de imagens
9. Qualidade: QA automático, métricas e corpus de testes
10. UX: telas, fluxo, estados de erro e fluxo de revisão
11. Roadmap em fases com critérios de aceitação
12. MVP: o que entra, o que fica de fora e por quê
13. Riscos técnicos e legais, com mitigação
14. Sustentabilidade do projeto gratuito (comunidade, doações voluntárias, governança)
15. Backlog inicial (épicos e tarefas) e decisões em aberto

Seja pragmático e realista. Diga o que é viável hoje, o que exige muito esforço e o que deve ficar para depois.
````

---

## 5. Arquitetura recomendada

### 5.1 Ideia central: Representação Intermediária (IR)

```
Arquivo (EPUB/DOCX/PDF/HTML/TXT)
        │  parser específico por formato
        ▼
   ┌─────────────────────────────────────┐
   │ IR: árvore de blocos                │
   │  - estrutura (capítulo, título,     │
   │    lista, tabela, imagem, nota)     │
   │  - estilo/layout (referência ao     │
   │    original, não traduzido)         │
   │  - texto traduzível com placeholders│
   └─────────────────────────────────────┘
        │  segmentador → blocos com contexto
        ▼
   Tradutor (local ou API) + glossário + QA
        │  texto traduzido volta para a IR
        ▼
   Exportador específico por formato → arquivo final
```

Por que isso importa: a tradução toca **só o texto**; a estrutura fica intacta e é re-aplicada na exportação. É o mesmo princípio do BabelDOC e do BreviaBook.

### 5.2 Módulos

| Módulo | Responsabilidade |
|---|---|
| `parsers` | Ler cada formato e produzir a IR |
| `segmenter` | Dividir em blocos com tamanho adequado ao modelo, mantendo limites naturais (parágrafo/capítulo) |
| `protector` | Trocar tags inline por placeholders e restaurar depois, validando |
| `glossary` | Extrair termos/nomes, manter glossário editável |
| `translator` | Interface única para provedores (Ollama, llama.cpp, API compatível OpenAI, etc.) |
| `qa` | Verificações automáticas e fila de reprocessamento |
| `ocr` | Detectar texto em imagens e produzir legenda |
| `exporter` | Reconstruir cada formato a partir da IR |
| `jobs` | Fila, checkpoints, retomada, progresso |
| `storage` | SQLite: projetos, blocos, glossários, histórico |
| `model-manager` | Detectar hardware, baixar/remover modelos, recomendar |

### 5.3 Estrutura de repositório sugerida

```
polyglottus/
├─ crates/                    # workspace Rust
│  ├─ pgl-ir/
│  ├─ pgl-parsers/
│  ├─ pgl-segmenter/
│  ├─ pgl-protector/
│  ├─ pgl-translator/
│  ├─ pgl-qa/
│  ├─ pgl-exporters/
│  ├─ pgl-jobs/
│  └─ pgl-storage/
├─ src-tauri/                 # shell Tauri (comandos, eventos, sidecar)
├─ ui/                        # React + TypeScript
│  ├─ screens/ (Import, Setup, Progress, Review, Export, Models)
│  └─ i18n/
├─ sidecar-python/            # OCR, imagens, utilitários de modelo
├─ test-corpus/               # livros de domínio público + casos difíceis
├─ docs/
└─ LICENSE
```

---

## 5.4 Plataformas: Windows e Linux (Flatpak)

Escopo atual: **Windows** e **Linux distribuído como Flatpak**. macOS está fora do escopo (o que também elimina a assinatura/notarização da Apple).

### Flatpak — pontos de atenção **[verificar cada um antes de decidir]**

| Tema | Implicação para o Polyglottus |
|---|---|
| **Sandbox e arquivos** | Acesso a livros deve usar o *portal de arquivos* (seletor de arquivos do sistema), não caminhos livres. Evitar pedir `--filesystem=home` sem necessidade. |
| **Comunicação com Ollama/llama.cpp** | O Flatpak roda isolado: falar com um Ollama instalado no host exige acesso à rede local (`localhost:11434`) e pode exigir permissões extras. Alternativa: empacotar o `llama.cpp` dentro do próprio Flatpak (ou como extensão), para funcionar "de fábrica". |
| **GPU** | Aceleração por GPU dentro do Flatpak depende de extensões/runtimes (NVIDIA, AMD/Vulkan, ROCm). Testar cedo; modo CPU deve sempre funcionar. |
| **Armazenamento de modelos** | Modelos são grandes (GBs). Definir pasta de dados do app (`~/.var/app/...`) e permitir apontar para outra pasta/disco com permissão explícita. |
| **Cofre de chaves** | Usar Secret Service/portal para guardar chaves de API do usuário. |
| **Build sem internet** | O Flathub exige build reprodutível e offline: dependências Rust (crates), npm e Python precisam ser listadas/vendorizadas nos manifestos. Isso torna o **sidecar Python mais trabalhoso** de empacotar. |
| **Regras do Flathub** | Exige licença open source e metadados do app (AppStream). Compatível com o princípio do projeto, mas é preciso cumprir as diretrizes de submissão. |
| **WebView do Tauri** | No Linux o Tauri usa WebKitGTK; usar o runtime GNOME/Freedesktop adequado e testar a renderização. |

### Windows — pontos de atenção

- Instalador (MSI/NSIS via Tauri) e **assinatura de código**: sem certificado, o SmartScreen exibe avisos. Avaliar certificado gratuito para open source ou documentar o aviso **[verificar opções]**.
- Dependência do WebView2 (já incluso no Windows 10/11 atualizado).
- Drivers de GPU (CUDA/Vulkan) variam; manter modo CPU como padrão seguro.
- Caminhos longos e caracteres Unicode em nomes de arquivos de livros.

### Efeito no roadmap

- **Fase 0:** criar um "Hello World" Tauri empacotado como Flatpak **e** como instalador Windows, rodando o CI para os dois. Validar o ciclo de empacotamento antes de investir no resto.
- **Decisão reforçada:** como o empacotamento offline do Flathub encarece o Python, a recomendação de **MVP sem Python** (OCR via Tesseract ou modelo de visão) fica ainda mais forte.
- **Testes:** matriz de CI com Windows 10/11 e pelo menos duas distros Linux com Flatpak (ex.: Fedora e Ubuntu).

---

## 6. Stack e licenças

### 6.1 Escolhas e alternativas

| Camada | Proposta original | Observação / alternativa |
|---|---|---|
| Núcleo | Rust | Bom para desempenho e streaming de arquivos grandes. Exige equipe confortável com Rust. |
| UI | Tauri + React + TS | Instaladores leves. Boa escolha para desktop multiplataforma. |
| OCR/imagens | Python | **Custo de empacotamento alto** (PyInstaller, tamanhos de instalador, bibliotecas nativas). Alternativas: Tesseract via binding Rust, ou OCR por modelo de visão servido pelo mesmo Ollama/llama.cpp. Considerar Python só como sidecar opcional "pacote de OCR avançado". |
| Banco | SQLite | Adequado. |
| Modelos | Ollama / llama.cpp | Suportar também qualquer servidor com API compatível com OpenAI (LM Studio, vLLM). Isso cobre quase tudo com **uma** interface. |

**Pergunta de arquitetura para o MVP:** vale começar com Tauri + núcleo Rust + **sem Python** (OCR via Tesseract ou modelo de visão) e adicionar o sidecar Python depois? Reduz muito o risco de empacotamento.

### 6.2 Licenças — ponto de atenção

- O projeto TBL usa **AGPL-3.0**; o plugin do Calibre e o manga-translator-ui usam **GPL**; bilingual_book_maker usa **MIT**.
- Bibliotecas populares têm licenças copyleft fortes. Exemplo: o **PyMuPDF** é distribuído sob AGPL (ou licença comercial) **[verificar versão atual]**. Usá-lo obriga o projeto a ser compatível com AGPL.
- Modelos têm licenças próprias (pesos do Gemma têm termos de uso específicos; NLLB tem licença não comercial **[verificar]**). Como o app **não distribui** modelos (o usuário baixa), o impacto é menor, mas a UI deve mostrar a licença de cada modelo antes do download.
- **Ação:** montar uma tabela de licenças de todas as dependências antes de escolher a licença do projeto.

---

## 7. Modelos locais

### 7.1 Duas famílias

| Família | Exemplos | Pontos fortes | Pontos fracos |
|---|---|---|---|
| **LLMs gerais ou ajustados para tradução** | TranslateGemma (4B, 12B, 27B; 55 idiomas), Qwen, Gemma, Mistral | Seguem instruções (glossário, estilo, formato), melhor fluência literária | Qualidade cai em idiomas de poucos recursos; podem alucinar/repetir |
| **Modelos dedicados de tradução** | NLLB-200, MADLAD-400, Opus-MT | Cobertura ampla (200+ e 400+ idiomas), rápidos, estáveis | Não seguem instruções complexas; menos fluência literária |

Notas da pesquisa:
- O TranslateGemma foi lançado em tamanhos 4B, 12B e 27B para 55 idiomas; segundo o relatório técnico, o 12B superou o Gemma 3 27B base em métricas automáticas, e o 12B foi pensado para rodar em notebooks comuns.
- Avaliações da comunidade e de linguistas relatam que idiomas diferem muito: alguns saem quase perfeitos, outros apresentam inserções e repetições. **Isso reforça a necessidade de QA automático e aviso de qualidade por idioma.**
- Guias comparativos indicam que, para idiomas de poucos recursos, modelos dedicados (como NLLB) tendem a superar LLMs gerais; para literatura e tom, serviços de nuvem como DeepL ainda costumam vencer modelos locais.

### 7.2 Estratégia sugerida

1. **Perfis de hardware** detectados no primeiro uso (sem GPU / GPU 6–8 GB / GPU 12+ GB) com modelo recomendado e tempo estimado.
2. **Roteamento por idioma:** par comum → LLM tradutor; idioma raro → NLLB/MADLAD; com aviso na UI.
3. **Interface única de provedor:** Ollama, servidor compatível OpenAI e, opcionalmente, API do usuário.
4. **Modo "dry-run":** estimar tokens, tempo (e custo, se API) antes de começar.
5. **Aviso de privacidade** ao selecionar API: "o texto do livro será enviado ao provedor X".

---

## 8. Preservação de layout

### 8.1 Tabela de fidelidade por formato

| Formato | Natureza | O que dá para preservar | Dificuldade | MVP? |
|---|---|---|---|---|
| **EPUB** | HTML + CSS reflowable | Quase tudo: estrutura, estilos, imagens, índice, metadados, notas | Média (tags inline, índice/NCX, direção do texto) | **Sim** |
| **TXT / Markdown** | Texto puro | Parágrafos, linhas em branco, indentação | Baixa | **Sim** |
| **HTML** | Marcação | Quase tudo | Média | **Sim** |
| **DOCX** | XML com estilos | Estilos, listas, imagens, tabelas; fragmentação em "runs" é o problema | Média-alta | Fase 2 |
| **PDF (com texto)** | Layout fixo | Possível com detecção de layout e re-diagramação, mas texto traduzido muda de tamanho | **Alta** | Fase 3 (melhor esforço) |
| **PDF (escaneado)** | Imagens | Exige OCR de página inteira | Muito alta | Fase 4 |

### 8.2 Problemas recorrentes (devem ter solução desenhada)

- **Marcação inline:** proteger com placeholders e validar contagem/ordem após a tradução.
- **Expansão/contração do texto:** idiomas diferentes ocupam espaços diferentes (importante em PDF/tabelas).
- **Direção de escrita (RTL):** árabe/hebraico exigem ajuste de CSS e de exportação.
- **Fontes:** o idioma de destino pode exigir fontes que o original não incorpora (CJK, árabe, indiano). Definir fonte-padrão embutida.
- **Índice e links internos:** reescrever âncoras e títulos do sumário.
- **Notas de rodapé:** manter vínculo número ↔ nota.
- **Idiomas de saída com pontuação diferente:** aspas, travessões e diálogos seguem convenções locais (guia de estilo por idioma).

---

## 9. OCR e imagens

### 9.1 Fluxo do MVP

1. Extrair imagens do livro e **copiá-las sem recompressão** (preservar bytes originais quando possível).
2. Heurística para decidir se vale rodar OCR (evitar OCR em fotos sem texto).
3. OCR com idioma detectado/configurado.
4. Traduzir o texto reconhecido (com contexto da legenda/parágrafo vizinho).
5. Inserir **legenda traduzida abaixo da imagem**, marcada como "gerada por OCR" e editável.
6. Se a confiança for baixa, sinalizar para revisão em vez de inserir lixo.

### 9.2 Opções de OCR

| Opção | Observação |
|---|---|
| Tesseract | Maduro, licença permissiva, ampla cobertura de idiomas; qualidade menor em layout complexo |
| PaddleOCR | Boa qualidade, usado pelos projetos de quadrinhos; dependência Python pesada |
| Modelo de visão (VLM) local | Evita sidecar Python; qualidade variável; mais lento. O TranslateGemma preserva capacidade multimodal do Gemma 3, mas isso deve ser testado para OCR **[verificar]** |

### 9.3 Fase futura: apagar e reescrever texto na imagem (inpainting)

Projetos como manga-image-translator, comic-translate e Koharu já fazem detectar → OCR → remover texto → traduzir → reescrever. Para livros técnicos com diagramas, isso é útil mas **não é MVP**. Considerar integrar ideias ou modelos desses projetos respeitando licenças.

---

## 10. Qualidade

### 10.1 QA automático por bloco

- Idioma de saída bate com o idioma de destino?
- Razão de tamanho original/tradução dentro da faixa esperada para o par de idiomas?
- Todos os placeholders voltaram, na ordem?
- Repetição anormal de n-gramas (sinal de alucinação em loop)?
- Termos do glossário respeitados?
- Números, datas e nomes próprios preservados?

Bloco reprovado → reprocessa (outro prompt, outro modelo ou tamanho menor) → se falhar, vai para a fila de revisão manual.

### 10.2 Consistência em livros longos

- **Glossário por livro**, gerado numa passada de "preparação" e editável (o Tolmach e o epub-tr usam essa ideia).
- **Resumo rolante** do capítulo anterior como contexto.
- **Guia de estilo** (formal/informal, regionalismo, tratamento de diálogos).
- **Sobreposição** de contexto entre blocos adjacentes.
- Passe opcional de **polimento** por capítulo.

### 10.3 Testes

- **Corpus de testes** em `test-corpus/`: livros de domínio público em EPUB/DOCX/PDF, vários idiomas e scripts, casos difíceis (poesia, tabelas, notas, RTL, imagens com texto).
- **EPUBCheck** em toda saída EPUB.
- **Regressão visual** para HTML/PDF.
- **Métricas automáticas** (chrF/COMET ou similares) com amostra de referência humana.
- **Teste de estresse:** livro de 1.000+ páginas sem estourar memória; interrupção e retomada.

---

## 11. Interface

### 11.1 Fluxo de telas

1. **Início / importar:** arrastar arquivo. Detecta formato e idioma. Mostra título, capítulos, nº de palavras, imagens.
2. **Configuração (simples por padrão, avançado opcional):** idioma de destino; escolha **Local (recomendado)** ou **API própria**; modelo recomendado para o hardware; guia de estilo (formalidade); glossário opcional.
3. **Estimativa:** tempo previsto, uso de disco/RAM, custo estimado se for API. Botão "Iniciar".
4. **Progresso:** barra geral, capítulo atual, velocidade, blocos com problema, pausar/retomar. Funciona em segundo plano.
5. **Revisão:** lado a lado (original / tradução), filtros "com alerta de QA", edição, "retraduzir este trecho", edição do glossário com opção de reaplicar.
6. **Exportação:** escolher formatos (EPUB, DOCX, HTML, TXT, bilíngue, PDF quando disponível), prévia e relatório de qualidade.
7. **Modelos:** lista de modelos instalados, recomendados, licenças, tamanho, botão baixar/remover.
8. **Configurações:** idioma da interface, chaves de API (cofre do sistema), pasta de dados, diagnósticos.

### 11.2 Princípios de UX

- **Padrão sensato:** o usuário não técnico só escolhe arquivo e idioma.
- **Nunca perder trabalho:** salvamento contínuo e retomada.
- **Transparência:** mostrar claramente quando algo está sendo enviado para a internet.
- **Erros acionáveis:** "este bloco falhou por X; tentar com outro modelo?".
- **Acessibilidade:** teclado, contraste, leitor de tela, escala de fonte.

---

## 12. Roadmap

### Fase 0 — Decisões (1–2 semanas)
- Decidir build vs. contribute vs. fork (seção 3).
- Tabela de licenças e escolha da licença do projeto.
- Definir IR (esquema) e formato do projeto no SQLite.
- Montar `test-corpus/` inicial.
- **Critério de aceitação:** documento de decisões e IR aprovados.

### Fase 1 — MVP (EPUB + TXT/HTML, modelo local)
- Parser/exportador EPUB e TXT/HTML via IR.
- Protetor de marcação inline.
- Segmentação, tradução por blocos via Ollama/API compatível OpenAI.
- Checkpoint e retomada; progresso em tempo real.
- Glossário básico; QA mínimo (idioma, placeholders, repetição).
- UI: importar → configurar → progresso → exportar.
- Imagens copiadas intactas (sem OCR ainda).
- **Critério de aceitação:** traduzir um EPUB de 300+ páginas, passar no EPUBCheck, retomar após interrupção, sem perda de formatação inline em ≥ 99% dos blocos do corpus.

### Fase 2 — Qualidade e revisão
- Tela de revisão lado a lado, retradução por trecho.
- Guia de estilo, resumo rolante, passe de polimento.
- Provedores de API opcionais (chave do usuário, cofre do sistema).
- Modo bilíngue.
- Gerenciador de modelos + detecção de hardware.
- OCR de imagens com legenda traduzida editável.
- **Critério:** revisão humana amostral dentro da meta definida; OCR com legenda em imagens do corpus.

### Fase 3 — Mais formatos
- DOCX (leitura e escrita).
- PDF com texto (melhor esforço, avaliar reutilizar abordagem do BabelDOC).
- Roteamento de modelos por idioma (LLM vs. NLLB/MADLAD).

### Fase 4 — Avançado
- PDF escaneado (OCR de página).
- Inpainting de texto em imagens.
- Plugins/provedores da comunidade.
- Tradução colaborativa (exportar/importar projetos de revisão).

---

## 13. Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Empacotar Rust + TS + Python (Windows e Flatpak) | Instaladores grandes/frágeis | Começar sem Python; sidecar opcional |
| Qualidade ruim em idiomas raros | Usuário frustrado | Roteamento por idioma, aviso na UI, QA, revisão |
| LLM quebra tags/HTML | Arquivo corrompido | Placeholders + validação + reprocessamento |
| Alucinação/repetição em modelos pequenos | Texto errado | QA automático, retradução, modelo maior sugerido |
| Hardware fraco | Livros levam dias | Estimativa prévia, perfis, modelos menores, pausar/retomar |
| PDF "preservando layout" | Escopo explode | Tratar como melhor esforço e fase tardia |
| Licenças copyleft de dependências | Conflito de licença | Auditoria antes de adotar bibliotecas |
| Direitos autorais / DRM | Risco legal | Não remover DRM; aviso e responsabilidade do usuário |
| Arquivos maliciosos | Segurança | Sanitizar HTML, limitar descompressão, sandbox do sidecar |
| Dependência de poucos mantenedores | Projeto estagna | Documentação, CI, issues "good first issue", governança aberta |

---

## 14. Como manter o projeto gratuito e open source

- **Licença clara** desde o dia 1 (MIT/Apache para máxima adoção; GPL/AGPL para impedir fechamento — decidir após a auditoria de dependências).
- **Sem servidor próprio obrigatório:** tudo roda local; custo de infraestrutura do projeto ≈ zero (repositório, CI gratuito para open source, site estático).
- **Sem taxa e sem revenda de API.** O app apenas aceita a chave do usuário.
- **Sem telemetria por padrão.** Se houver diagnóstico opcional, é opt-in e transparente.
- **Sustentabilidade voluntária:** doações opcionais (GitHub Sponsors/Open Collective) sem bloquear recursos.
- **Comunidade:** documentação, guia de contribuição, código de conduta, issues rotuladas, traduções da interface por voluntários.
- **Distribuição:** releases no GitHub com assinaturas/checksums; instalador para Windows e pacote Flatpak (Flathub).

---

## 15. Decisões em aberto

1. Construir do zero, contribuir com TBL ou forkar? (seção 3)
2. Licença do projeto (depende das dependências) (seção 6.2)
3. MVP com ou sem Python?
4. OCR: Tesseract, PaddleOCR ou modelo de visão?
5. Meta de qualidade aceitável para o MVP e como medi-la.
6. Quais idiomas são "oficialmente suportados" no lançamento (com nível de qualidade declarado)?
7. Modelo/hardware mínimo recomendado para a primeira versão.
8. PDF: tentar no MVP+1 ou apenas exportar PDF a partir de HTML/EPUB?

---

## Apêndice — Resumo do que mudou em relação ao prompt original

- Adicionada **análise competitiva** e discussão **build vs. contribute**.
- **IR** como decisão arquitetural central.
- **Níveis de fidelidade por formato** (PDF tratado como melhor esforço).
- **Proteção de marcação inline**, **QA automático** e **glossário/guia de estilo**.
- **Gerenciador de modelos** com detecção de hardware e estimativa prévia.
- **Honestidade sobre "qualquer idioma"** e roteamento por tipo de modelo.
- **Licenças, privacidade, segurança e questões legais** como requisitos.
- **MVP mais enxuto:** EPUB/TXT/HTML primeiro; DOCX, PDF e inpainting depois.
- Pedidos de **suposições explícitas**, **critérios de aceitação** e **backlog**.
