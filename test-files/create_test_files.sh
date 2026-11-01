#!/bin/bash
set -e

TEST_DIR="/home/carlos/Documentos/Polyglottus/test-files"
cd "$TEST_DIR"

echo "Criando EPUB de teste..."

# Criar estrutura EPUB mínima
mkdir -p test_epub/OEBPS
mkdir -p test_epub/META-INF

# mimetype (deve ser primeiro, sem compressão)
echo -n "application/epub+zip" > test_epub/mimetype

# META-INF/container.xml
cat > test_epub/META-INF/container.xml << 'EOF'
<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
EOF

# OEBPS/content.opf
cat > test_epub/OEBPS/content.opf << 'EOF'
<?xml version="1.0"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf" unique-id="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">test-book-001</dc:identifier>
    <dc:title>Livro de Teste</dc:title>
    <dc:language>en</dc:language>
    <dc:creator>Autor de Teste</dc:creator>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>
EOF

# OEBPS/nav.xhtml
cat > test_epub/OEBPS/nav.xhtml << 'EOF'
<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Sumário</title></head>
<body>
  <nav epub:type="toc">
    <ol>
      <li><a href="chapter1.xhtml">Capítulo 1</a></li>
    </ol>
  </nav>
</body>
</html>
EOF

# OEBPS/chapter1.xhtml
cat > test_epub/OEBPS/chapter1.xhtml << 'EOF'
<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Capítulo 1</title></head>
<body>
  <h1>Capítulo Um</h1>
  <p>This is the first paragraph of the test book.</p>
  <p>This is the second paragraph with <b>bold text</b> and <i>italic text</i>.</p>
  <ul>
    <li>First item</li>
    <li>Second item</li>
  </ul>
  <p>Final paragraph with a <a href="http://example.com">link</a>.</p>
</body>
</html>
EOF

# Criar EPUB
cd test_epub
zip -0 ../test_book.epub mimetype
zip -r ../test_book.epub META-INF OEBPS
cd ..

echo "EPUB criado: test_book.epub"

echo ""
echo "Criando DOCX de teste..."

# Criar DOCX mínimo usando Python se disponível
python3 << 'PYEOF'
import zipfile
import os

docx_path = "/home/carlos/Documentos/Polyglottus/test-files/test_document.docx"

with zipfile.ZipFile(docx_path, 'w', zipfile.ZIP_DEFLATED) as docx:
    # [Content_Types].xml
    content_types = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>'''
    docx.writestr('[Content_Types].xml', content_types)
    
    # _rels/.rels
    rels = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>'''
    docx.writestr('_rels/.rels', rels)
    
    # word/document.xml
    document = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>This is the first paragraph of the test document.</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>This is the second paragraph with </w:t></w:r>
      <w:r><w:rPr><w:b/></w:rPr><w:t>bold text</w:t></w:r>
      <w:r><w:t> and </w:t></w:r>
      <w:r><w:rPr><w:i/></w:rPr><w:t>italic text</w:t></w:r>
      <w:r><w:t>.</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Final paragraph with a link.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>'''
    docx.writestr('word/document.xml', document)
    
    # word/_rels/document.xml.rels
    doc_rels = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>'''
    docx.writestr('word/_rels/document.xml.rels', doc_rels)

print(f"DOCX criado: {docx_path}")
PYEOF

echo ""
echo "Arquivos de teste criados:"
ls -la /home/carlos/Documentos/Polyglottus/test-files/