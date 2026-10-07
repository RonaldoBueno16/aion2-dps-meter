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
- Na primeira abertura (e depois de mover o exe de pasta), o overlay pede para liberar o
  Axon no Firewall do Windows. **Liberar** cria a regra de entrada "Axon (captura)", só
  para o próprio exe, e apaga as regras de entrada antigas dele (inclusive o bloqueio que
  um "Cancelar" no aviso do Windows deixa); **Agora não** começa a medir sem mexer no
  firewall, e o pedido volta na próxima abertura. A captura só começa depois do clique,
  porque a regra só vale para socket aberto depois dela. Sem a regra, o firewall pode
  descartar o que chega do servidor e o medidor fica em "Procurando o servidor do
  jogo...". Para desfazer: Firewall do Windows > Configurações avançadas > Regras de
  Entrada. Antes da 0.6.0 a regra era criada sem perguntar. Antivírus com
  firewall próprio (Kaspersky, Avast, Norton...) não usam essa regra: aí é preciso liberar
  o `Axon.exe` nele. O rodapé avisa quando só sai tráfego e nada chega, e também quando o
  jogo está aberto há 2 minutos sem o servidor aparecer (VPN, ExitLag e similares).
- O jogo precisa estar em "tela cheia em janela" (borderless), que é o padrão atual.
- O Axon fica como ícone na área de notificação (a seta ao lado do relógio), fora da barra
  de tarefas. Desde a 0.12.0 o overlay fica visível com o jogo na frente, atrás de outra
  janela, minimizado ou fechado; para tirá-lo da tela, use o ícone (ou o atalho, com o jogo
  na frente). Clique
  esquerdo no ícone liga ou desliga o overlay; o direito abre o menu (Mostrar/Esconder, "Clique
  atravessa o overlay" e "Fechar Axon"). Escondido, o medidor continua contando. O jogo é
  achado pela janela (classe `UnrealWindow`, título "AION2"), sem abrir handle no processo dele.
- **Atalhos** (desde a 0.8.0): `Ctrl+H` mostra ou esconde o overlay; `Ctrl+T` liga ou
  desliga o clique atravessando o overlay até o jogo (o rodapé avisa enquanto está ligado, e
  o item do menu da bandeja também desliga). Os padrões vêm do medidor do TK e não foram
  conferidos contra os atalhos do jogo. Ficam registrados (`RegisterHotKey`) só com o jogo
  em primeiro plano: aí a combinação não chega a nenhum outro programa, nem ao jogo; fora
  dele, volta ao normal (`Ctrl+T` abre aba no navegador). Há mais dois, que vêm desligados:
  `atalho_resumo` copia o resumo da luta e `atalho_compacta` liga ou desliga a barra
  compacta. Para trocar, feche o Axon e edite `atalho_mostrar`, `atalho_atravessar`,
  `atalho_resumo` e `atalho_compacta` no `config.json` (ex.: `"Ctrl+Shift+F9"`; A-Z,
  0-9 ou F1-F24 com Ctrl, Alt ou Win; vazio desliga). O overlay não recebe teclado (não
  tira o foco do jogo), por isso a troca não é pela tela.
- Visual (desde a 0.8.0, inspirado no medidor do TK): cabeçalho com o logo e botões de
  ícone, a barra do alvo, as abas e uma linha por jogador. A linha tem o medalhão da classe (o
  emblema oficial `UT_Class_*_Large`, da CDN da NCSoft, o mesmo que o questlog e o medidor Abyss
  usam, tingido na cor da classe e com anel da mesma cor; até a 0.7.1 era o ícone da primeira
  skill da classe), a posição, o
  nome, uma barra em degradê na cor da classe, proporcional ao primeiro, e três números:
  total, por segundo e %. A sua linha tem borda dourada. Ao passar o mouse aparecem
  `Classe · Nv · GS` (classe em inglês, por escolha; GS é o número que o jogo mostra como
  Power; `?` = ainda não chegou; `~31` = valor da memória, visto numa sessão anterior e talvez
  velho) e os números que não cabem na linha; clicar abre a ficha com os mesmos dados e as skills.
- **Alvo** (desde a 0.8.0), acima das abas: na guerra com chefe, o chefe, mesmo que você
  bata em outro mob; sem chefe, o último mob em que você bateu (o alvo selecionado no jogo vai
  do cliente para o servidor e não chega ao Axon, então selecionar sem bater não muda a
  barra); com você ainda não reconhecido, o mob que mais apanhou. Mostra o retrato (quando o
  questlog tem; senão ⚔), o nome e o level em português, o HP como o servidor manda
  (`0x8D00`, outra escala que a do dano: PROTOCOLO.md §6), o seu dano e o do grupo nele.
  O HP máximo vem do pacote de criação do mob (`0x3641`, PROTOCOLO.md §7c): com ele, o card
  mostra também o % de HP, uma barra fina de HP no pé e "derrota em", quanto falta para o HP
  zerar na velocidade em que ele caiu nos últimos 30 s (aparece com 5 s de leituras e o HP
  caindo).
- **Mate em** (desde a 0.13.0): quando o servidor manda um prazo para matar o alvo junto com a
  entrada dele em combate (`0x8D21`, PROTOCOLO.md §5b), o card e a barra compacta mostram
  "mate em 4:32", contando até o prazo. Fica vermelho quando o "derrota em" passa do que
  falta (no ritmo atual, não dá tempo); some quando ele sai de combate ou morre. Visto num
  chefe só, em 2026-10-06: duas entradas em combate, cada uma com o prazo 300 s à frente. O
  prazo vem na hora do servidor e é comparado com a do Windows, como o renascer dos chefes de
  campo: com o relógio do PC errado, o tempo sai errado na mesma medida (na captura, os dois
  bateram com 0,3 s de diferença).
- **Guerra com chefe** (desde a 0.8.0): quando um mob da luta é chefe, a aba DPS conta só o
  dano no chefe, e o tempo conta do primeiro golpe nele; o selo "⚔ só no chefe" aparece ao lado
  das abas. Num evento de world boss, os golpes nos mobs em volta não inflam o placar. Sem
  chefe, a aba DPS soma o dano em todos os mobs. Tank e Healer não mudam. Chefe é o mob que o
  questlog marca como herói, lendário ou nomeado (world boss e chefe de dungeon, como Kromede e
  Bakarma, vêm assim; elite e invocação ficam de fora), achado pelo código do NPC no pacote de
  criação (`0x3641`). Esse pacote chega quando o mob aparece na tela: com o Axon aberto depois
  disso, sobra só a reserva de 30 ou mais jogadores diferentes batendo no mesmo mob, que pega
  world boss e não pega chefe de dungeon. Abra o Axon antes de entrar na dungeon ou de chegar ao
  evento. Morto, a barra fica marrom com "Derrotado". Sem o questlog, o nome aparece como
  `NPC 2400425`; sem o pacote de criação, `Chefe #id` (reconhecido pela multidão) ou `Alvo #id`,
  e só o HP atual, sem % nem barra.
- Nas três abas aparecem só os 10 primeiros, cada um com a posição antes do nome. Se você
  está abaixo do 10º, a sua linha aparece embaixo, depois de um traço, com a sua posição
  (ex.: `37`). Com "Só o meu dano", a posição mostrada é a real.
- **Atualização**: na abertura, o Axon consulta a última release do GitHub. Com versão
  nova, o rodapé mostra `v0.5.0 → v0.5.1` e o botão Atualizar, que baixa o `Axon.exe` da
  release, confere tamanho e SHA-256 (o `digest` que a API do GitHub publica), troca o exe
  (o atual vira `Axon.exe.old`, apagado na abertura seguinte) e reabre o Axon. Funciona a
  partir da 0.5.0; quem está numa versão anterior baixa a nova pela página de Releases.
  Desde a 0.7.0, sem versão nova, o rodapé mostra o botão "Verificar atualização", que faz a
  mesma consulta sem reabrir o Axon e diz o resultado: `é a versão mais nova`, `o GitHub não
  respondeu` (sem internet ou acima do limite de 60 consultas por hora da API) ou a versão
  nova com o Atualizar. A consulta da abertura continua calada quando não há novidade.
- Cada aba mostra três números por jogador, com a legenda acima das linhas. Expandido, cada
  skill que aconteceu aparece com os mesmos três números (aí o % é a parte da skill no total
  do jogador) e uma barra fina pela parte; o mouse na skill mostra golpes, CRIT, AVG e MAX.
  Até a 0.7.1 eram cinco colunas por aba: CRIT, AVG e MAX foram para o mouse e para a ficha.
  Desde a 0.8.0, o por segundo (DPS, DTPS, HPS) divide o total de cada jogador pelo tempo
  dele: do primeiro ao último golpe (ou cura, ou golpe recebido) dele na luta, no mínimo 1 s.
  Quem chega no meio ou morre cedo não tem o número diluído pelo tempo em que não estava lá.
  A skill expandida divide pelo mesmo tempo do jogador (as skills somam o DPS dele), e o
  relógio do rodapé continua sendo o tempo da luta. Até a 0.7.1 todos dividiam pelo tempo da
  luta. É o aDPS do Abyss DPS Meter, que vem com o eDPS (tempo da luta) ligado: para comparar
  os dois, mude o cálculo de DPS do Abyss para aDPS.
  - **DPS**: `Dano | DPS | %`. Na ficha e no mouse: CRIT (fração de golpes críticos), AVG
    (dano médio por golpe) e MAX (maior golpe); no mouse, também a fração pelas costas do
    alvo (byte de direção do pacote). Perfeito e dano duplo ficam de fora até serem
    conferidos na tela (PROTOCOLO.md §4).
  - **Tank**: `Recebido | DTPS | %`. Na ficha e no mouse: PARRY, a fração de golpes
    aparados (o único sinal de mitigação que o pacote traz; bloqueio de escudo e esquiva não
    têm marca conhecida), CRIT recebidos e MAX, o maior golpe levado; no mouse, também golpes,
    aparos e mortes. O AVG fica de fora: o golpe médio depende de qual monstro bateu em quem.
  - **Healer**: `Cura | HPS | %`, com CRIT, AVG e MAX como na DPS. O pacote não separa a
    sobrecura, então o HPS pode incluir cura que passou do HP cheio.
  - Proposta das medidas de Tank e Healer feita a partir do LOA Details (Tanked, T%, TPS),
    do Details! e do Recount (Damage Taken, Parry, Healing Done, HPS), cruzada com o que
    o pacote 0x3804 traz.
  - **Buffs recebidos** (desde a 0.8.0), embaixo das skills do jogador expandido: os 8
    buffs que ficaram mais tempo ativos nele, com a parte da luta em que cada um esteve
    ativo. O pacote não traz nome de buff: o nome e o ícone são os da skill que dá o buff
    (código do buff / 10, que bateu em 74% das aplicações de classe). Efeitos da mesma skill,
    e o mesmo buff vindo de jogadores diferentes, contam juntos (nunca passam de 100%). Ficam
    de fora buff permanente, instantâneo e efeito que não vem de skill de classe.
- A memória (`%LOCALAPPDATA%\Aion2Meter\jogadores.json`) guarda o último level e Power de
  cada nome, inclusive o seu, a cada 30 s e ao fechar. Com o overlay aberto no meio da
  sessão, você é reconhecido pelo nome no primeiro abate ou invocação. Valor lido na
  conexão atual sempre vence.
- Arraste a janela pelo fundo; ela não sai de cima do jogo (aberta em outro monitor, é
  trazida para dentro). Clique num jogador para ver as skills. ↺ (Zerar) começa
  uma luta nova. Ela também acaba sozinha depois de 15 s sem dano (ajustável) e, desde a
  0.8.0, quando todos os mobs dela saem de combate (pacote 0x8D21 ou morte; mob sem golpe
  nem entrada em combate há 5 s não segura a luta) ou quando o último chefe dela morre, mesmo
  com gente ainda batendo nos mobs em volta. O placar da luta que acabou fica na tela
  até o próximo golpe entre jogador e mob; cura e morte de jogador depois do fim não abrem
  luta nova (o grupo se cura depois do último mob). O que chega até 1 s depois do fim ainda
  conta na luta que acabou.
- **Lutas** (desde a 0.8.0), ☰ no cabeçalho: as 20 últimas lutas desta execução (só na
  memória; fechar o Axon apaga), com hora, duração, dano total, quem mais bateu e o alvo.
  Clique numa para ver o placar dela no lugar da luta de agora, com o alvo, as abas, as
  skills e os buffs; o rodapé vira "Luta das 14:32" e o ↺ dá lugar ao "● Ao vivo", que volta
  à luta de agora.
- **Copiar resumo** (desde a 0.8.0), ⧉ no cabeçalho: copia para a área de transferência o
  alvo, a duração, o total e os 10 primeiros da aba DPS (mais você, se ficou abaixo), cada um
  com posição, nome, DPS e %, para colar no chat do jogo. Sai numa linha só, ou um jogador por
  linha com "Resumo em linhas". O rodapé mostra "Resumo copiado" por 3 s.
- **Barra compacta** (desde a 0.8.0), ▭ no cabeçalho: o medidor vira uma linha com o % de HP
  e o "derrota em" do alvo (e o "mate em", quando há prazo), o nome dele, o seu DPS, o do grupo (dano total da aba DPS dividido
  pelo tempo da luta) e o ping. O ▭ da barra volta ao medidor completo. Fica salva no
  `config.json`.
- **Bosses** (desde a 0.11.0), ♛ no cabeçalho: os chefes de campo da região em que você está,
  nas abas Vivos e Mortos (abre em Mortos). O servidor manda a lista a cada poucos segundos,
  com o mapa aberto ou não (PROTOCOLO.md §7e), e ela traz só vivo ou morto e uma hora. Morto:
  a hora de renascer (`22:20`, ou `ter 22:20` se não for hoje) e a contagem, dourada faltando
  até 10 min, do que renasce antes ao que renasce depois. Vivo: desde quando (a hora marcada;
  em 2026-10-05 um chefe nasceu 2 min antes dela), do level mais alto ao mais baixo. Fora da
  região o servidor para de mandar: depois de 30 s sem lista aparece "Lista das 17:22", e quem
  passou da hora de renascer vai para Vivos como provável. Com a lista em dia, o chefe que
  passou da hora e o servidor ainda diz morto fica em `00:00`. Nome, level e retrato vêm do
  questlog (`getRegion`): o pacote traz só o número do chefe, e o 21º é o 21º NPC da região
  em ordem de código, conferido em 3 dos 24 de Altgard (Gartua Imortal, Profanador Newbold e
  Arconte da Alma Perdida Axios); se a contagem não bater, aparece "Chefe 21". A posição dos
  vivos vem no pacote e não aparece.
- **Drops** (desde a 0.11.0): clicar num chefe na tela Bosses abre, ao lado da janela, o que ele
  deixa cair segundo o questlog (`getNpc`, e `getItem` para o que vem no baú). O painel abre do
  lado com espaço na área (do jogo ou do monitor); à esquerda, a janela anda e o medidor fica no
  lugar. O ✕, o
  mesmo chefe de novo, recolher ou a barra compacta fecham. Em cima, o retrato, o nível e quando
  renasce; embaixo, nesta ordem: o conjunto do chefe (as peças com o nome dele, lado a lado na
  cor da raridade, com a % de cada uma), o que cai sempre (o baú de saque, que abre e mostra o
  que vem dentro), o que mais cai sem ser equipamento (materiais, a obra-prima do chefe) e os
  outros equipamentos por raridade ("Único   32 itens   0,036% a 0,16% cada", que abre a lista).
  As peças do conjunto somam 100% no questlog nos 24 chefes de Altgard (7 peças verdes ou azuis
  nos de nível 45; 9 douradas, com acessórios, nos de 48 e 51): o painel diz "deve cair uma por
  morte", o que ainda não foi conferido no jogo. A raridade aparece na cor do questlog (a mesma
  do jogo); o nome só onde foi conferido num item do jogo: Único (dourado, 41) e Especial
  (ciano, 71). Nas outras, um quadrado na cor. A % é a do questlog, sem conta nossa por cima.
  Clicar num item (uma peça do conjunto ou uma linha) troca a lista pela ficha dele, como a
  dica do jogo: nível do item, atributos (com o nome que o jogo dá em português, pelo
  `statFormat` do questlog, e a ajuda de cada um no mouse), os atributos ao vincular a alma
  (fixos, ou "4 sorteados destes" com a faixa de cada um), encaixes de Pedra de Mana e Pedra
  Divina, o texto de ajuda do item, o nível mínimo de uso e a chance naquele chefe; "‹ Drops"
  volta. Conferido com a dica do jogo no Guarda-braço da Alma Forjada (Ataque 92, Precisão 50,
  Acerto Crítico 50; Poder 10, Precisão 30, Bloqueio 15, PV 120; nível 45).
  Nada disso vai para o disco: drops, fichas, região, retratos e ícones ficam na memória enquanto
  o Axon está aberto (4 imagens baixando por vez, com um cinza pulsando no lugar até chegar).
- ⚙ abre as configurações, que valem na hora e ficam em
  `%LOCALAPPDATA%\Aion2Meter\config.json`:
  - **Números de cada jogador**: clique no total, no por segundo ou na % da linha de
    amostra para esconder ou mostrar (o escondido fica riscado); vale nas três abas. Na
    0.8.0 substituiu as colunas por aba e Classe/Nv/GS, que foram para o mouse e a ficha (o
    `config.json` antigo continua abrindo).
  - **Só o meu dano**: só a sua linha, nas três abas; a % fica sempre em 100%. Enquanto
    você não foi reconhecido (ver Pendências), a lista fica vazia com um aviso.
  - **Alcance**: Proximidade (todos que aparecem perto). Party está desativado até uma
    captura em grupo mostrar o pacote do grupo.
  - **Luta**: segundos sem dano até a luta zerar, de 5 a 120.
  - **Exibição** (desde a 0.8.0): "Ocultar nomes" troca o nome dos outros jogadores pelo da
    classe (ou "Jogador", com a classe ainda desconhecida) no medidor, nas lutas anteriores e
    no resumo copiado; o seu continua. "Resumo em linhas" muda o formato do resumo copiado.
    "Atualizar a cada": de quanto em quanto o placar é relido, de 100 a 1000 ms em passos de
    100 (padrão 500).
  - **Tamanho**: de 60% a 200%, escalando a janela inteira.
  - **Transparência do fundo**: de 0% a 90%, em passos de 5% (padrão 15%). Texto, barras e
    borda continuam opacos.
  - **Atalhos**: mostra os quatro atalhos, ou avisa que um está desligado, em uso por outro
    programa ou que o texto do `config.json` não vale.
- ‹ ou › (a seta aponta para a borda mais perto) desliza a janela até a borda do jogo e
  deixa só a aba "Overlay"; clicar nela traz a janela de volta ao
  mesmo lugar. Recolhido, o medidor continua contando. Com as animações do Windows
  desligadas, a janela vai direto, sem deslizar.
- O rodapé mostra o estado da luta (bolinha verde e "Em luta" enquanto o placar anda,
  "Aguardando" depois de 5 s parado), o total da aba, o ping e o tempo da luta. O ping (desde
  a 0.8.0) é o menor tempo de ida e volta até o servidor do jogo nos últimos 10 s, medido no
  TCP (PROTOCOLO.md §8b), com três barrinhas: verde até 60 ms, amarelo até 120, laranja até
  200 e vermelho acima. É a latência da rede; o número que o jogo mostra pode sair um pouco
  maior. Depois do total vem a Energia Odyle (desde a 0.8.0): o cristal do item Energia Odyle
  (`Icon_Item_Odenergy_A_001`, da CDN; "Odyle" escrito enquanto não baixou) e `550 (+270)`, a
  básica e a carregada como o servidor manda no login e a cada mudança, como o uso de essência
  OD (PROTOCOLO.md §7d), sem o máximo, que não vem no pacote. Com o Axon aberto depois do login,
  ela aparece na próxima mudança ou no próximo login. A Odyle só aparece se couber antes do
  ping. Na 0.9.0 a Fenda Espaço-Temporal também ficava no rodapé; desde a 0.10.0 ela fica nos
  eventos, logo abaixo. Embaixo de tudo ficam a versão e o botão de atualização, e os avisos
  enquanto procura o servidor do jogo, enquanto baixa os nomes das skills ou quando a captura
  para.
- Os eventos (desde a 0.10.0) ficam embaixo do rodapé, contando a cada segundo pelo relógio do
  PC, no horário de Brasília (servidor SA). Recolhidos, numa linha, os próximos que couberem
  inteiros, na ordem em que acontecem: o ícone de cada um e a contagem (`41:20`, `1:27:19`),
  e o nome nos resets, que não têm ícone; o mouse sobre a linha diz o nome de cada um. O clique
  na linha mostra todos, um por linha, com o ícone, o nome, o início (`21:00`, ou `qui 21:00`
  se não for hoje) e a contagem, e esconde de novo; a escolha fica na config. Aberto, o evento
  vai para o começo, com `fecha em 02:41` em verde (e a bolinha verde na lista); faltando até
  10 min, a contagem fica dourada. O mouse sobre cada linha da lista explica a regra. Nas noites
  de cerco, perto das 21h e das 21h30, com dois eventos abertos, a linha recolhida pode ficar sem
  espaço para a Fenda, que continua na lista. São 9: Fenda Espaço-Temporal (a cada 3 h, às 02h, 05h,
  ..., 23h; portal aberto 10 min), Shugo Festa (toda hora cheia; 3 min para entrar), Invasão
  Dimensional (toda hora, aos 30 min; 3 min para entrar), Vigilante Kairah (a cada 3 h, às 01h,
  04h, ..., 22h; 30 min; horário não confirmado), Cerco de Artefato (segunda, quinta e sábado às
  21h), Chefes do Cerco (os mesmos dias, às 21h30), Senhor Guardião Nahma (sexta e domingo às
  21h) e os resets diário (04h) e semanal (quarta às 04h). A NCSoft não publicou os horários do
  SA: a tabela é a do shugo.gg, conferida com metabot.gg e aion2rifttimer.com em 2026-10-06
  (`crates/overlay/src/eventos.rs`). A Arena of Tactics ficou de fora: só o metabot.gg traz o
  horário dela. Desde a 0.11.0 os chefes de campo mortos da região entram também: na linha
  recolhida, junto com os eventos pela contagem (o retrato e o tempo até renascer); na lista,
  só a linha "Chefes de Altgard   20 vivos, 4 mortos" depois dos eventos, e o clique nela abre a
  tela Bosses com cada chefe.
- Nomes e ícones dos eventos: Vigilante Kairah e Senhor Guardião Nahma são os nomes dos NPCs em
  português no questlog; Shugo Festa vem dos itens dela (Chave de Recompensa da Shugo Festa) e a
  Fenda, do Bilhete de Entrada da Fenda Espaço-Temporal. Invasão Dimensional, Cerco de Artefato,
  Chefes do Cerco e os resets são traduções nossas. Os ícones vêm da CDN do jogo, como os
  emblemas das classes, e ficam no cache dos ícones: o bilhete da Fenda, o retrato do Shugo
  Gerente do Festival, o Baú de Mérito Dimensional (Invasão), os retratos da Kairah, do Executor
  Argo (Chefes do Cerco) e do Nahma, recortados no rosto para caber em 16 px, e a Prova de Herói:
  Reshanta Inferior (Cerco). Os resets mostram ↻. Enquanto o ícone não baixou, aparece só a
  moldura.
- O dano é o número que o jogo mostra ao bater (o campo do pacote, sem conversão). Até a
  0.5.1 ele era multiplicado por 18,82 (HP do mob, ver PROTOCOLO.md §6) e não batia com a tela.
  Até a 0.7.1, golpe de mob com a flag 0x20 saía com dano 10.000 no Tank (27 golpes no world
  boss de 2026-10-03; o campo antes do dano é varint e era lido como 1 byte).
- Invocações, pets e armadilhas somam na linha do dono, inclusive o espírito do Elementalist e
  os golpes que chegam antes do pacote de criação da invocação.
- Skills aparecem em português e com ícone. Nomes: questlog.gg (base comunitária montada
  do cliente Global, API não documentada); ícones: CDN oficial da NCSoft. Nome, level e
  retrato dos mobs vêm do mesmo jeito (`getNpc` do questlog). Tudo fica em
  `%LOCALAPPDATA%\Aion2Meter` (`skills-pt.json`, `npcs-pt.json` e `icones/`); apague a pasta
  para rebaixar. A consulta é uma a cada 400 ms, e nome e retrato de mob passam na frente das
  skills: num world boss, centenas de skills entram na fila antes do chefe.
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
    Golpes de monstro aparecem como `Golpe 1235330 · Arconte da Alma Perdida Axios`, com o
    retrato do mob quando houver: nenhuma base pública tem nome de skill de mob (o questlog
    só tem skills de jogador, e a tabela do RATmeter cobre 9% dos golpes de mob das
    capturas, quase todos como "Attack"), então o golpe leva o nome do mob que o usou. Sem
    o mob identificado, fica "Golpe de monstro (código)".
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
| `questlog.gg` | Com o cache vazio, uma listagem por classe; depois, uma vez por skill ou mob que ainda não está no cache; a cada execução, uma vez por região (chefes de campo), ao abrir os drops de um chefe ou a ficha de um item, e uma vez a lista dos nomes de atributo (`statFormat`) | Nome da classe, código da skill, do NPC, da região, do baú de saque ou do item; o idioma |
| `assets.playnccdn.com` (CDN da NCSoft) | Uma vez por ícone (skill, emblema de classe, Odyle, evento) ou retrato de mob que ainda não foi baixado; a cada execução, uma vez por retrato de chefe de campo e ícone de item dos drops | Nome do arquivo do ícone |
| `api.github.com` | A cada abertura | Pedido da última release deste repositório |
| `github.com` e o servidor de arquivos do GitHub | Só ao clicar em Atualizar | Pedido do `Axon.exe` da release |

O cache das skills, dos mobs e dos ícones fica em `%LOCALAPPDATA%\Aion2Meter`. Os chefes de
campo, os drops e as imagens deles ficam só na memória e somem ao fechar o Axon.

## Licença

MIT, ver [LICENSE](LICENSE). Do RATmeter (GPL-3.0) vieram só fatos sobre o protocolo, como
números de opcode e códigos de skill de cura; o código deste repositório foi escrito a
partir do formato descrito em [PROTOCOLO.md](PROTOCOLO.md). O `dados/skills.json`, que é do
RATmeter, continua fora do repositório e das releases. Do [Aion2-Dps-Meter do
TK](https://github.com/TK-open-public/Aion2-Dps-Meter) (MIT, cliente coreano) vieram a pista
dos opcodes 0x8D21 e 0x382A/B/C e a ideia do histórico de lutas e dos atalhos; o formato foi
conferido de novo em capturas do cliente global (o layout de dano dele não vale aqui) e o
código foi escrito do zero.

## Estrutura

| Pasta | Conteúdo |
|---|---|
| `crates/nucleo` | Protocolo (varint, LZ4, framing, parsers), captura (raw socket, pcapng, remontagem TCP), medição (placar, catálogo de skills) e formatação pt-BR |
| `crates/nucleo/tests` | LZ4 contra o `lz4_flex`, framing, remontagem TCP, parsers com bytes reais, placar e troca de servidor (`cargo test`). O teste de rede do catálogo é opcional: `cargo test -p nucleo --test catalogo -- --ignored` |
| `crates/overlay` | Janela sempre no topo (egui/eframe), gera o `Axon.exe` (ícone de `assets/axon.ico`, embutido pelo `build.rs`). O build de debug abre sem administrador e aceita `cargo run -p overlay -- --replay captura.pcapng [--tank] [--expandir] [--config] [--zoom 1.3] [--recolher \| --recolher-e-voltar] [--posicao x y] [--limite N] [--nova-versao] [--pedir-firewall] [--lutas] [--chefes] [--drops N] [--ate S]` para ver a janela sem o jogo (no replay, a config é lida mas não é gravada; `--lutas` separa as lutas como ao vivo e abre a tela Lutas; `--chefes` abre a tela Bosses; `--drops 2400424` abre o painel de drops do NPC dado; `--ate 90` lê só os 90 primeiros segundos da captura, para ver a janela no meio de uma luta) |
| `crates/overlay/assets` | `logo-axon.jpg` (a logo original) e `axon.ico`, o hexágono recortado dela com fundo transparente, de 16 a 256 px |
| `crates/replay` | Replay de `.pcapng` com diagnóstico e modo `ao-vivo` no console; `--lutas` lista as lutas como o overlay ao vivo as separa |
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
3. Confira: "sincronizado=True", eventos de dano válidos > 0, "Razão queda/dano" estável
   em cada alvo (o fator muda de mob para mob, de 4,5 a 18,82 nas capturas; PROTOCOLO.md §6)
   e o placar no fim. Se o dano sumir, os opcodes mudaram: use `--op XXXX --hex N`
   para inspecionar e atualize `crates/nucleo/src/protocolo/opcodes.rs`.

## Pendências conhecidas

- **Level e "(você)"**: o seu level e o "(você)" só chegam no login (`0x3633`), então
  abra o overlay antes de entrar no mundo. Troca de servidor é resolvida
  sozinha (PROTOCOLO.md §8) e começa uma luta nova. O dos outros chega quando eles entram no seu
  campo de visão (`0x3645`). Depois de um level up o número fica velho até o próximo
  login (ou até o jogador sair e voltar à sua visão): o pacote de level up não é conhecido.
- **DoT** (`0x3805`): alvo e autor conferidos no world boss; o valor não foi conferido com a
  tela. A skill do DoT não identifica o autor (PROTOCOLO.md §4).
- **Healer não conferido**: nenhuma captura teve curandeiro, e a escala 1:1 da cura é
  suposição. Golpe de jogador em mob com skill que o questlog marca como cura (dreno, por
  exemplo) sai do DPS e entra no Healer. Só mexer nessa regra quando uma captura mostrar
  uma skill assim.
- **Tank**: escala 1:1 conferida num golpe só. "aggro" é inferido do último golpe de
  cada mob, porque o pacote de troca de alvo não foi achado.
- Invocação criada antes de o Axon abrir continua como linha própria "#id": o dono só vem no
  pacote de criação.
- **Firewall**: a regra não foi conferida num PC com o Firewall do Windows
  ativo (o PC de desenvolvimento usa o firewall do Kaspersky).
- **Party** (só quem está no seu grupo): o pacote do grupo (`0x9702` em outros medidores)
  não apareceu em nenhuma captura local; falta uma captura em grupo.
- **Energia Odyle**: conferida com a tela no login (`0x610B`) e no uso de essência OD
  (`0x610C`). O gasto em dungeon ainda não foi capturado; deve chegar no mesmo `0x610C`.
- **Ping**: conferido só contra as capturas (mínimo de 10 a 14 ms por janela de 10 s); falta
  comparar ao vivo com o número do jogo.
- **Chefes de campo**: só Altgard (região 1110) foi capturada, e a ordem dos nomes foi conferida
  em 3 dos 24 chefes. Outras regiões usam a mesma regra sem conferência.
- **Drops**: as % são as do questlog e não foram conferidas no jogo. Que cai uma peça do conjunto
  por morte é leitura nossa da soma de 100%; falta conferir matando o chefe algumas vezes.

## Conferido

- Crítico: dentro da mesma skill, `tipo_dano 3` tem média 1,6 vez maior que o tipo 2
  (Disparo Rápido: 384 contra 238), então a leitura de crítico está certa. O 0% do teste
  ao vivo (153 golpes) é dado real daquela luta.
- Port de C# para Rust (0.1.0 → 0.2.0): o replay em Rust deu saída idêntica à do CLI em
  C# nas 5 capturas locais, inclusive nas opções de diagnóstico.
