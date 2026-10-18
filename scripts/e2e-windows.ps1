# Run in Windows PowerShell 5.1. Uses synthetic speech only; never opens a microphone.
param(
    [string]$WhisperModel = $env:WHISPER_MODEL,
    [string]$VadModel = $env:WHISPER_VAD_MODEL,
    [string]$NotesModel = 'qwen3:8b'
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
$Work = Join-Path ([IO.Path]::GetTempPath()) ('motet-e2e-' + [guid]::NewGuid())
$OldLocation = Get-Location
$OldHost = $env:OLLAMA_HOST
function Run-Native([string]$Tool, [string[]]$Arguments) {
    & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Tool failed with exit code $LASTEXITCODE" }
}
function Check([string]$Name, [bool]$Passed) {
    if (-not $Passed) { throw "FAIL $Name" }
    Write-Host "PASS $Name"
}
try {
    foreach ($Tool in 'cargo', 'ffmpeg', 'whisper-cli', 'ollama') {
        Get-Command $Tool -ErrorAction Stop | Out-Null
    }
    foreach ($Model in $WhisperModel, $VadModel) {
        if (-not $Model -or -not (Test-Path -LiteralPath $Model)) { throw 'Set -WhisperModel and -VadModel to installed model paths.' }
    }
    $WhisperPath = (Resolve-Path -LiteralPath $WhisperModel).Path.Replace('\', '/')
    $VadPath = (Resolve-Path -LiteralPath $VadModel).Path.Replace('\', '/')
    $env:OLLAMA_HOST = 'http://127.0.0.1:11434'
    Set-Location $Root
    Run-Native cargo @('fmt', '--check')
    Run-Native cargo @('clippy', '--all-targets', '--', '-D', 'warnings')
    Run-Native cargo @('test')
    Run-Native cargo @('build', '--release')
    $Binary = Join-Path $Root 'target/release/motet.exe'
    New-Item -ItemType Directory $Work | Out-Null
    Set-Location $Work
    $Settings = @("WHISPER_MODEL='$WhisperPath'", "WHISPER_VAD_MODEL='$VadPath'", "OLLAMA_MODEL='$NotesModel'", 'FIX_MODEL=')
    [IO.File]::WriteAllLines((Join-Path $Work '.env'), $Settings, (New-Object Text.UTF8Encoding($false)))
    Add-Type -AssemblyName System.Speech
    $Speech = New-Object System.Speech.Synthesis.SpeechSynthesizer
    try {
        $Speech.SetOutputToWaveFile((Join-Path $Work 'them.wav'))
        $Speech.Speak('Thanks for joining. Who can take the login bug?')
        $Speech.SetOutputToNull()
        $Speech.SetOutputToWaveFile((Join-Path $Work 'me.wav'))
        $Speech.Speak('I can take the login bug. We will ship on Friday.')
        $Speech.SetOutputToNull()
    } finally { $Speech.Dispose() }
    Run-Native ffmpeg @('-loglevel','error','-y','-i','them.wav','-ar','48000','call.system.wav')
    Run-Native ffmpeg @('-loglevel','error','-y','-f','lavfi','-t','3','-i','anullsrc=r=48000:cl=mono','-i','me.wav','-filter_complex','[0][1]concat=n=2:v=0:a=1','-ar','48000','call.mic.wav')
    $Help = & $Binary --help
    Check 'CLI lists record and run' (($Help -match 'record') -and ($Help -match 'run'))
    $ErrorActionPreference = 'Continue'
    & $Binary run --file missing.wav 2>&1 | Out-Null
    $ErrorActionPreference = 'Stop'
    Check 'missing input is refused' ($LASTEXITCODE -ne 0)
    Run-Native $Binary @('run','--file','call.system.wav','--file','call.mic.wav')
    $Lines = Get-Content call.txt
    Check 'system track is Them' ($Lines[0] -match 'Them:')
    Check 'mic track retains its leading silence' ([bool]($Lines -match '^\[00:0[2-5]\] Me:'))
    Check 'notes written with transcript' ((Test-Path call.md) -and ((Get-Content call.md -Raw) -match '<details>'))
    Run-Native $Binary @('run','--file','me.wav')
    Check 'single-file import' ((Get-Content me.txt -Raw) -match '(?i)friday')
    Write-Host 'Windows synthetic e2e: all checks passed. Live capture is still untested.'
} finally {
    Set-Location $OldLocation
    $env:OLLAMA_HOST = $OldHost
    if (Test-Path $Work) { Remove-Item -LiteralPath $Work -Recurse -Force }
}
