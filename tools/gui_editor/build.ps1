param([switch]$Dev, [switch]$Test, [switch]$LocalFidelity)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '../..')).Path
$env:RUSTUP_AUTO_INSTALL = '0'
$env:CARGO_HOME = Join-Path $here '.cargo-cache'
$env:npm_config_cache = Join-Path $here '.npm-cache'
$rustBins = Join-Path ([Environment]::GetFolderPath('UserProfile')) '.cargo/bin'
$env:PATH = "$rustBins;$env:PATH"
$env:TEMP = Join-Path $here '.tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
Push-Location $here
try {
    npm ci; if ($LASTEXITCODE) { throw 'npm ci failed' }
    if ($Test) {
        npm test; if ($LASTEXITCODE) { throw 'Frontend tests failed' }
        cargo test --locked --manifest-path src-tauri/Cargo.toml;
        if ($LocalFidelity) {
            if ($LASTEXITCODE) { throw 'Public Rust tests failed' }
            cargo test --locked --manifest-path src-tauri/Cargo.toml --features local-fidelity -- --ignored
        } if ($LASTEXITCODE) { throw 'Rust tests failed' }
    } elseif ($Dev) {
        npm run tauri -- dev; if ($LASTEXITCODE) { throw 'Development launch failed' }
    } else {
        npm run tauri -- build --no-bundle -- --locked; if ($LASTEXITCODE) { throw 'Build failed' }
    }
} finally { Pop-Location }
