# Medidor de DPS para AION 2 (Global)

Overlay externo que mostra o dano de cada jogador e de cada skill, lendo o tráfego de
rede do jogo passivamente. Não injeta código, não lê memória e não abre handle no
processo do jogo. Formato do protocolo e medições em [PROTOCOLO.md](PROTOCOLO.md).

## Usar

Pela release: baixe o zip da última versão em Releases, extraia e abra o
`Aion2Meter.Overlay.exe` (o .NET vai dentro do executável; passo a passo nas notas da
release). Para compilar daqui:

```powershell
dotnet build -c Release
.\src\Aion2Meter.Overlay\bin\Release\net10.0-windows\Aion2Meter.Overlay.exe
```

- O Windows pede permissão de administrador ao abrir (a captura por raw socket exige).
- O jogo precisa estar em "tela cheia em janela" (borderless), que é o padrão atual.
- Cada jogador ocupa duas linhas: nome, DPS (DTPS no Tank, HPS no Healer), total e % em
  cima; `Classe · Nv · Power` sempre embaixo (classe em inglês, por escolha). `?` = ainda
  não chegou; `~31` = valor da memória, visto numa sessão anterior e talvez velho.
- A memória (`%LOCALAPPDATA%\Aion2Meter\jogadores.json`) guarda o último level e Power de
  cada nome, inclusive o seu. Com o overlay aberto no meio da sessão, você é reconhecido
  pelo nome no primeiro abate ou invocação. Valor lido na conexão atual sempre vence.
- Arraste a janela pelo fundo. Clique num jogador para ver as skills. "Zerar" começa
  uma luta nova; ela também zera sozinha depois de 15 s sem dano.
- Dano exibido em unidades de HP do alvo (campo do pacote × 18,82, ver PROTOCOLO.md §6).
- Invocações, pets e armadilhas somam na linha do dono.
- Skills aparecem em português e com ícone. Nomes: questlog.gg (base comunitária montada
  do cliente Global, API não documentada); ícones: CDN oficial da NCSoft. Tudo fica em
  `%LOCALAPPDATA%\Aion2Meter` (`skills-pt.json` e `icones/`); apague a pasta para rebaixar.
- Clicar no overlay não deveria tirar o foco do teclado do jogo (`WS_EX_NOACTIVATE`;
  a confirmar no jogo, junto com arrastar e expandir jogador).
- Rede: a captura é passiva, mas o medidor faz requisições próprias ao questlog.gg (só o
  código da skill) e ao CDN da NCSoft (ícones), uma vez por skill, depois fica no cache.
- Abas **DPS | Tank | Healer**: a luta é a mesma (duração, "Zerar" e os 15 s valem para
  as três), cada aba com o seu total e o detalhe por skill:
  - **DPS**: dano causado em monstros.
  - **Tank**: dano recebido de monstros (DTPS), golpes aparados e mortes (☠). "aggro N"
    marca quantos monstros acertaram aquele jogador por último nos últimos 8 s. O valor
    de ameaça fica no servidor e não chega ao jogo, então não existe tabela de ameaça.
    Golpes de monstro aparecem como "Golpe de monstro (código)": o questlog só tem skills
    de jogador.
  - **Healer**: cura feita em jogadores, inclusive em si mesmo. Cura é a skill da lista
    fixa ou com o marcador `SkillUIHPHeal` na descrição do questlog.
- Jogador → jogador sem ser cura (PvP, buffs) fica de fora: o medidor não mede PvP.

## Estrutura

| Pasta | Conteúdo |
|---|---|
| `src/Aion2Meter.Core` | Protocolo (varint, LZ4, framing, parsers), captura (raw socket, pcapng, remontagem TCP), medição (placar) |
| `src/Aion2Meter.Overlay` | Janela WPF sempre no topo |
| `src/Aion2Meter.Cli` | Replay de `.pcapng` com diagnóstico e modo `ao-vivo` no console |
| `tests/Aion2Meter.Testes` | LZ4 contra o compressor de referência (K4os), framing, remontagem TCP, parsers com bytes reais e placar (`dotnet test`) |
| `dados/skills.json` | Opcional, fora do repositório e das releases: nomes em inglês do RATmeter (GPL-3.0, ver PROTOCOLO.md §9), só reserva quando o questlog não tem a skill |
| `capturar.ps1` | Grava `captura.pcapng` com o pktmon do Windows (admin). Capturas ficam fora do repositório: têm o seu tráfego |
| `ao-vivo.ps1` | Roda o modo `ao-vivo` do CLI como admin e grava `ao-vivo-log.txt` |
| `.github/workflows/release.yml` | Tag `v*` → testes → executável único → release com o zip |

## Lançar uma versão

1. Ajuste `Version` em `Directory.Build.props` (SemVer: funcionalidade nova sobe a casa do
   meio e zera a última, 0.1.0 → 0.2.0; correção sobe a última, 0.2.0 → 0.2.1).
2. Com a mudança já no `main`: `git tag v0.2.0` e `git push origin v0.2.0`.
3. O workflow roda os testes e publica a release com `Aion2Meter-0.2.0-win-x64.zip`
   (acompanhe com `gh run watch`). A versão aparece no rodapé do overlay.

## Depois de um patch do jogo

1. Com o jogo aberto, rode `capturar.ps1` como administrador e bata em mobs no bipe.
2. `dotnet run -c Release --project src/Aion2Meter.Cli -- captura.pcapng`
3. Confira: "sincronizado=True", eventos de dano válidos > 0, "Razão queda/dano" perto
   de 18,8 e o placar no fim. Se o dano sumir, os opcodes mudaram: use `--op XXXX --hex N`
   para inspecionar e atualize `Protocolo/Opcodes.cs`.

## Pendências conhecidas

- **Level e "(você)"**: o seu level e o "(você)" só chegam no login (`0x3633`), então
  abra o overlay antes de entrar no mundo. Troca de servidor é resolvida
  sozinha (PROTOCOLO.md §8) e começa uma luta nova. O dos outros chega quando eles entram no seu
  campo de visão (`0x3645`). Depois de um level up o número fica velho até o próximo
  login (ou até o jogador sair e voltar à sua visão): o pacote de level up não é conhecido.
- **Fator 18,82 medido só com Ranger.** Conferir com outras classes numa captura em grupo.
- **DoT** (`0x3805`): layout não conferido (Ranger não gerou nenhum).
- **Healer não conferido**: nenhuma captura teve curandeiro, e a escala 1:1 da cura é
  suposição. Golpe de jogador em mob com skill que o questlog marca como cura (dreno, por
  exemplo) sai do DPS e entra no Healer com fator 1 em vez de 18,82. Só mexer nessa regra
  quando uma captura mostrar uma skill assim.
- **Tank**: escala 1:1 conferida num golpe só. "aggro" é inferido do último golpe de
  cada mob, porque o pacote de troca de alvo não foi achado.
- Invocação de jogador sem legião: o vínculo depende só do nome do dono.

## Conferido

- Crítico: dentro da mesma skill, `tipo_dano 3` tem média 1,6 vez maior que o tipo 2
  (Disparo Rápido: 384 contra 238), então a leitura de crítico está certa. O 0% do teste
  ao vivo (153 golpes) é dado real daquela luta.
