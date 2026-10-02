# Captura o tráfego do servidor de jogo do AION 2 com o pktmon (nativo do Windows).
# Precisa rodar como administrador, com o personagem já logado no mundo.
# Uso: powershell -ExecutionPolicy Bypass -File capturar.ps1 [-Segundos 60]
param([int]$Segundos = 60)

$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
$log = Join-Path $dir 'captura-log.txt'
Set-Content -Path $log -Value "Captura iniciada $(Get-Date -Format s)" -Encoding utf8

function Log([string]$texto) {
    Write-Host $texto
    Add-Content -Path $log -Value $texto -Encoding utf8
}

function Invoke-Pktmon {
    $saida = & pktmon.exe @args 2>&1 | Out-String
    Log "> pktmon $($args -join ' ')"
    Log $saida.Trim()
}

try {
    $pids = (Get-Process AION2 -ErrorAction Stop).Id
    $conn = Get-NetTCPConnection -OwningProcess $pids -State Established -ErrorAction Stop |
            Where-Object { $_.RemotePort -ne 443 -and $_.RemoteAddress -ne '127.0.0.1' } |
            Select-Object -First 1
    if (-not $conn) { throw 'Conexão com o servidor de jogo não encontrada.' }

    $porta = $conn.RemotePort
    Log "servidor=$($conn.RemoteAddress) porta_servidor=$porta porta_local=$($conn.LocalPort)"

    Invoke-Pktmon stop
    Invoke-Pktmon filter remove
    Invoke-Pktmon filter add aion2 -t TCP -p $porta
    Invoke-Pktmon start --capture --comp nics --pkt-size 0 -f (Join-Path $dir 'captura.etl')

    [console]::Beep(880, 400)
    for ($i = $Segundos; $i -gt 0; $i--) {
        Write-Host -NoNewline "`rCapturando: bata num mob. Faltam $i s   "
        Start-Sleep -Seconds 1
    }
    Write-Host ''

    Invoke-Pktmon stop
    [console]::Beep(660, 200); [console]::Beep(440, 400)

    # Teleporte ou troca de canal pode abrir outra conexão: o filtro de porta perderia o resto.
    $depois = Get-NetTCPConnection -OwningProcess $pids -State Established -ErrorAction SilentlyContinue |
              Where-Object { $_.RemotePort -ne 443 -and $_.RemoteAddress -ne '127.0.0.1' } |
              Select-Object -First 1
    if ($depois -and ($depois.RemotePort -ne $porta -or $depois.LocalPort -ne $conn.LocalPort)) {
        Log "AVISO: a conexão do jogo mudou para $($depois.RemoteAddress):$($depois.RemotePort) durante a captura; o arquivo pode estar incompleto."
    }
    Invoke-Pktmon etl2pcap (Join-Path $dir 'captura.etl') -o (Join-Path $dir 'captura.pcapng')
}
catch {
    Log "ERRO: $($_.Exception.Message)"
}
finally {
    Invoke-Pktmon filter remove
    Log "Captura finalizada $(Get-Date -Format s)"
}
