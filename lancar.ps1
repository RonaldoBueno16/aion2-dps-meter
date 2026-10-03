# Lança uma versão do Axon a partir deste PC. Todo exe que vai para uma release sai daqui: o
# GitHub Actions do repositório está parado (conta travada por cobrança; o dono decidiu em
# 2026-10-03 manter o lançamento local). Com o Actions funcionando, a tag sozinha publica pelo
# .github/workflows/release.yml: aí não use -Publicar, ou os dois tentam criar a mesma release.
#
# Uso (Windows PowerShell 5.1, na pasta do repositório):
#   powershell -ExecutionPolicy Bypass -File lancar.ps1 -Versao 0.6.0                 # ensaio do main
#   powershell -ExecutionPolicy Bypass -File lancar.ps1 -Versao 0.6.0 -Ramo feat/x     # ensaio de um ramo
#   powershell -ExecutionPolicy Bypass -File lancar.ps1 -Versao 0.6.0 -Publicar        # publica
#
# O ensaio faz tudo menos publicar: clone limpo, testes, build, conferências do exe e o zip.
# -Publicar faz o mesmo e, se tudo passar, cria a tag no commit testado, publica a release, baixa
# de volta e confere o SHA-256 com o digest da API do GitHub (o que o botão Atualizar confere).
# Não abre o Axon.exe (ele pede administrador). Cada execução usa uma pasta nova em
# %TEMP%\axon-lancamento; nada é apagado.
param(
    [Parameter(Mandatory)][string]$Versao,
    [string]$Ramo = 'main',
    [switch]$Publicar
)

$ErrorActionPreference = 'Stop'
$Repo = 'RonaldoBueno16/aion2-dps-meter'
$Tag = "v$Versao"
$Zip = "Aion2Meter-$Versao-win-x64.zip"
# O botão Atualizar recusa exe maior que isto (TAMANHO_MAXIMO em crates/overlay/src/atualizacao.rs).
$TamanhoMaximo = 64MB

function Passo([string]$Texto) { Write-Host "`n==> $Texto" -ForegroundColor Cyan }
function Falhar([string]$Texto) { throw "PAROU: $Texto" }

# Comando externo (git, gh, cargo): falha é código de saída diferente de 0. O 'Continue' local
# evita que o PowerShell 5.1 trate o que eles escrevem no stderr como erro.
function Nativo([string]$Nome, [scriptblock]$Comando) {
    $ErrorActionPreference = 'Continue'
    & $Comando
    if ($LASTEXITCODE -ne 0) { Falhar "$Nome (código $LASTEXITCODE)" }
}

function Sucesso([scriptblock]$Comando) {
    $ErrorActionPreference = 'Continue'
    $null = & $Comando 2>$null
    $LASTEXITCODE -eq 0
}

function Json([string]$Nome, [scriptblock]$Comando) {
    (Nativo $Nome $Comando) -join "`n" | ConvertFrom-Json
}

if ($Versao -notmatch '^\d+\.\d+\.\d+$') { Falhar "versão '$Versao' fora do formato 1.2.3" }
if ($Publicar -and $Ramo -ne 'main') { Falhar 'só se publica do main' }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path" }
# Um CARGO_TARGET_DIR herdado mandaria o build para fora do clone limpo.
Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue

Passo 'Conferências antes do build'
if (-not (Sucesso { gh auth status })) { Falhar 'gh sem login: rode gh auth login' }
if (Sucesso { gh release view $Tag --repo $Repo }) { Falhar "a release $Tag já existe" }
$ultima = (Json 'última release' { gh api "repos/$Repo/releases/latest" }).tag_name
if ([version]$Versao -le [version]$ultima.TrimStart('v')) { Falhar "$Tag não é maior que a última release ($ultima)" }
$tagRemota = Nativo 'tags do GitHub' { git -C $PSScriptRoot ls-remote --tags origin "refs/tags/$Tag" }
# Tag já publicada: o build sai dela. Senão sai do ramo, e a tag nasce no commit testado.
$fonte = if ($tagRemota) { $Tag } else { $Ramo }
Write-Host "Última release: $ultima. Build de: $fonte."

Passo "Clone limpo de $fonte"
$pasta = Join-Path $env:TEMP ('axon-lancamento\{0}-{1:yyyyMMdd-HHmmss}' -f $Versao, (Get-Date))
Nativo 'clone' { git clone --quiet --depth 1 --branch $fonte "https://github.com/$Repo.git" $pasta }
$commit = Nativo 'commit' { git -C $pasta rev-parse HEAD }
$versaoCargo = (Select-String -Path (Join-Path $pasta 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
if ($versaoCargo -ne $Versao) { Falhar "o Cargo.toml de $fonte diz $versaoCargo, não $Versao" }
$proibidos = Nativo 'arquivos' { git -C $pasta ls-files } |
    Where-Object { $_ -match '\.(pcapng|etl)$' -or $_ -eq 'dados/skills.json' }
if ($proibidos) { Falhar "capturas ou dados do RATmeter no git: $($proibidos -join ', ')" }
Write-Host "Commit $commit, Cargo.toml $versaoCargo, sem capturas nem skills.json."

Push-Location $pasta
try {
    Passo 'Testes'
    Nativo 'cargo test' { cargo test --workspace --locked }
    Passo 'Build de release'
    Nativo 'cargo build' { cargo build --release -p overlay --locked }
} finally {
    Pop-Location
}

Passo 'Conferindo o Axon.exe (sem abrir)'
$exe = Join-Path $pasta 'target\release\Axon.exe'
$arquivo = Get-Item $exe
if ($arquivo.VersionInfo.FileVersion -ne $Versao) { Falhar "o exe diz versão '$($arquivo.VersionInfo.FileVersion)'" }
if ($arquivo.VersionInfo.ProductName -ne 'Axon') { Falhar "o exe diz produto '$($arquivo.VersionInfo.ProductName)'" }
if ($arquivo.Length -lt 1MB -or $arquivo.Length -gt $TamanhoMaximo) { Falhar "tamanho fora do normal: $($arquivo.Length) bytes" }
if (-not (Select-String -Path $exe -Pattern 'requireAdministrator' -SimpleMatch -Encoding ascii -Quiet)) {
    Falhar 'o manifesto não pede administrador: a captura não abriria'
}
$hash = (Get-FileHash $exe -Algorithm SHA256).Hash
Write-Host "Versão $Versao, $($arquivo.Length) bytes, pede administrador. SHA-256 $hash"

Passo "Empacotando $Zip"
$caminhoZip = Join-Path $pasta $Zip
Compress-Archive -Path $exe -DestinationPath $caminhoZip
Expand-Archive $caminhoZip (Join-Path $pasta 'conferir-zip')
$dentro = @(Get-ChildItem (Join-Path $pasta 'conferir-zip') -File -Recurse)
if ($dentro.Count -ne 1 -or $dentro[0].Name -ne 'Axon.exe') { Falhar "o zip devia ter só o Axon.exe: $($dentro.Name -join ', ')" }
if ((Get-FileHash $dentro[0].FullName -Algorithm SHA256).Hash -ne $hash) { Falhar 'o exe dentro do zip é outro' }
Write-Host 'Zip só com o Axon.exe, o mesmo do build.'

if (-not $Publicar) {
    Write-Host "`nEnsaio completo, nada foi publicado." -ForegroundColor Green
    Write-Host "  exe: $exe"
    Write-Host "  zip: $caminhoZip"
    Write-Host "  SHA-256: $hash"
    Write-Host "Para publicar: powershell -ExecutionPolicy Bypass -File lancar.ps1 -Versao $Versao -Publicar"
    return
}

if (-not $tagRemota) {
    Passo "Criando a tag $Tag no commit testado"
    Nativo 'fetch' { git -C $PSScriptRoot fetch --quiet origin }
    Nativo 'tag' { git -C $PSScriptRoot tag -a $Tag $commit -m "Axon $Tag" }
    Nativo 'push da tag' { git -C $PSScriptRoot push --quiet origin $Tag }
}

Passo "Publicando a release $Tag"
Push-Location $pasta
try {
    # O zip e o exe solto: o botão Atualizar baixa o asset chamado Axon.exe.
    Nativo 'gh release create' {
        gh release create $Tag $caminhoZip $exe --repo $Repo --verify-tag --title "Axon $Tag" `
            --notes-file .github/notas-da-release.md --generate-notes
    }
} finally {
    Pop-Location
}

Passo 'Conferindo o que subiu'
$baixado = Join-Path $pasta 'baixado'
Nativo 'gh release download' { gh release download $Tag --repo $Repo --dir $baixado }
Expand-Archive (Join-Path $baixado $Zip) (Join-Path $baixado 'zip')
$release = Json 'release na API' { gh api "repos/$Repo/releases/tags/$Tag" }
$problemas = @()
if ((Get-FileHash (Join-Path $baixado 'Axon.exe') -Algorithm SHA256).Hash -ne $hash) { $problemas += 'o Axon.exe baixado é outro' }
if ((Get-FileHash (Join-Path $baixado 'zip\Axon.exe') -Algorithm SHA256).Hash -ne $hash) { $problemas += 'o exe do zip baixado é outro' }
$digest = ($release.assets | Where-Object name -eq 'Axon.exe').digest
if ($digest -ne "sha256:$($hash.ToLower())") { $problemas += "digest da API '$digest' diferente do build" }
$nomes = ($release.assets.name | Sort-Object) -join ', '
if ($nomes -ne "$Zip, Axon.exe") { $problemas += "assets inesperados: $nomes" }
$latest = (Json 'última release' { gh api "repos/$Repo/releases/latest" }).tag_name
if ($latest -ne $Tag) { $problemas += "a Latest é $latest" }
if ($problemas) { Falhar ("release publicada, mas: " + ($problemas -join '; ')) }

Write-Host "`nRelease $Tag publicada e conferida." -ForegroundColor Green
Write-Host "  $($release.html_url)"
Write-Host "  SHA-256 $hash (igual ao digest da API, ao exe solto e ao do zip)"
