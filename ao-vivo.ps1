# Roda o medidor em modo ao vivo (raw socket, precisa de administrador) e grava a saída em ao-vivo-log.txt.
# Uso: powershell -ExecutionPolicy Bypass -File ao-vivo.ps1 [-Segundos 45]
param([int]$Segundos = 45)

$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
# Compilar antes: cargo build --release -p replay
$exe = Join-Path $dir 'target\release\replay.exe'
$log = Join-Path $dir 'ao-vivo-log.txt'

[Console]::OutputEncoding = [Text.Encoding]::UTF8
[console]::Beep(880, 400)
& $exe ao-vivo $Segundos 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log
[console]::Beep(660, 200); [console]::Beep(440, 400)
