# Medidor de DPS para AION 2 (Global)

Overlay externo que mostra o dano de cada jogador e de cada skill, lendo o tráfego de
rede do jogo passivamente. Não injeta código, não lê memória e não abre handle no
processo do jogo. Formato do protocolo e medições em [PROTOCOLO.md](PROTOCOLO.md).

## Usar

Pela release: baixe o zip da última versão em Releases, extraia e abra o
`Aion2Meter.exe` (executável único de ~2 MB, sem instalar nada; passo a passo nas notas
da release). Para compilar daqui (Rust estável, testado com 1.99 MSVC):

```powershell
cargo build --release -p overlay
.\target\release\Aion2Meter.exe
```

- O Windows pede permissão de administrador ao abrir (a captura por raw socket exige).
- O jogo precisa estar em "tela cheia em janela" (borderless), que é o padrão atual.
- Cada jogador ocupa duas linhas: o nome em cima e `Classe · Nv · GS` embaixo (classe em
  inglês, por escolha; GS é o número que o jogo mostra como Power). `?` = ainda não
  chegou; `~31` = valor da memória, visto numa sessão anterior e talvez velho.
- Aba DPS: à direita de cada jogador, a tabela `DPS | Damage(%) | CRIT | AVG | MAX`.
  Damage é o dano com a parte do jogador no total do grupo medido; CRIT é a fração de
  golpes críticos; AVG, o dano médio por golpe; MAX, o maior golpe. Expandido, cada skill
  que aconteceu aparece com as mesmas colunas (aí o % é a parte da skill no total do
  jogador). Tank e Healer mostram DTPS/HPS, total e % e, expandidos, o detalhe e as 8
  maiores skills.
- A memória (`%LOCALAPPDATA%\Aion2Meter\jogadores.json`) guarda o último level e Power de
  cada nome, inclusive o seu, a cada 30 s e ao fechar. Com o overlay aberto no meio da
  sessão, você é reconhecido pelo nome no primeiro abate ou invocação. Valor lido na
  conexão atual sempre vence.
- Arraste a janela pelo fundo. Clique num jogador para ver as skills. "Zerar" começa
  uma luta nova; ela também zera sozinha depois de 15 s sem dano (ajustável).
- ⚙ abre as configurações, que valem na hora e ficam em
  `%LOCALAPPDATA%\Aion2Meter\config.json`:
  - **Linha de cada jogador**: clique numa coluna da tabela da aba DPS ou em Classe, Nv
    ou GS na linha de amostra para esconder ou mostrar (o escondido fica riscado).
  - **Só o meu dano**: só a sua linha, nas três abas; a % fica sempre em 100%. Enquanto
    você não foi reconhecido (ver Pendências), a lista fica vazia com um aviso.
  - **Alcance**: Proximidade (todos que aparecem perto). Party está desativado até uma
    captura em grupo mostrar o pacote do grupo.
  - **Luta**: segundos sem dano até a luta zerar, de 5 a 120.
  - **Tamanho**: de 60% a 200%, escalando a janela inteira. A alça no canto de baixo à
    direita faz o mesmo arrastando.
- ‹ ou › (a seta aponta para a borda mais perto) desliza a janela até a borda da área de
  trabalho do monitor e deixa só a aba "Overlay"; clicar nela traz a janela de volta ao
  mesmo lugar. Recolhido, o medidor continua contando. Com as animações do Windows
  desligadas, a janela vai direto, sem deslizar.
- O rodapé mostra a versão e só avisa enquanto procura o servidor do jogo, enquanto baixa
  os nomes das skills ou quando a captura para.
- Dano exibido em unidades de HP do alvo (campo do pacote × 18,82, ver PROTOCOLO.md §6).
- Invocações, pets e armadilhas somam na linha do dono.
- Skills aparecem em português e com ícone. Nomes: questlog.gg (base comunitária montada
  do cliente Global, API não documentada); ícones: CDN oficial da NCSoft. Tudo fica em
  `%LOCALAPPDATA%\Aion2Meter` (`skills-pt.json` e `icones/`); apague a pasta para rebaixar.
- Clicar no overlay não tira o foco do teclado do jogo (`WS_EX_NOACTIVATE`, reaplicado a
  cada quadro porque o winit apaga o bit ao mostrar a janela; conferido no jogo em
  2026-10-02, junto com arrastar e expandir jogador).
- Rede: a captura é passiva, mas o medidor faz requisições próprias ao questlog.gg (só o
  código da skill) e ao CDN da NCSoft (ícones), uma vez por skill, depois fica no cache.
- Abas **DPS | Tank | Healer**: a luta é a mesma (duração, "Zerar" e o tempo sem dano
  valem para as três), cada aba com o seu total e o detalhe por skill:
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
| `crates/nucleo` | Protocolo (varint, LZ4, framing, parsers), captura (raw socket, pcapng, remontagem TCP), medição (placar, catálogo de skills) e formatação pt-BR |
| `crates/nucleo/tests` | LZ4 contra o `lz4_flex`, framing, remontagem TCP, parsers com bytes reais, placar e troca de servidor (`cargo test`). O teste de rede do catálogo é opcional: `cargo test -p nucleo --test catalogo -- --ignored` |
| `crates/overlay` | Janela sempre no topo (egui/eframe), gera o `Aion2Meter.exe`. O build de debug abre sem administrador e aceita `cargo run -p overlay -- --replay captura.pcapng [--tank] [--expandir] [--config] [--zoom 1.3] [--recolher \| --recolher-e-voltar] [--posicao x y]` para ver a janela sem o jogo (no replay, a config é lida mas não é gravada) |
| `crates/replay` | Replay de `.pcapng` com diagnóstico e modo `ao-vivo` no console |
| `dados/skills.json` | Opcional, fora do repositório e das releases: nomes em inglês do RATmeter (GPL-3.0, ver PROTOCOLO.md §9), só reserva quando o questlog não tem a skill |
| `capturar.ps1` | Grava `captura.pcapng` com o pktmon do Windows (admin). Capturas ficam fora do repositório: têm o seu tráfego |
| `ao-vivo.ps1` | Roda o modo `ao-vivo` do replay como admin e grava `ao-vivo-log.txt` (antes: `cargo build --release -p replay`) |
| `.github/workflows/release.yml` | Tag `v*` → testes → `Aion2Meter.exe` → release com o zip |

## Lançar uma versão

1. Ajuste `version` em `[workspace.package]` do `Cargo.toml` (SemVer: funcionalidade nova
   sobe a casa do meio e zera a última, 0.2.0 → 0.3.0; correção sobe a última, 0.3.0 →
   0.3.1). A versão aparece no rodapé do overlay.
2. Com a mudança já no `main`: `git tag v0.3.0` e `git push origin v0.3.0`.
3. O workflow confere se a tag bate com o `Cargo.toml`, roda os testes e publica a release
   com `Aion2Meter-0.3.0-win-x64.zip` (acompanhe com `gh run watch`).

Enquanto o Actions deste repositório falhar antes de criar os jobs (`startup_failure`,
visto em 2026-10-02), gere o zip num clone limpo da tag (`cargo build --release -p
overlay`, zip só com o `Aion2Meter.exe`) e publique com `gh release create` usando
`.github/notas-da-release.md`.

## Depois de um patch do jogo

1. Com o jogo aberto, rode `capturar.ps1` como administrador e bata em mobs no bipe.
2. `cargo run --release -p replay -- captura.pcapng`
3. Confira: "sincronizado=True", eventos de dano válidos > 0, "Razão queda/dano" perto
   de 18,8 e o placar no fim. Se o dano sumir, os opcodes mudaram: use `--op XXXX --hex N`
   para inspecionar e atualize `crates/nucleo/src/protocolo/opcodes.rs`.

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
- **Party** (só quem está no seu grupo): o pacote do grupo (`0x9702` em outros medidores)
  não apareceu em nenhuma captura local; falta uma captura em grupo.

## Conferido

- Crítico: dentro da mesma skill, `tipo_dano 3` tem média 1,6 vez maior que o tipo 2
  (Disparo Rápido: 384 contra 238), então a leitura de crítico está certa. O 0% do teste
  ao vivo (153 golpes) é dado real daquela luta.
- Port de C# para Rust (0.1.0 → 0.2.0): o replay em Rust deu saída idêntica à do CLI em
  C# nas 5 capturas locais, inclusive nas opções de diagnóstico.
