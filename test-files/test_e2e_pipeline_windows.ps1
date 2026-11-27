#!/usr/bin/env pwsh
# Test script para pipeline E2E: import → translate (FakeProvider) → export
# Uso: pwsh test-e2e-pipeline-windows.ps1

$CLI_BIN = "C:\Users\carlo\Documents\Polyglottus\target\release\pgl-cli.exe"
$TEST_FILES_DIR = "C:\Users\carlo\Documents\Polyglottus\test-files"
$OUTPUT_DIR = "C:\Users\carlo\Documents\Polyglottus\test-output"

Write-Host "=== Polyglottus E2E Pipeline Test ===" -ForegroundColor Cyan
Write-Host "CLI: $CLI_BIN"
Write-Host "Test files: $TEST_FILES_DIR"
Write-Host "Output: $OUTPUT_DIR"
Write-Host ""

New-Item -ItemType Directory -Path $OUTPUT_DIR -Force | Out-Null

# Verificar se CLI existe
if (-not (Test-Path $CLI_BIN)) {
    Write-Host "ERRO: CLI não encontrado em $CLI_BIN" -ForegroundColor Red
    Write-Host "Compile primeiro com: cargo build --release -p pgl-cli" -ForegroundColor Yellow
    exit 1
}

# Função helper para executar CLI via cmd /c
function Run-Cmd {
    param([string]$CmdLine)
    try {
        $fullCmd = """$CLI_BIN"" $CmdLine"
        Write-Host "DEBUG: $fullCmd" -ForegroundColor DarkGray
        $output = cmd /c $fullCmd 2>&1
        $exitCode = $LASTEXITCODE
        return @{ Output = $output; ExitCode = $exitCode }
    } catch {
        return @{ Output = $_.Exception.Message; ExitCode = 1 }
    }
}

Write-Host "=== 1. Testando import EPUB ===" -ForegroundColor Green
$result = Run-Cmd "import --input `"$TEST_FILES_DIR\test_book.epub`" --name `"TesteEPUB`""
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO no import EPUB (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}

# Extrair project ID (UUID)
$PROJECT_ID = ($result.Output | Select-String -Pattern '[a-f0-9-]{36}' | Select-Object -First 1).Matches[0].Value
if (-not $PROJECT_ID) {
    Write-Host "ERRO: Não foi possível extrair Project ID" -ForegroundColor Red
    exit 1
}
Write-Host "Project ID: $PROJECT_ID"
Write-Host ""

Write-Host "=== 2. Testando import DOCX ===" -ForegroundColor Green
$result = Run-Cmd "import --input `"$TEST_FILES_DIR\test_document.docx`" --name `"TesteDOCX`""
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO no import DOCX (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}

$PROJECT_ID2 = ($result.Output | Select-String -Pattern '[a-f0-9-]{36}' | Select-Object -First 1).Matches[0].Value
if (-not $PROJECT_ID2) {
    Write-Host "ERRO: Não foi possível extrair Project ID 2" -ForegroundColor Red
    exit 1
}
Write-Host "Project ID 2: $PROJECT_ID2"
Write-Host ""

Write-Host "=== 3. Listando projetos ===" -ForegroundColor Green
$result = Run-Cmd "list"
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO no list (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}
Write-Host ""

Write-Host "=== 4. Info do projeto EPUB ===" -ForegroundColor Green
$result = Run-Cmd "info $PROJECT_ID"
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO no info (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}
Write-Host ""

Write-Host "=== 5. Testando tradução com FakeProvider (EPUB) ===" -ForegroundColor Green
$result = Run-Cmd "translate --project $PROJECT_ID --provider fake --target-lang pt-BR --batch-size 5 --max-parallel 2 --output-format epub"
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO na tradução EPUB (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}

# Extrair Job ID do output do translate
$JOB_ID = ($result.Output | Select-String -Pattern 'Job ID: ([a-f0-9-]{36})' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $JOB_ID) {
    Write-Host "ERRO: Não foi possível extrair Job ID" -ForegroundColor Red
    exit 1
}
Write-Host "Job ID: $JOB_ID"

Write-Host "Aguardando tradução concluir..."
Start-Sleep -Seconds 5

# Verificar status
$result = Run-Cmd "info $PROJECT_ID"
Write-Host $result.Output
Write-Host ""

Write-Host "=== 6. Exportando EPUB traduzido ===" -ForegroundColor Green
$OUTPUT_EPUB = "$OUTPUT_DIR\test_book_translated.epub"
$result = Run-Cmd "export --job $JOB_ID --output `"$OUTPUT_EPUB`" --format epub"
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO na exportação EPUB (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}
Write-Host "Exportado para: $OUTPUT_EPUB"
Write-Host ""

Write-Host "=== 7. Testando tradução com FakeProvider (DOCX) ===" -ForegroundColor Green
$result = Run-Cmd "translate --project $PROJECT_ID2 --provider fake --target-lang pt-BR --batch-size 5 --max-parallel 2 --output-format docx"
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO na tradução DOCX (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}

# Extrair Job ID do output do translate
$JOB_ID2 = ($result.Output | Select-String -Pattern 'Job ID: ([a-f0-9-]{36})' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $JOB_ID2) {
    Write-Host "ERRO: Não foi possível extrair Job ID 2" -ForegroundColor Red
    exit 1
}
Write-Host "Job ID 2: $JOB_ID2"

Write-Host "Aguardando tradução concluir..."
Start-Sleep -Seconds 5

$result = Run-Cmd "info $PROJECT_ID2"
Write-Host $result.Output
Write-Host ""

Write-Host "=== 8. Exportando DOCX traduzido ===" -ForegroundColor Green
$OUTPUT_DOCX = "$OUTPUT_DIR\test_document_translated.docx"
$result = Run-Cmd "export --job $JOB_ID2 --output `"$OUTPUT_DOCX`" --format docx"
Write-Host $result.Output
if ($result.ExitCode -ne 0) {
    Write-Host "ERRO na exportação DOCX (exit code: $($result.ExitCode))" -ForegroundColor Red
    exit 1
}
Write-Host "Exportado para: $OUTPUT_DOCX"
Write-Host ""

Write-Host "=== 9. Verificando arquivos de saída ===" -ForegroundColor Green
Get-ChildItem $OUTPUT_DIR | Select-Object Name, Length
Write-Host ""

Write-Host "=== 10. Validando EPUB exportado ===" -ForegroundColor Green
if (Test-Path $OUTPUT_EPUB) {
    Write-Host "EPUB exportado com sucesso!"
    Get-Item $OUTPUT_EPUB | Select-Object Name, Length
}
Write-Host ""

Write-Host "=== 11. Validando DOCX exportado ===" -ForegroundColor Green
if (Test-Path $OUTPUT_DOCX) {
    Write-Host "DOCX exportado com sucesso!"
    Get-Item $OUTPUT_DOCX | Select-Object Name, Length
}
Write-Host ""

Write-Host "=== Teste E2E Concluído ===" -ForegroundColor Cyan
Write-Host "Resultados em: $OUTPUT_DIR"
