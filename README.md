# Axon: medidor de DPS para AION 2 (Global)

Overlay externo que mostra o dano de cada jogador e de cada skill, lendo o tráfego de
rede do jogo passivamente. Não injeta código, não lê memória e não abre handle no
processo do jogo. Formato do protocolo e medições em [PROTOCOLO.md](PROTOCOLO.md).

## Usar

Pela release: baixe o zip da última versão em Releases, extraia e abra o
`Axon.exe` (executável único de ~2 MB, sem instalar nada; passo a passo nas notas
da release). Para compilar daqui (Rust estável, testado com 1.99 MSVC):

```powershell
cargo build --release -p overlay
.\target\release\Axon.exe
```

- O Windows pede permissão de administrador ao abrir (a captura por raw socket exige).
- A captura acompanha a rede: se o IP mudar (outra Wi-Fi, VPN ligada ou desligada, volta da
  suspensão), em até 2 s o endereço novo passa a ser lido e o que sumiu é fechado.
- Ao abrir, o Axon cria no Firewall do Windows a regra de entrada "Axon (captura)", só
  para o próprio exe, e apaga as regras de entrada antigas dele (inclusive o bloqueio que
  um "Cancelar" no aviso do Windows deixa). Sem ela, o firewall pode descartar o que chega
  do servidor e o medidor fica em "Procurando o servidor do jogo...". Antivírus com
  firewall próprio (Kaspersky, Avast, Norton...) não usam essa regra: aí é preciso liberar
  o `Axon.exe` nele. O rodapé avisa quando só sai tráfego e nada chega, e também quando o
  jogo está aberto há 2 minutos sem o servidor aparecer (VPN, ExitLag e similares).
- O jogo precisa estar em "tela cheia em janela" (borderless), que é o padrão atual.
- O Axon fica como ícone na área de notificação (a seta ao lado do relógio), fora da barra
  de tarefas. O overlay só aparece com o jogo em primeiro plano: aberto com outro programa
  na frente, ele espera invisível e surge quando você volta ao jogo. Clique esquerdo no
  ícone liga ou desliga o overlay; o direito abre o menu (Mostrar/Esconder e "Fechar
  Axon"). Escondido, o medidor continua contando. O jogo é achado pela janela (classe
  `UnrealWindow`, título "AION2"), sem abrir handle no processo dele.
- Cada jogador ocupa duas linhas: o nome em cima e `Classe · Nv · GS` embaixo (classe em
  inglês, por escolha; GS é o número que o jogo mostra como Power). `?` = ainda não
  chegou; `~31` = valor da memória, visto numa sessão anterior e talvez velho.
- Nas três abas aparecem só os 10 primeiros, cada um com a posição antes do nome. Se você
  está abaixo do 10º, a sua linha aparece embaixo, depois de um traço, com a sua posição
  (ex.: `37.`). Com "Só o meu dano", a posição mostrada é a real.
- **Atualização**: na abertura, o Axon consulta a última release do GitHub. Com versão
  nova, o rodapé mostra `v0.5.0 → v0.5.1` e o botão Atualizar, que baixa o `Axon.exe` da
  release, confere tamanho e SHA-256 (o `digest` que a API do GitHub publica), troca o exe
  (o atual vira `Axon.exe.old`, apagado na abertura seguinte) e reabre o Axon. Funciona a
  partir da 0.5.0; quem está numa versão anterior baixa a nova pela página de Releases.
- Cada aba tem uma tabela à direita de cada jogador. Expandido, cada skill que aconteceu
  aparece com as mesmas colunas (aí o % é a parte da skill no total do jogador):
  - **DPS**: `DPS | Damage(%) | CRIT | AVG | MAX`. Damage é o dano com a parte do jogador
    no total medido; CRIT, a fração de golpes críticos; AVG, o dano médio por golpe; MAX,
    o maior golpe.
  - **Tank**: `DTPS | Taken(%) | PARRY | CRIT | MAX`. Taken é o dano recebido com a parte
    do jogador; PARRY, a fração de golpes aparados (o único sinal de mitigação que o
    pacote traz; bloqueio de escudo e esquiva não têm marca conhecida); CRIT, críticos
    recebidos; MAX, o maior golpe levado. O AVG fica de fora: o golpe médio depende de
    qual monstro bateu em quem. Golpes, aparos e mortes aparecem ao passar o mouse.
  - **Healer**: `HPS | Heal(%) | CRIT | AVG | MAX`, as mesmas contas da DPS sobre a cura.
    O pacote não separa a sobrecura, então o HPS pode incluir cura que passou do HP cheio.
  - Proposta das colunas de Tank e Healer feita a partir do LOA Details (Tanked, T%, TPS),
    do Details! e do Recount (Damage Taken, Parry, Healing Done, HPS), cruzada com o que
    o pacote 0x3804 traz.
- A memória (`%LOCALAPPDATA%\Aion2Meter\jogadores.json`) guarda o último level e Power de
  cada nome, inclusive o seu, a cada 30 s e ao fechar. Com o overlay aberto no meio da
  sessão, você é reconhecido pelo nome no primeiro abate ou invocação. Valor lido na
  conexão atual sempre vence.
- Arraste a janela pelo fundo; ela não sai de cima do jogo (aberta em outro monitor, é
  trazida para dentro). Clique num jogador para ver as skills. "Zerar" começa
  uma luta nova; ela também zera sozinha depois de 15 s sem dano (ajustável).
- ⚙ abre as configurações, que valem na hora e ficam em
  `%LOCALAPPDATA%\Aion2Meter\config.json`:
  - **Linha de cada jogador**: escolha a aba e clique numa coluna da tabela ou em Classe,
    Nv ou GS na linha de amostra para esconder ou mostrar (o escondido fica riscado). Cada
    aba guarda as suas colunas; Classe, Nv e GS valem para as três.
  - **Só o meu dano**: só a sua linha, nas três abas; a % fica sempre em 100%. Enquanto
    você não foi reconhecido (ver Pendências), a lista fica vazia com um aviso.
  - **Alcance**: Proximidade (todos que aparecem perto). Party está desativado até uma
    captura em grupo mostrar o pacote do grupo.
  - **Luta**: segundos sem dano até a luta zerar, de 5 a 120.
  - **Tamanho**: de 60% a 200%, escalando a janela inteira.
  - **Transparência do fundo**: de 0% a 90%, em passos de 5% (padrão 15%). Texto, barras e
    borda continuam opacos.
- ‹ ou › (a seta aponta para a borda mais perto) desliza a janela até a borda do jogo e
  deixa só a aba "Overlay"; clicar nela traz a janela de volta ao
  mesmo lugar. Recolhido, o medidor continua contando. Com as animações do Windows
  desligadas, a janela vai direto, sem deslizar.
- O rodapé mostra a versão e só avisa enquanto procura o servidor do jogo, enquanto baixa
  os nomes das skills ou quando a captura para.
- O dano é o número que o jogo mostra ao bater (o campo do pacote, sem conversão). Até a
  0.5.1 ele era multiplicado por 18,82 (HP do mob, ver PROTOCOLO.md §6) e não batia com a tela.
- Invocações, pets e armadilhas somam na linha do dono.
- Skills aparecem em português e com ícone. Nomes: questlog.gg (base comunitária montada
  do cliente Global, API não documentada); ícones: CDN oficial da NCSoft. Tudo fica em
  `%LOCALAPPDATA%\Aion2Meter` (`skills-pt.json` e `icones/`); apague a pasta para rebaixar.
- Clicar no overlay não tira o foco do teclado do jogo (`WS_EX_NOACTIVATE`, reaplicado a
  cada quadro porque o winit apaga o bit ao mostrar a janela; conferido no jogo em
  2026-10-02, junto com expandir jogador). O arraste é próprio desde a 0.3.0 (cursor +
  `SetWindowPos`): o `StartDrag` do winit ignora arrastes enquanto não vê o fim do
  anterior, e um arraste que não entra no laço de mover do Windows podia travar os seguintes.
- Rede: a captura é passiva; as requisições próprias do medidor estão em
  [Privacidade](#privacidade).
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

## Avisos do Windows e do navegador

O `Axon.exe` ainda não tem assinatura digital. Sem ela, o navegador e o Windows desconfiam
de exe novo até muita gente baixar aquele mesmo arquivo, e cada versão recomeça do zero.

- **Ao baixar** (Chrome, Edge): o aviso de arquivo pouco baixado ou perigoso tem a opção de
  manter o arquivo, às vezes dentro do menu `⋯` do download. Escolha manter.
- **Ao abrir**, "O Windows protegeu o computador": clique em **Mais informações** e depois
  em **Executar assim mesmo**.
- **"O Controle Inteligente de Aplicativos bloqueou"** (Windows 11): esse recurso não deixa
  liberar um programa só. O Axon só passa nele quando o exe tiver assinatura.
- Para conferir que o arquivo é o da release: `Get-FileHash .\Axon.exe` no PowerShell e
  compare com o SHA-256 que a página da release mostra ao lado do `Axon.exe`.
- O botão Atualizar grava o exe novo sem a marca de "baixado da internet", que é o que faz
  o Windows mostrar o "O Windows protegeu o computador": o aviso fica na primeira instalação.

## Privacidade

A captura só lê: nada do tráfego do jogo sai do seu PC. Nome de personagem, dano, level e o
resto do que o overlay mostra não são enviados a lugar nenhum. O Axon faz só estas
requisições próprias, todas HTTPS, com o user-agent `Aion2Meter/<versão> (medidor de DPS
pessoal)`:

| Para onde | Quando | O que vai no pedido |
|---|---|---|
| `questlog.gg` | Com o cache vazio, uma listagem por classe; depois, uma vez por skill que ainda não está no cache | Nome da classe ou código da skill |
| `assets.playnccdn.com` (CDN da NCSoft) | Uma vez por ícone que ainda não foi baixado | Nome do arquivo do ícone |
| `api.github.com` | A cada abertura | Pedido da última release deste repositório |
| `github.com` e o servidor de arquivos do GitHub | Só ao clicar em Atualizar | Pedido do `Axon.exe` da release |

O que é baixado fica em `%LOCALAPPDATA%\Aion2Meter`.

## Licença

MIT, ver [LICENSE](LICENSE). Do RATmeter (GPL-3.0) vieram só fatos sobre o protocolo, como
números de opcode e códigos de skill de cura; o código deste repositório foi escrito a
partir do formato descrito em [PROTOCOLO.md](PROTOCOLO.md). O `dados/skills.json`, que é do
RATmeter, continua fora do repositório e das releases.

## Estrutura

| Pasta | Conteúdo |
|---|---|
| `crates/nucleo` | Protocolo (varint, LZ4, framing, parsers), captura (raw socket, pcapng, remontagem TCP), medição (placar, catálogo de skills) e formatação pt-BR |
| `crates/nucleo/tests` | LZ4 contra o `lz4_flex`, framing, remontagem TCP, parsers com bytes reais, placar e troca de servidor (`cargo test`). O teste de rede do catálogo é opcional: `cargo test -p nucleo --test catalogo -- --ignored` |
| `crates/overlay` | Janela sempre no topo (egui/eframe), gera o `Axon.exe` (ícone de `assets/axon.ico`, embutido pelo `build.rs`). O build de debug abre sem administrador e aceita `cargo run -p overlay -- --replay captura.pcapng [--tank] [--expandir] [--config] [--zoom 1.3] [--recolher \| --recolher-e-voltar] [--posicao x y] [--limite N] [--nova-versao]` para ver a janela sem o jogo (no replay, a config é lida mas não é gravada) |
| `crates/overlay/assets` | `logo-axon.jpg` (a logo original) e `axon.ico`, o hexágono recortado dela com fundo transparente, de 16 a 256 px |
| `crates/replay` | Replay de `.pcapng` com diagnóstico e modo `ao-vivo` no console |
| `dados/skills.json` | Opcional, fora do repositório e das releases: nomes em inglês do RATmeter (GPL-3.0, ver PROTOCOLO.md §9), só reserva quando o questlog não tem a skill |
| `capturar.ps1` | Grava `captura.pcapng` com o pktmon do Windows (admin). Capturas ficam fora do repositório: têm o seu tráfego |
| `ao-vivo.ps1` | Roda o modo `ao-vivo` do replay como admin e grava `ao-vivo-log.txt` (antes: `cargo build --release -p replay`) |
| `lancar.ps1` | Ensaio e publicação de uma release a partir deste PC (ver "Lançar uma versão") |
| `.github/workflows/release.yml` | Tag `v*` → testes → `Axon.exe` → release com o zip (parado enquanto o Actions estiver travado) |

## Lançar uma versão

1. Ajuste `version` em `[workspace.package]` do `Cargo.toml` (SemVer: funcionalidade nova
   sobe a casa do meio e zera a última, 0.2.0 → 0.3.0; correção sobe a última, 0.3.0 →
   0.3.1). A versão aparece no rodapé do overlay.
2. Mescle no `main`.
3. Ensaio: `powershell -ExecutionPolicy Bypass -File lancar.ps1 -Versao 0.3.0`. Se passar,
   o mesmo comando com `-Publicar`.

Todo exe de release sai do `lancar.ps1`, sempre de um clone limpo do GitHub numa pasta nova em
`%TEMP%\axon-lancamento`. Ele para no primeiro problema e:

- confere que a release ainda não existe, que a versão é maior que a última e bate com o
  `Cargo.toml`, e que nenhuma captura (`.pcapng`, `.etl`) nem o `dados/skills.json` está no git;
- roda `cargo test --workspace --locked` e `cargo build --release -p overlay --locked`;
- confere o `Axon.exe` sem abri-lo: versão e produto nos dados do exe, tamanho abaixo do limite
  do botão Atualizar, manifesto pedindo administrador;
- monta `Aion2Meter-0.3.0-win-x64.zip` só com o `Axon.exe` e confere que é o mesmo arquivo;
- com `-Publicar`, cria a tag no commit testado, publica a release com o zip, o `Axon.exe`
  solto (o que a atualização pelo rodapé baixa) e `.github/notas-da-release.md`, baixa tudo de
  volta e confere o SHA-256 com o `digest` da API do GitHub e que ela virou a Latest.

O lançamento é local porque o Actions deste repositório está parado desde 2026-10-02 (conta
travada por cobrança: "account is locked due to a billing issue"), e o dono decidiu em
2026-10-03 não liberar a cobrança por enquanto. O `.github/workflows/release.yml` (tag `v*` →
testes → release) fica para quando o Actions voltar; aí a tag sozinha publica, e o
`lancar.ps1` só serve para ensaio.

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
- **DoT** (`0x3805`): layout não conferido (Ranger não gerou nenhum).
- **Healer não conferido**: nenhuma captura teve curandeiro, e a escala 1:1 da cura é
  suposição. Golpe de jogador em mob com skill que o questlog marca como cura (dreno, por
  exemplo) sai do DPS e entra no Healer. Só mexer nessa regra quando uma captura mostrar
  uma skill assim.
- **Tank**: escala 1:1 conferida num golpe só. "aggro" é inferido do último golpe de
  cada mob, porque o pacote de troca de alvo não foi achado.
- Invocação de jogador sem legião: o vínculo depende só do nome do dono.
- **Firewall**: a regra automática não foi conferida num PC com o Firewall do Windows
  ativo (o PC de desenvolvimento usa o firewall do Kaspersky).
- **Party** (só quem está no seu grupo): o pacote do grupo (`0x9702` em outros medidores)
  não apareceu em nenhuma captura local; falta uma captura em grupo.

## Conferido

- Crítico: dentro da mesma skill, `tipo_dano 3` tem média 1,6 vez maior que o tipo 2
  (Disparo Rápido: 384 contra 238), então a leitura de crítico está certa. O 0% do teste
  ao vivo (153 golpes) é dado real daquela luta.
- Port de C# para Rust (0.1.0 → 0.2.0): o replay em Rust deu saída idêntica à do CLI em
  C# nas 5 capturas locais, inclusive nas opções de diagnóstico.
