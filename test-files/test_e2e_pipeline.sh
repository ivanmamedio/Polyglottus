#!/bin/bash
# Test script for E2E pipeline: import → translate (FakeProvider) → export
# Usage: ./test_e2e_pipeline.sh

set -e

CLI_BIN="/home/carlos/Documentos/Polyglottus/target/release/pgl-cli"
TEST_FILES_DIR="/home/carlos/Documentos/Polyglottus/test-files"
OUTPUT_DIR="/home/carlos/Documentos/Polyglottus/test-output"

echo "=== Polyglottus E2E Pipeline Test ==="
echo "CLI: $CLI_BIN"
echo "Test files: $TEST_FILES_DIR"
echo "Output: $OUTPUT_DIR"
echo ""

mkdir -p "$OUTPUT_DIR"

# Check if CLI exists
if [ ! -f "$CLI_BIN" ]; then
    echo "ERRO: CLI não encontrado em $CLI_BIN"
    echo "Compile primeiro com: cargo build --release -p pgl-cli"
    exit 1
fi

echo "=== 1. Testando import EPUB ==="
PROJECT_OUTPUT=$("$CLI_BIN" import "$TEST_FILES_DIR/test_book.epub" --name "Teste EPUB" 2>&1)
echo "$PROJECT_OUTPUT"

# Extrair project ID (assumindo formato de saída)
PROJECT_ID=$(echo "$PROJECT_OUTPUT" | grep -oE '[a-f0-9-]{36}' | head -1)
if [ -z "$PROJECT_ID" ]; then
    echo "ERRO: Não foi possível extrair Project ID"
    exit 1
fi
echo "Project ID: $PROJECT_ID"
echo ""

echo "=== 2. Testando import DOCX ==="
PROJECT_OUTPUT2=$("$CLI_BIN" import "$TEST_FILES_DIR/test_document.docx" --name "Teste DOCX" 2>&1)
echo "$PROJECT_OUTPUT2"

PROJECT_ID2=$(echo "$PROJECT_OUTPUT2" | grep -oE '[a-f0-9-]{36}' | head -1)
if [ -z "$PROJECT_ID2" ]; then
    echo "ERRO: Não foi possível extrair Project ID 2"
    exit 1
fi
echo "Project ID 2: $PROJECT_ID2"
echo ""

echo "=== 3. Listando projetos ==="
"$CLI_BIN" list
echo ""

echo "=== 4. Info do projeto EPUB ==="
"$CLI_BIN" info "$PROJECT_ID"
echo ""

echo "=== 5. Testando tradução com FakeProvider (EPUB) ==="
"$CLI_BIN" translate \
    --project "$PROJECT_ID" \
    --provider fake \
    --target-lang pt-BR \
    --batch-size 5 \
    --max-parallel 2 \
    --output-format epub \
    --bilingual false

echo "Aguardando tradução concluir..."
sleep 5

# Verificar status
"$CLI_BIN" info "$PROJECT_ID"
echo ""

echo "=== 6. Exportando EPUB traduzido ==="
OUTPUT_EPUB="$OUTPUT_DIR/test_book_translated.epub"
"$CLI_BIN" export --job "$PROJECT_ID" --output "$OUTPUT_EPUB" --format epub
echo "Exportado para: $OUTPUT_EPUB"
echo ""

echo "=== 7. Testando tradução com FakeProvider (DOCX) ==="
"$CLI_BIN" translate \
    --project "$PROJECT_ID2" \
    --provider fake \
    --target-lang pt-BR \
    --batch-size 5 \
    --max-parallel 2 \
    --output-format docx \
    --bilingual false

echo "Aguardando tradução concluir..."
sleep 5

"$CLI_BIN" info "$PROJECT_ID2"
echo ""

echo "=== 8. Exportando DOCX traduzido ==="
OUTPUT_DOCX="$OUTPUT_DIR/test_document_translated.docx"
"$CLI_BIN" export --job "$PROJECT_ID2" --output "$OUTPUT_DOCX" --format docx
echo "Exportado para: $OUTPUT_DOCX"
echo ""

echo "=== 9. Verificando arquivos de saída ==="
ls -la "$OUTPUT_DIR"
echo ""

echo "=== 10. Validando EPUB exportado ==="
if [ -f "$OUTPUT_EPUB" ]; then
    echo "Verificando estrutura do EPUB..."
    unzip -l "$OUTPUT_EPUB"
    echo ""
    # Verificar se tem nav.xhtml e content.opf
    unzip -p "$OUTPUT_EPUB" OEBPS/nav.xhtml | head -20
    echo ""
    unzip -p "$OUTPUT_EPUB" OEBPS/content.opf | head -30
fi
echo ""

echo "=== 10. Validando DOCX exportado ==="
if [ -f "$OUTPUT_DOCX" ]; then
    echo "Verificando estrutura do DOCX..."
    unzip -l "$OUTPUT_DOCX"
    echo ""
    unzip -p "$OUTPUT_DOCX" word/document.xml | head -30
fi

echo ""
echo "=== Teste E2E Concluído ==="
echo "Resultados em: $OUTPUT_DIR"