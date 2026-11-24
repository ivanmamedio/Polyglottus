# Polyglottus - PowerShell Helper Script
# Execute: .\run.ps1 <command>
# Commands: check, test, build-cli, run-cli, e2e, ui-dev

param([string]$command = "help")

# Define o PATH para incluir o cargo/rustup
$env:PATH = "C:\Users\carlo\.cargo\bin;" + $env:PATH
$projectDir = "C:\Users\carlo\Documents\Polyglottus"

switch ($command) {
    "check" {
        Write-Host "=== Running cargo check --workspace ===" -ForegroundColor Cyan
        Set-Location $projectDir
        cargo check --workspace
    }
    "test" {
        Write-Host "=== Running cargo test --workspace (excluding pgl-models) ===" -ForegroundColor Cyan
        Set-Location $projectDir
        cargo test --workspace --exclude pgl-models
    }
    "build-cli" {
        Write-Host "=== Building pgl-cli (release) ===" -ForegroundColor Cyan
        Set-Location $projectDir
        cargo build --release -p pgl-cli
    }
    "run-cli" {
        Write-Host "=== Running pgl-cli --help ===" -ForegroundColor Cyan
        Set-Location $projectDir
        cargo run --bin pgl-cli -- --help
    }
    "e2e" {
        Write-Host "=== Running E2E pipeline test ===" -ForegroundColor Cyan
        Set-Location $projectDir
        if (-not (Test-Path "target/release/pgl-cli.exe")) {
            Write-Host "CLI not found. Building first..." -ForegroundColor Yellow
            cargo build --release -p pgl-cli
        }
        & ".\test-files\test_e2e_pipeline.sh"
    }
    "ui-dev" {
        Write-Host "=== Starting Tauri dev mode ===" -ForegroundColor Cyan
        Set-Location "$projectDir\apps\desktop"
        cargo tauri dev
    }
    "fmt" {
        Write-Host "=== Running cargo fmt --all ===" -ForegroundColor Cyan
        Set-Location $projectDir
        cargo fmt --all
    }
    "clippy" {
        Write-Host "=== Running cargo clippy ===" -ForegroundColor Cyan
        Set-Location $projectDir
        cargo clippy --workspace --all-targets -- -D warnings
    }
    "help" {
        Write-Host "Polyglottus Helper Script" -ForegroundColor Green
        Write-Host ""
        Write-Host "Usage: .\run.ps1 <command>"
        Write-Host ""
        Write-Host "Commands:"
        Write-Host "  check    - Run cargo check --workspace"
        Write-Host "  test     - Run cargo test (excluding pgl-models)"
        Write-Host "  build-cli - Build pgl-cli release binary"
        Write-Host "  run-cli   - Run pgl-cli --help"
        Write-Host "  e2e      - Run E2E pipeline test (import -> translate -> export)"
        Write-Host "  ui-dev   - Start Tauri dev mode (UI)"
        Write-Host "  fmt      - Format code with cargo fmt"
        Write-Host "  clippy   - Run clippy lints"
        Write-Host "  help     - Show this help"
    }
    default {
        Write-Host "Unknown command: $command" -ForegroundColor Red
        Write-Host "Run .\run.ps1 help for available commands"
    }
}
