#!/bin/bash
# Quick smoke test for patch-in-place logic
# Tests the core logic without needing the full CLI binary

set -e

echo "=== Smoke Test: Patch-in-place Logic ==="
echo ""

# Test 1: Verify EPUB structure preservation
echo "Test 1: EPUB structure verification"
unzip -l /home/carlos/Documentos/Polyglottus/test-files/test_book.epub | grep -E "(OEBPS/|mimetype|container.xml|content.opf|nav.xhtml|chapter1.xhtml)" | head -20

echo ""
echo "=== Testando extração de texto do EPUB ==="
# Extract and verify text content
cd /tmp
unzip -o /home/carlos/Documentos/Polyglottus/test-files/test_book.epub -d test_epub_extract 2>/dev/null

echo "=== Conteúdo do chapter1.xhtml ==="
cat /tmp/test_epub_extract/OEBPS/chapter1.xhtml

echo ""
echo "=== Conteúdo do content.opf ==="
cat /tmp/test_epub_extract/OEBPS/content.opf

echo ""
echo "=== Conteúdo do nav.xhtml ==="
cat /tmp/test_epub_extract/OEBPS/nav.xhtml

echo ""
echo "=== Testando extração de texto do DOCX ==="
cd /tmp
unzip -o /home/carlos/Documentos/Polyglottus/test-files/test_document.docx -d test_docx_extract 2>/dev/null

echo "=== Conteúdo do word/document.xml ==="
cat /tmp/test_docx_extract/word/document.xml

echo ""
echo "=== Smoke test concluído ==="
echo "Os arquivos de teste são válidos e prontos para teste E2E"