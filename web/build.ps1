param([string]$WasmBindgen = 'wasm-bindgen')

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo build --locked --release --lib --target wasm32-unknown-unknown
    if ($LASTEXITCODE) { throw 'Rust WebAssembly build failed' }

    $output = New-Item -ItemType Directory -Path 'release/UltraMP3-Web' -Force
    & $WasmBindgen --target web --no-typescript --out-dir (Join-Path $output.FullName 'pkg') --out-name ultramp3 'target/wasm32-unknown-unknown/release/ultramp3_windows.wasm'
    if ($LASTEXITCODE) { throw 'wasm-bindgen failed' }

    Copy-Item -LiteralPath 'web/index.html', 'web/player.css', 'web/player.js' -Destination $output.FullName -Force
    Copy-Item -LiteralPath 'extracted/05_0_UltraDemo.mp3' -Destination (Join-Path $output.FullName 'UltraDemo.mp3') -Force
} finally {
    Pop-Location
}
