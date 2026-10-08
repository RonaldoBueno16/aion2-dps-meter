# F5. Alertas configuráveis (refinamento)

> Anexo do refinamento da 0.14.0 (2026-10-08), escrito por um agente e revisado na integração em
> `docs/0.14.0.md`. Os scripts e respostas citados em `<scratchpad>` ficaram na máquina do
> refinamento e não foram versionados; os pacotes e números que os testes usam estão neste texto.

Refinamento técnico e de negócio da frente F5 da 0.14.0, feito em 2026-10-08 no ramo `feat/0.14.0`
sem mudar o repositório. Afirmação marcada como **conferido** traz a evidência (arquivo:linha do
repositório, fonte de dependência no cache do cargo ou página da Microsoft com URL na seção 4);
**hipótese** é o que ninguém mediu ainda e vira item do teste manual.

Resumo da recomendação:

- 0.14.0: aviso de renascimento dos chefes de campo marcados (N min antes e na hora), aviso de
  início de cada evento da tabela (N min antes e na hora, resets incluídos), chave geral no menu da
  bandeja e botão "Testar alerta".
- Canal padrão: faixa dentro do próprio overlay mais um som curto. Canal alternativo: balão da
  bandeja, usado quando a faixa não pode ser vista (overlay escondido ou recolhido). Toast WinRT
  fica de fora.
- A checagem roda no fio da bandeja (temporizador de 1 s), lendo a `Sessao` direto, para continuar
  com o overlay escondido.
- Chefe de outra região: guardar na memória a última lista de cada região visitada. Nada vai para
  o disco na 0.14.0; o que precisaria ir está listado em 3.3 para decisão do dono.

## 1. Discovery

### 1.1 Fontes de dado que já existem

| Fonte | O que traz | Evidência |
|---|---|---|
| `0x9101` chefes de campo | Região, e por chefe: vivo/morto, `hora_ms` (morto: quando renasce; vivo: hora marcada em que nasceu, ou 0) | **conferido**: `crates/nucleo/src/protocolo/combate.rs:494-550`, PROTOCOLO.md:402-437 |
| Frequência do `0x9101` | De 1,0 a 6,3 s, só dentro da região, com o mapa aberto ou não | **conferido**: PROTOCOLO.md:404-405 |
| Guarda no medidor | Só a ÚLTIMA lista: `chefes_de_campo: Option<(ChefesDeCampo, Hora)>`, sobrescrita a cada pacote | **conferido**: `medidor.rs:271-273`, `sessao.rs:259-262` |
| Chefe nasce antes da hora | Um chefe passou a vivo 136 s antes da hora marcada | **conferido**: PROTOCOLO.md:428-430, `janela/chefes.rs:197-200` |
| Timers longos | Gartua Imortal visto às 17:22 com 4h58min para renascer | **conferido**: PROTOCOLO.md:426-427 |
| Eventos de horário fixo | 9 eventos (Fenda, Shugo, Invasão, Kairah, Cerco, Chefes do Cerco, Nahma, resets), horário de Brasília pelo relógio do PC | **conferido**: `crates/overlay/src/eventos.rs:57-133` |
| Kairah | Horário não confirmado | **conferido**: `eventos.rs:92-93` |
| Resets | `aberto: 0`: nunca passam pelo estado `Aberto`; no instante T o estado já é `Fechado(86400)` | **conferido**: `eventos.rs:119-132`, teste em `eventos.rs:298-306` |
| "Mate em" | Prazo do `0x8D21`, visto num chefe só (300 s) | **conferido**: `medidor.rs:698-712`, README.md:73-80 |
| Energia Odyle | Básica e carregada; o máximo não vem no pacote; gasto em dungeon não capturado | **conferido**: `medidor.rs:511-516`, PROTOCOLO.md:385-397 |
| O jogo mostra os chefes | O mapa mostra vivo/morto e o timer de todos os chefes da região; quem luta, posição e distância ficam escondidos | **conferido** (confirmação do dono em 2026-10-06, memória `axon-chefes-info-do-jogo`) |

### 1.2 Catálogo de alertas possíveis

"Jogo mostra" responde à regra do projeto: nunca avisar o que o jogo esconde.

| # | Alerta | Fonte | Gatilho exato | Jogo mostra? | Recomendação |
|---|---|---|---|---|---|
| A1 | Chefe marcado renasce em N min | `0x9101` `hora_ms` | Chefe morto com `hora_ms = R` e `R - N·60 s ≤ agora < R`. Dentro da região, se a lista em dia já diz vivo, o pré-aviso não sai (A2 cobre) | Sim: timer no mapa | **0.14.0** |
| A2 | Chefe marcado renasceu | `0x9101` `vivo` | Na região: a lista em dia passa de morto para vivo (pode vir antes de R). Fora dela (lista com mais de 30 s, `chefes.rs:17-19`): `agora ≥ R`, com o texto "deve ter renascido (lista das 17:22)" | Sim na região; fora dela é a conta sobre a hora que o jogo mostrou, a mesma regra do "provável" da 0.11.0 (README.md:166-169) | **0.14.0** (opção "na hora") |
| A3 | Chefe marcado morreu | `0x9101` `vivo` | Na região, lista em dia passa de vivo para morto. A primeira lista depois de abrir o Axon ou de entrar na região não tem estado anterior e não dispara | Sim: mapa (a memória do projeto diz que o aviso de morte cabe) | Depois (barato; pergunta 6) |
| A4 | Evento começa em N min (por evento) | `eventos.rs` | Próximo início `T` e `T - N·60 s ≤ agora < T`. `T` vem de uma função nova em `eventos.rs` que expõe o `desde`/`ate` que `estado()` já calcula antes do ramo `Aberto` (`eventos.rs:143-155`). Com o evento aberto, `agora + falta` é a hora de fechar e não serve | Horário público (tabela comunitária, `eventos.rs:1-5`); o jogo conta alguns (Shugo: timer ao lado do mapa, `eventos.rs:75`) | **0.14.0** |
| A5 | Evento começou | `eventos.rs` | Último início `T0` (a mesma função) e `T0 ≤ agora < T0 + 60 s`, nunca pela transição para `Aberto` (os resets nunca abrem) | Idem | **0.14.0** (opção "na hora") |
| A6 | Reset diário / semanal em N min | `eventos.rs:119-132` | O mesmo de A4/A5: os resets são eventos da tabela | Público | **0.14.0** (vem junto com A4) |
| A7 | Vigilante Kairah | `eventos.rs:87-94` | O mesmo de A4, com "horário não confirmado" no texto | Não confirmado | 0.14.0 com o rótulo, ou fora (pergunta 8) |
| A8 | Evento fecha em N min (Fenda 10 min, Shugo 3 min) | `eventos.rs` `aberto` | `Estado::Aberto(s)` com `s ≤ N·60` | Público | Depois |
| A9 | "Mate em" abaixo de N s / "não dá tempo" | `0x8D21` prazo + "derrota em" | `prazo - agora ≤ N` no chefe da luta | Sim: o prazo bateu com o relógio do jogo (README.md:73-80) | Depois: visto num chefe só; o card já fica vermelho |
| A10 | Odyle cheia / acima de N | `0x610B`/`0x610C` | Sem gatilho de "cheia": o máximo não vem (PROTOCOLO.md:387) e a recarga não foi capturada (PROTOCOLO.md:397) | Sim | Depois (F6 e captura nova) |
| A11 | Ticket recarregado, semanal pendente | F6 | Definido pela F6 | Sim | Depois (F6 alimenta o motor da F5) |
| A12 | Chefe marcado está vivo ao entrar na região | `0x9101` | Primeira lista da região com o marcado vivo | Sim | Depois |
| A13 | Item da lista de desejos: o chefe que o deixa cair renasce | F7 + drops do questlog | A1 aplicado aos chefes ligados aos itens da lista | Sim | Depois (F7 alimenta A1) |
| A14 | Axon parou de medir (captura caiu, servidor sumiu) | Rodapé (README.md:31-32) | Mesmo critério do aviso do rodapé | Estado do Axon | Depois (útil com o overlay escondido) |
| A15 | Ping alto | `Sessao::ping` (README.md:222-227) | Ping acima de N ms por X s | Sim | Depois, valor baixo |
| A16 | Groggy (F1), recorde batido (F3) | F1, F3 | Definido nas frentes | Depende da frente | Depois |

Recusados (o jogo esconde, ou fora do escopo):

- chefe em combate longe, quem luta com ele, posição, distância: o jogo esconde, e o `0x9101` nem
  traz combate (a posição é o ponto fixo de nascimento) (memória `axon-chefes-info-do-jogo`);
- jogador de outra facção entrou na visão: o medidor não mede PvP (README.md:298);
- qualquer alerta que dependa de ler memória do jogo, mandar pacote ou automatizar ação.

### 1.3 Recomendação

Entra na 0.14.0: A1, A2, A4, A5, A6 e A7 (este com rótulo, se o dono aceitar). Os seis saem de
dois motores: o de horário fixo (eventos) e o de renascimento (chefes marcados). A3 é barato (o
mesmo diff de listas do A2) e fica como pergunta ao dono. O resto depende de dado que não existe
(A9, A10) ou de outra frente (A11, A13, A16).

## 2. Negócio

### 2.1 Problema

O jogador quer estar no chefe de campo quando ele renasce e não perder Nahma, Cerco, Shugo ou a
Fenda. Hoje o Axon mostra as contagens (área de eventos e tela Bosses), mas só ajuda quem está
olhando para ela; o jogador em combate, em outra região, com o overlay escondido ou fora do jogo
perde o horário. O jogo mostra o timer do chefe só no mapa da região e não avisa.

### 2.2 Histórias

1. Como jogador, marco no ♛ os chefes que me interessam e recebo um aviso N minutos antes de cada
   um renascer, para dar tempo de chegar.
2. Como jogador, escolho para cada evento (Nahma, Cerco, Shugo, Fenda, resets...) se quero aviso e
   com quantos minutos de antecedência.
3. Como jogador em combate, ouço o aviso e vejo uma faixa no overlay sem o jogo perder o foco do
   teclado.
4. Como jogador com o overlay escondido pela bandeja, ainda recebo o aviso (som e balão do Windows).
5. Como jogador que saiu de Altgard, continuo recebendo o aviso do Gartua enquanto o Axon estiver
   aberto, sabendo que a hora é a última que o jogo mostrou.
6. Como jogador, desligo todos os alertas de uma vez pelo menu da bandeja, sem abrir o overlay.
7. Como jogador, aperto "Testar alerta" e vejo se a faixa, o som e o balão aparecem no meu PC com
   o jogo aberto.

### 2.3 Critérios de aceite

1. Evento com alerta de 10 min, Axon aberto antes: o alerta sai uma vez quando faltam 10:00 (no
   tick de 1 s seguinte) e mais uma no início, com "na hora" ligado. Nenhuma repetição no mesmo
   início.
2. Axon aberto quando faltam 4 min para um evento com alerta de 10 min: o alerta sai uma vez, com
   "em 4 min". Axon aberto (ou PC acordado da suspensão) depois do início: nada sai daquele início.
3. Reset diário com alerta: sai às 03:50 (10 min) e às 04:00 (na hora), apesar de o reset nunca
   entrar em `Aberto`.
4. Chefe marcado morto com R às 22:20:40 e antecedência de 5 min: o alerta sai às 22:15:40.
   Nascendo antes (servidor diz vivo às 22:18), sai "renasceu" e o pré-aviso que ainda não saiu
   fica cancelado.
5. Chefe marcado, jogador fora da região há mais de 30 s: o pré-aviso sai no horário; em R sai
   "deve ter renascido (lista das HH:MM)".
6. Overlay ligado e aberto: a faixa aparece em até 1 s depois do gatilho, sem o jogo perder o foco
   do teclado (o personagem continua andando com a tecla pressionada).
7. Overlay escondido (bandeja) ou recolhido na borda: o som toca e, com o balão em "quando a faixa
   não aparece", a chamada `Shell_NotifyIconW` é feita com título até 48 caracteres e texto até
   200. O balão aparece quando o estado do Windows deixa; com o jogo na frente e o "Não incomodar"
   automático ligado ele é descartado (`NIF_REALTIME`), e quem entrega é o som.
8. Dois alertas no mesmo tick saem juntos: um som, um balão ("2 alertas: ..."), duas faixas.
9. "Alertas" desmarcado no menu da bandeja: nada sai, nem som. Marcado de novo: alertas cujo
   instante já passou não saem atrasados.
10. Replay de debug (`--replay`): nenhum alerta sai, exceto o de `--testar-alerta`.
11. `config.json` antigo, sem `alertas`: abre com os padrões. Valor fora da faixa volta para dentro
    dela; valor desconhecido no campo do balão volta ao padrão sem derrubar o resto da config.
12. Nenhum texto de alerta leva nome de jogador; nada novo vai para a rede.

### 2.4 Escopo

Dentro (0.14.0):

- motor de alertas com A1, A2, A4, A5, A6 (A7 conforme a pergunta 8);
- memória por região dos chefes de campo (só RAM);
- faixa no overlay, som próprio, balão da bandeja quando a faixa não aparece;
- 🔔 por chefe na tela Bosses e por evento na lista de eventos expandida;
- seção "Alertas" nas configurações (com a F4);
- item "Alertas" com marca no menu da bandeja;
- botão "Testar alerta" (e `--testar-alerta` no debug);
- README (seção nova, Privacidade e "Dados em disco").

Fora (0.14.0): A3, A8 a A16, adiar (snooze), horário silencioso, toast WinRT, persistir horas de
chefe em disco, antecedência por chefe (se o dono escolher a global na pergunta 2), escolher o som.

### 2.5 Configurações

Todas em `config.json`, objeto `alertas` (padrões entre parênteses):

| Campo | Padrão | Faixa | Onde muda |
|---|---|---|---|
| `ligados` | `true` | bool | Menu da bandeja e seção Alertas |
| `eventos` | `{}` (nenhum) | id do evento → minutos antes, 0 a 60 | 🔔 na lista de eventos (liga com 5) e chips na seção Alertas (0, 1, 3, 5, 10, 15, 30, 60) |
| `chefes` | `[]` | ids do `0x9101` (ex.: 111021), até 100 | 🔔 na tela Bosses; ✕ na seção Alertas |
| `chefes_antes_min` | `5` | 0 a 60 | −/+ na seção Alertas |
| `na_hora` | `true` | bool | Seção Alertas (vale para "começou" e "renasceu"; com antecedência 0 o "na hora" sai sempre) |
| `som` | `true` | bool | Seção Alertas |
| `balao` | `"sem_banner"` | `"nunca"`, `"sem_banner"`, `"sempre"` | Seção Alertas |
| `banner_s` | `20` | 5 a 120 | −/+ na seção Alertas |
| `so_com_jogo_aberto` | `false` (pergunta 4) | bool | Seção Alertas |

O overlay não recebe teclado (README.md:50-51): todo ajuste é por clique (chips, −/+, 🔔). O glifo
🔔 (U+1F514) e o 🔕 (U+1F515) existem na `seguisym.ttf` que o overlay já carrega
(**conferido** neste PC pelo mapa de glifos da fonte; `janela.rs:2091-2092` carrega a fonte).

### 2.6 Comportamento (item 4 do pedido)

| Tema | Recomendação | Por quê |
|---|---|---|
| Antecedência | Por evento (0 a 60 min) e uma global para os chefes (0 a 60, padrão 5). 0 = só na hora: dispara no início (ou no renascer) mesmo com `na_hora` desligado | O pedido fala em "quanto antes de cada evento" e "quais chefes"; por chefe fica como pergunta 2 |
| Repetição | No máximo dois por ocorrência: "antes" e "na hora". Sem repetir em intervalo | A antecedência já cobre o "me lembra de novo" |
| Não repetir o mesmo | Chave `(origem, instante alvo, momento)`; chave usada não dispara de novo; chaves com mais de 1 dia saem da memória | Chefe morto de novo ganha outro R e outra chave |
| Adiar (snooze) | Fora | Exige faixa clicável, e com "clique atravessa" ligado o ✕ nem recebe clique; a antecedência resolve o caso |
| Horário silencioso | Fora | Sem teclado, a hora teria de ser montada por cliques; o "Não incomodar" do Windows já cala o balão, e a chave da bandeja cala som e faixa |
| Só com o jogo aberto | Opção, padrão a decidir (pergunta 4) | `jogo::aberto()` existe (`jogo.rs:32-35`); evento é lembrete útil com o jogo fechado, chefe só tem dado com ele aberto |
| Agrupar | Alertas do mesmo tick saem juntos | O Windows mostra um balão por vez e enfileira os outros (doc do NOTIFYICONDATAW) |
| Atraso | Gatilho por janela de tempo, nunca por igualdade | O `WM_TIMER` pode atrasar; PC suspenso não pode soltar alerta velho |
| Reinício do Axon | Dentro da janela "antes", o alerta sai de novo uma vez | A chave fica só na memória; aceitar a repetição evita outro arquivo |

### 2.7 Perguntas ao dono

1. Canal padrão: faixa no overlay + som, e o balão do Windows só quando a faixa não aparece. Ok?
   Consequência: com o som desligado e o overlay escondido durante o jogo, o alerta pode se perder
   (3.1).
2. Antecedência dos chefes: uma para todos (recomendado, padrão 5 min) ou uma por chefe?
3. "Na hora" (evento começou, chefe renasceu) ligado por padrão?
4. Alertas com o jogo fechado: avisar (recomendado para eventos: lembra de abrir o jogo) ou só
   com o jogo aberto?
5. Disco: guardar entre execuções a hora de renascer dos chefes marcados (campos em 3.3)?
   Recomendação: não na 0.14.0.
6. "Chefe marcado morreu" (A3) entra na 0.14.0?
7. Som: som próprio do Axon no volume do Axon no mixer (recomendado) ou no volume de "sons do
   sistema" (`SND_SYSTEM`), que muita gente deixa baixo ou zerado?
8. Kairah, com horário não confirmado: oferecer alerta com o rótulo "horário não confirmado" ou
   deixar sem alerta até confirmar?
9. Os ids dos chefes marcados (números como 111021, sem nome nem hora) vão para o `config.json`.
   Ok, dado que a 0.11.0 tirou do disco os dados de chefe (commit d4fe201)?

## 3. Técnico

### 3.1 Canal de entrega no Windows 11

#### Balão da bandeja (`Shell_NotifyIconW` com `NIF_INFO`)

- **Conferido** (doc Shell_NotifyIconW): no Windows 10 o balão vira banner e fica na Central de
  Notificações; no Windows 11 o banner é transitório e, depois de sumir, não aparece na central.
- **Conferido** (doc Notifications and the Notification Area): o sistema consulta
  `SHQueryUserNotificationState` e decide se mostra o balão; notificação com o usuário ausente
  entra na fila; em "quiet time" é descartada.
- **Conferido** (doc QUERY_USER_NOTIFICATION_STATE): `QUNS_BUSY` = "A full-screen application is
  running or Presentation Settings are applied"; `QUNS_RUNNING_D3D_FULL_SCREEN` = Direct3D em modo
  exclusivo.
- **Conferido** (página de suporte "Notifications and Do Not Disturb in Windows"): o "Não
  incomodar" pode ligar sozinho "When playing a game" e "When using an app in full-screen mode";
  ligado, só passam banners de alarmes, lembretes e apps escolhidos (prioridade); o resto vai para
  a central. A página não diz quais regras vêm ligadas por padrão.
- **Hipótese**: as regras "ao jogar" e "tela cheia" vêm ligadas por padrão (só guia de terceiro
  afirma). **Hipótese**: o AION 2 em tela cheia em janela (README.md:33) faz o Windows responder
  `QUNS_BUSY`; a sonda rodada neste PC deu `QUNS_ACCEPTS_NOTIFICATIONS`, mas com o jogo fechado.
  **Hipótese**: no Windows 11, balão segurado pelo "Não incomodar" se perde (o banner é transitório
  pela doc; o caminho dele com o "Não incomodar" ligado não está escrito).
- Foco: **hipótese** de que o balão não tira o foco do jogo (é UI do shell; a doc não fala de foco).
- Dependência: nenhuma. **Conferido** no fonte do `windows-sys 0.61.2`: `NIF_INFO`
  (`Win32/UI/Shell/mod.rs:3906`), `NIF_REALTIME` (:3908), `NIIF_RESPECT_QUIET_TIME` (:3918),
  `NIIF_USER` (:3919), `NIN_BALLOONUSERCLICK` (:3930), `SHQueryUserNotificationState` (:655), todos
  na feature `Win32_UI_Shell`, que o overlay já ativa (`crates/overlay/Cargo.toml:21-31`).
- Elevado: o ícone já funciona no processo elevado (release pede administrador, `build.rs:12-16`;
  filtro de mensagens em `bandeja.rs:118-122`).
- Uso: `NIM_MODIFY` com `NIF_INFO | NIF_REALTIME` (alerta de hora marcada mostrado depois engana:
  doc do `NIF_REALTIME`), `NIIF_USER | NIIF_LARGE_ICON | NIIF_RESPECT_QUIET_TIME` (a doc recomenda o
  último sempre) e `NIIF_NOSOUND` quando o som do Axon já tocou. Com `NIIF_LARGE_ICON`, o
  `hBalloonIcon` precisa vir no tamanho `SM_CXICON` (doc do NOTIFYICONDATAW); hoje o ícone da bandeja
  é carregado em `SM_CXSMICON` (`bandeja.rs:180-181`). Ficar na versão legada do ícone:
  `NIM_SETVERSION`/`NOTIFYICON_VERSION_4` muda o `lParam` do callback (doc), e o `bandeja.rs:140-144`
  depende da versão atual.

#### Toast WinRT

- **Conferido** (quickstart de app notifications do Windows App SDK, atualizado em 2026-09-10):
  "App notifications aren't supported for elevated (admin) apps" e `Show` falha em silêncio. O
  Axon de release roda elevado (`build.rs:12-16`).
- **Conferido** (doc arquivada "How to enable desktop toast notifications through an
  AppUserModelID", de 2018; a página atual equivalente deu 404 em 2026-10-08): sem atalho no Iniciar
  com AppUserModelID, um app desktop não levanta toast; a doc recomenda criar o atalho no instalador.
  O Axon é um exe único sem instalação (README.md:9-11).
- **Conferido**: o `windows-sys 0.61.2` só tem `Wdk` e `Win32` (sem WinRT): toast exige o crate
  `windows`, dependência nova.
- **Hipótese**: o `ToastNotificationManager` clássico também falha elevado.
- Sujeito ao mesmo "Não incomodar" do balão. Conclusão: fora.

#### Som (`PlaySoundW` / `MessageBeep`)

- **Conferido** (doc PlaySound): `SND_ASYNC` volta na hora; `SND_MEMORY` toca um WAV da memória, que
  precisa continuar válido até o fim do som; `SND_NODEFAULT` não troca pelo som padrão; sem
  `SND_NOSTOP`, o som novo para o anterior do mesmo processo; sem `SND_SYSTEM`, o som vai para a
  sessão de áudio do próprio processo; com ele, para o volume de "sons do sistema".
- **Conferido** (doc MessageBeep): toca o som que o usuário configurou no painel de Som e pode ser
  desligado lá; serve de reserva.
- Dependência: a feature `Win32_Media_Audio` do mesmo `windows-sys` (**conferido**: `PlaySoundW` em
  `Win32/Media/Audio/mod.rs:13`, feature em `Cargo.toml:99` do crate). Feature nova no
  `Cargo.toml` do overlay, sem crate novo.
- **Hipótese**: o "Não incomodar" não cala `PlaySound` (a página de suporte só fala de banners e
  da central). **Hipótese**: o som é audível por cima do áudio do jogo.
- Foco: nenhuma janela envolvida.
- Uso: WAV curto (duas notas de ~120 ms, PCM 16 bits mono) montado na memória na primeira vez, em
  `OnceLock<Vec<u8>>` estático, com `SND_MEMORY | SND_ASYNC | SND_NODEFAULT`. Sem arquivo novo no
  repositório e sem mexer no `build.rs`; o exe continua único.

#### Faixa dentro do overlay

- Aparece sobre o jogo: o overlay é sempre no topo (`main.rs:31`) e o jogo roda em tela cheia em
  janela (README.md:33). **Conferido**.
- Foco: **conferido**. `WS_EX_NOACTIVATE` é reaplicado a cada quadro (`janela.rs:2154-2169`,
  conferido no jogo em 2026-10-02, README.md:278-282); a doc de Extended Window Styles diz que a
  janela com esse estilo não vira a janela em primeiro plano ao ser clicada; a bandeja mostra o
  overlay com `SW_SHOWNOACTIVATE` (`bandeja.rs:292`), que a doc do ShowWindow descreve como mostrar
  sem ativar.
- Não passa pelo "Não incomodar" (é a janela do próprio Axon).
- Limite: some com o overlay escondido (`SW_HIDE`, e o egui para de desenhar, `bandeja.rs:5-7`) e
  com o overlay recolhido na borda. Com "clique atravessa" ligado (`bandeja.rs:41-42`) o ✕ não
  recebe clique: a faixa precisa expirar sozinha (`banner_s`).
- Dependência: nenhuma.

#### Decisão sobre `SHQueryUserNotificationState`

A doc da área de notificação pede que método próprio de notificação consulte o estado antes de
mostrar. Decisão proposta: faixa e som respeitam `QUNS_NOT_PRESENT` (tela bloqueada, protetor de
tela) e `QUNS_PRESENTATION_MODE` (o usuário pediu para bloquear pop-ups): nesses estados o som não
toca e o balão não sai. `QUNS_BUSY` e `QUNS_RUNNING_D3D_FULL_SCREEN` são ignorados, porque o caso de
uso é avisar com o jogo em tela cheia. "Quiet time" vale só para o balão (flag acima).

#### Comparação e recomendação

| Canal | Com o jogo em tela cheia em janela | "Não incomodar" esconde? | Tira o foco? | Dependência nova | Elevado |
|---|---|---|---|---|---|
| Faixa no overlay | Sim (conferido) | Não | Não (conferido) | Nenhuma | Funciona |
| Som | Sim, sem janela | Hipótese: não | Não | Feature `Win32_Media_Audio` | Funciona (hipótese) |
| Balão da bandeja | Só se o Windows não estiver em "Não incomodar" (hipótese: com o jogo, está) | Sim (conferido para as regras de jogo e tela cheia) | Hipótese: não | Nenhuma | Funciona (o ícone já roda elevado) |
| Toast WinRT | Idem balão | Sim | Hipótese: não | Crate `windows` + atalho com AUMID | Windows App SDK: não (conferido) |

- **Padrão**: faixa no overlay + som.
- **Alternativo**: balão da bandeja, automático quando a faixa não pode ser vista (overlay escondido
  pela bandeja ou recolhido na borda), e opcional "sempre".
- Quem quiser o balão com o jogo aberto pode pôr o Axon em "Notificações prioritárias" do Windows
  (doc de suporte); se um exe sem pacote aparece nessa lista é **hipótese** para o teste manual.
- Caso em que o alerta se perde: som desligado e overlay escondido (ou recolhido) com o jogo na
  frente. A faixa não aparece e o balão é descartado pelo `NIF_REALTIME` se o Windows estiver em
  "Não incomodar". A seção Alertas avisa isso ao desligar o som (pergunta 1).

### 3.2 Onde a checagem roda

Fio da bandeja, segundo temporizador (id 2, 1000 ms) na mesma janela oculta (`bandeja.rs:125` já
cria o de 200 ms). É o único fio do Axon que continua vivo com o overlay escondido
(`bandeja.rs:1-7`); a cópia `self.chefes` do overlay só anda dentro do `update()`
(`janela.rs:326`), então a checagem lê a `Sessao` direto.

```
fio da bandeja (bandeja.rs)                          fio do egui (janela.rs)
WM_TIMER id 2, a cada 1 s:                           Overlay::novo: alertas::definir_regras(config)
  1. trava a Sessao, clona chefes_por_regiao, solta  config mudou: alertas::definir_regras(...)
  2. lê REGRAS (Mutex), LIGADO, RECOLHIDO,           a cada quadro: RECOLHIDO = !dobra.aberto()
     jogo::aberto(), jogo::seguindo()                update(): drena a FILA de faixas e desenha
  3. alertas::vencidos(...)   [lógica pura]
  4. SHQueryUserNotificationState
  5. som (PlaySoundW assíncrono)
  6. balão (Shell_NotifyIconW NIM_MODIFY) se a faixa não aparece ou balao = "sempre"
  7. FILA.push(alertas) + ctx.request_repaint()
```

- `Bandeja::iniciar` (`bandeja.rs:68`, chamado em `janela.rs:297`) passa a receber
  `Arc<Mutex<Sessao>>` e um clone do `egui::Context`. **Conferido**: o `Context` do egui 0.33.3 é
  `Send + Sync` (`egui-0.33.3/src/context.rs:4078-4080`).
- O lock da `Sessao` é o mesmo da captura: clonar o mapa (dezenas de regiões × 24 chefes) e soltar.
- `REGRAS`: `static Mutex<Alertas>` no módulo `alertas`, gravado pelo fio do egui ao carregar e a
  cada mudança de config; o tick só lê.
- Nomes dos chefes: `dados_jogo::regiao(codigo)` funciona de qualquer fio (catálogo global; pede ao
  questlog uma vez). Reusar `chefes::chefes_vistos` (`janela/chefes.rs:51-80`, função pura que
  já faz nome, "provável" e lista antiga) tornando-a `pub(crate)`, para as duas telas e o alerta
  usarem a mesma regra.
- Gate do replay: sem alertas quando `!jogo::seguindo()` (`jogo.rs:23-25`; o replay desliga o
  seguir em `janela.rs:213`), exceto `--testar-alerta`.
- Faixa visível: `LIGADO` (`bandeja.rs:40`) e um `AtomicBool RECOLHIDO` que o overlay grava quando a
  dobra muda. Com o overlay visível, o `update()` roda pelo menos a cada `atualizacao_ms`
  (`janela.rs:1655-1656`), e o `request_repaint` traz a faixa na hora.

### 3.3 Chefe em outra região e disco

Hoje a lista de uma região some quando chega a de outra (`sessao.rs:261` sobrescreve
`medidor.chefes_de_campo`). Por isso o alerta do Gartua já falharia dentro da mesma execução, antes
de qualquer questão de disco.

Proposta 0.14.0, só memória: `Medidor` ganha `chefes_por_regiao: HashMap<u32, (ChefesDeCampo,
Hora)>`, preenchido no mesmo ponto do `sessao.rs:259-262`, sem zerar na troca de conexão nem no
Zerar (mesma regra do campo atual, `medidor.rs:271-272`; `nova_conexao` em `medidor.rs:924-941` não
toca nele). O `chefes_de_campo` atual continua como "região em que você está". Some ao fechar o
Axon, como o README promete (README.md:330-331).

O que o alerta faz fora da região, com essa memória:

- morto com R no futuro: o pré-aviso sai no horário; a hora é a que o jogo mostrou quando você
  estava lá;
- em R: "deve ter renascido (lista das 17:22)", a mesma regra do "provável" da tela Bosses;
- depois de R o Axon não sabe se alguém matou de novo; o texto diz de quando é a lista.

Para funcionar entre execuções, iria para o disco (`%LOCALAPPDATA%\Aion2Meter\renascimentos.json`),
só para os chefes marcados:

| Campo | Exemplo | Para quê |
|---|---|---|
| id do chefe (`0x9101`) | `111021` | Qual chefe |
| `renasce_ms` | `1791336040627` | A hora de renascer que o jogo mostrou |
| `visto_ms` | `1791322956000` | Quando a lista foi vista ("lista das 17:22") |

Descarte: entrada com `renasce_ms` mais de 1 h no passado, ou `visto_ms` com mais de 48 h; lista
nova da região sobrescreve. Nada de vivo/morto dos não marcados, posição ou nome. Risco extra:
manutenção do servidor pode zerar os timers (hora 0 dos vivos é hipótese de reinício,
PROTOCOLO.md:419-421), e a entrada guardada sairia errada.

Recomendação: só memória na 0.14.0, como o dono pediu para a lista de chefes (commit d4fe201,
README.md:195-196); o arquivo acima fica para a pergunta 5.

### 3.4 Modelo de dados e config

```rust
// config.rs: Config ganha `pub alertas: Alertas` (#[serde(default)] já cobre config antigo).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Alertas {
    pub ligados: bool,                  // true
    pub eventos: BTreeMap<String, u32>, // {} ; id do evento -> minutos antes (0..=60)
    pub chefes: BTreeSet<u32>,          // {} ; id do 0x9101 (regiao*100 + n), até 100
    pub chefes_antes_min: u32,          // 5 (0..=60)
    pub na_hora: bool,                  // true
    pub som: bool,                      // true
    pub balao: String,                  // "sem_banner" | "nunca" | "sempre"; outro valor volta ao padrão
    pub banner_s: u32,                  // 20 (5..=120)
    pub so_com_jogo_aberto: bool,       // pergunta 4
}

// eventos.rs: Evento ganha um id estável (o nome é texto de tela e pode mudar).
pub id: &'static str, // "fenda", "shugo", "invasao", "kairah", "cerco", "chefes_cerco",
                      // "nahma", "reset_diario", "reset_semanal"
// e a função que expõe o `desde`/`ate` que estado() já calcula (eventos.rs:143-155):
/// Último início e próximo início, em segundos unix. Vale com o evento aberto ou fechado.
pub fn inicios(evento: &Evento, hora: Hora) -> (i64, i64)

// alertas.rs (novo): lógica pura, sem Windows, testável com hora simulada.
pub enum Origem { Evento(&'static str), Chefe(u32) }
pub enum Momento { Antes, NaHora }
pub struct Alerta { pub origem: Origem, pub momento: Momento, pub alvo_unix: i64,
                    pub titulo: String, pub texto: String, pub icone: Option<String> }
pub struct Memoria { disparados: HashMap<(Origem, i64, Momento), i64>,
                     ultimo_estado: HashMap<u32, (bool, i64)> } // chefe -> (vivo, hora_ms)
pub fn vencidos(regras: &Alertas, chefes: &[ChefeVisto], agora: Hora, jogo_aberto: bool,
                memoria: &mut Memoria) -> Vec<Alerta>
```

Exemplo de `config.json`:

```json
"alertas": {
  "ligados": true,
  "eventos": { "nahma": 10, "shugo": 2, "reset_semanal": 30 },
  "chefes": [111013, 111021],
  "chefes_antes_min": 5,
  "na_hora": true,
  "som": true,
  "balao": "sem_banner",
  "banner_s": 20,
  "so_com_jogo_aberto": false
}
```

O `balao` fica como texto com conversão própria porque enum do serde com valor desconhecido faria o
`from_str` falhar e a config inteira voltaria ao padrão (`config.rs:79`). Ids de evento que não
existem mais na tabela são ignorados.

### 3.5 Encaixe no código

| Arquivo | Mudança |
|---|---|
| `crates/nucleo/src/medicao/medidor.rs`, `sessao.rs` | `chefes_por_regiao` (RAM) |
| `crates/overlay/src/alertas.rs` (novo) | Regras, `vencidos`, memória de disparados, WAV na memória, `tocar`, texto do balão |
| `crates/overlay/src/bandeja.rs` | Timer 2, chamada do tick, balão (`NIM_MODIFY` + `NIF_INFO`), item "Alertas" no menu, `RECOLHIDO` |
| `crates/overlay/src/eventos.rs` | Campo `id` |
| `crates/overlay/src/config.rs` | `Alertas` + faixas + teste |
| `crates/overlay/src/janela.rs` | Passar `Sessao` e `Context` à bandeja; faixa(s) no topo do conteúdo; 🔔 nas linhas da lista de eventos; `--testar-alerta` |
| `crates/overlay/src/janela/chefes.rs` | `chefes_vistos` `pub(crate)`; 🔔 por linha |
| Seção "Alertas" | Na tela de configurações da F4 |
| `crates/overlay/Cargo.toml` | Feature `Win32_Media_Audio` |
| `README.md` | Seção Alertas; "Dados em disco" (config) |

Faixa: logo abaixo do cabeçalho, até 3 empilhadas (a 4ª vira "+1"), com o ícone da área de eventos
(retrato do chefe ou ícone do evento), o texto, a contagem viva até o alvo e ✕; borda dourada;
some em `banner_s` ou no ✕. Na barra compacta aparece acima da linha. Recolhido conta como "faixa
não aparece".

### 3.6 Riscos

| Risco | Efeito | Mitigação |
|---|---|---|
| "Não incomodar" ligado pelo jogo | Balão não aparece ou se perde | Padrão é faixa + som; balão só como reserva |
| Relógio do PC errado | Alerta fora de hora (eventos e chefes contam pelo relógio do Windows) | Mesmo caso já documentado (README.md:77-80); nada novo |
| Tabela de eventos muda ou está errada (Kairah) | Alerta errado | Rótulo "não confirmado"; tabela já citada na dica |
| Chefe nasce antes da hora | Pré-aviso sai com ele vivo | Na região, vivo cancela o pré-aviso e solta "renasceu" |
| Fora da região, chefe morto de novo | "Deve ter renascido" errado | Texto diz a hora da lista |
| Manutenção zera timers | Hora velha na memória | Lista nova sobrescreve; sem disco na 0.14.0 |
| Lock da `Sessao` no tick | Captura espera | Clonar e soltar; 1 vez por segundo |
| `WM_TIMER` atrasado ou PC suspenso | Alerta perdido ou atrasado | Gatilho por janela; teste de pulo de relógio |
| Som desligado e overlay escondido no jogo | Alerta se perde (balão descartado) | Aviso na seção Alertas ao desligar o som |
| Barulho (Shugo toda hora) | Jogador desliga tudo | Padrão sem nenhum evento marcado; chave na bandeja |
| Config editada à mão com valor ruim | Config inteira volta ao padrão | `balao` como texto; faixas em `dentro_das_faixas`; teste |
| Teste de debug não representa a release | Debug roda sem elevação (`build.rs:12-16`) | Teste manual com o exe de release |

### 3.7 Plano de verificação

Testes de lógica pura em `alertas.rs` (hora simulada em ticks, mesmas constantes de
`eventos.rs:220-232`), cada um com red check (quebrar a condição de propósito, ver o teste falhar,
desfazer):

1. Evento com 10 min: dispara em `T-600 s`, não em `T-601 s`; ticks seguintes não repetem; a
   próxima ocorrência dispara de novo. Red check: trocar `≤` por `<` na janela, tirar o registro da
   chave.
2. Axon aberto no meio da janela: dispara uma vez com o tempo real; depois de T não dispara.
   Nahma aberto às 21:25 com 10 min de antecedência: não dispara "começa em" (red check: usar
   `agora + falta` de `Estado::Aberto` como próximo início faz o teste falhar).
   Antecedência 0 com `na_hora = false`: dispara no início.
3. Pulo de relógio de 2 h (suspensão): nada de alerta vencido.
4. Tick atrasado 5 s: o alerta sai uma vez.
5. Reset diário com "na hora": dispara às 04:00:00 a 04:00:59 (red check: gatilho por
   `Estado::Aberto` não dispara).
6. Chefe: pré-aviso em `R - 300 s`; R novo gera chave nova; vivo antes de R na lista em dia
   cancela o pré-aviso e solta "renasceu"; lista antiga em R solta "deve ter renascido"; primeira
   lista sem estado anterior não solta transição; `hora_ms = i64::MAX` não estoura (como em
   `chefes.rs:295-301`).
7. `ligados = false`, `so_com_jogo_aberto` com jogo fechado, `seguindo = false`: nada sai.
8. Dois alertas no mesmo tick viram um texto de balão; título cortado em 48 e texto em 200
   caracteres sem quebrar caractere.
9. Config: arquivo sem `alertas`, faixas, `balao` desconhecido, id de evento inexistente (no teste
   que já existe em `config.rs:122-148`).
10. Nucleo: lista de Altgard e depois de outra região ficam as duas; a mesma região substitui.

Comando: `CARGO_TARGET_DIR=target/agente-F5 cargo test --workspace` (pela ferramenta Bash).

Teste manual do dono (com o exe de release, elevado, e o jogo em tela cheia em janela):

1. Rodar a sonda só leitura do Apêndice A, trocar para o jogo em até 5 s e anotar as linhas com
   `na_frente=True` (`QUNS_BUSY` com o jogo na frente confirma a hipótese).
2. Conferir em Configurações > Sistema > Notificações quais regras de "Ativar não incomodar
   automaticamente" estão marcadas.
3. "Testar alerta" com o jogo na frente: a faixa aparece? O som toca e dá para ouvir por cima do
   jogo? O balão aparece? O personagem continua andando com a tecla pressionada durante o alerta?
4. O mesmo com o overlay escondido pela bandeja, com o overlay recolhido e com o jogo minimizado.
5. O Axon aparece em "Notificações prioritárias > Adicionar apps"?
6. Real: Shugo Festa com 1 min, esperar a hora cheia.
7. Real: marcar um chefe morto de Altgard com timer curto, sair da região e esperar.

### 3.8 Tamanho

**M**: memória por região (P), `alertas.rs` com testes (M), som + balão + faixa (M), 🔔 nas duas
telas, menu da bandeja e "Testar" (P). Vira **G** se a F4 atrasar e a F5 tiver de montar a própria
tela de configurações.

### 3.9 Dependências

- **F4 (configurações com seções)**: a seção "Alertas" mora lá. Se a F4 atrasar, a F5 sai assim
  mesmo: 🔔 na tela Bosses e na lista de eventos (liga com o padrão de 5 min), chave na bandeja e o
  resto pelo `config.json`.
- **F6 (tickets e semanais)**: fonte futura (A10, A11) no mesmo motor; a F5 não espera a F6.
- **F7 (lista de desejos)**: alimenta o A1 (marcar os chefes que deixam cair itens da lista). Precisa
  do mapa item → chefe da F7 (drops do questlog, em memória); entra depois.
- **F1 e F3**: fontes futuras (groggy, recorde) no mesmo motor.

### 3.10 Dados em disco (para o OK do dono)

| Arquivo | Campo novo | Conteúdo |
|---|---|---|
| `config.json` | `alertas` | Preferências da seção 2.5, incluindo os ids dos chefes marcados (só números como 111021) |
| `renascimentos.json` | (só com a pergunta 5 aprovada) | id, `renasce_ms`, `visto_ms` dos chefes marcados |

Nenhuma requisição de rede nova.

## 4. URLs consultadas

Microsoft (todas lidas em 2026-10-08):

- https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ns-shellapi-notifyicondataw
- https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw
- https://learn.microsoft.com/en-us/windows/win32/shell/notification-area
- https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ne-shellapi-query_user_notification_state
- https://support.microsoft.com/en-us/windows/notifications-and-do-not-disturb-in-windows-feeca47f-0baf-5680-16f0-8801db1a8466
- https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-quickstart
- https://learn.microsoft.com/en-us/previous-versions/windows/desktop/legacy/hh802762(v=vs.85)
- https://learn.microsoft.com/en-us/previous-versions/dd743680(v=vs.85) (PlaySound)
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messagebeep
- https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow
- 404 em 2026-10-08: https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-other-apps
  e .../send-local-toast-desktop-cpp-wrl

Busca (só para achar as páginas acima; o conteúdo de terceiros não sustenta nenhuma afirmação
marcada como conferida): uma busca sobre as regras automáticas do "Não incomodar" (guia de
terceiro, geekrewind.com, origem da hipótese de padrão ligado) e uma sobre timers de chefe do AION 2
(aion2hub.com/tools/event-timer, sem informação sobre alertas).

Fontes locais: `windows-sys-0.61.2` e `egui-0.33.3` no cache do cargo; mapa de glifos de
`C:\Windows\Fonts\seguisym.ttf`; sonda só leitura do Apêndice A (resultado com o jogo fechado:
`QUNS_ACCEPTS_NOTIFICATIONS`).

## Apêndice A. Sonda só leitura do estado de notificação

Não abre janela nem dispara notificação: lê `SHQueryUserNotificationState` e se a janela do
AION 2 está na frente, a cada 2 s durante 20 s (dá tempo de trocar do terminal para o jogo).
Salvar como `estado_notificacao.py` e rodar com `python -I estado_notificacao.py`.

```python
"""Só leitura: estado de notificação do Windows (SHQueryUserNotificationState) e se a janela do
AION 2 existe e está na frente, a cada 2 s durante 20 s. Não abre janela, não dispara notificação."""
import ctypes
import time
from ctypes import wintypes

user32 = ctypes.windll.user32
shell32 = ctypes.windll.shell32
NOMES = {1: "QUNS_NOT_PRESENT", 2: "QUNS_BUSY", 3: "QUNS_RUNNING_D3D_FULL_SCREEN",
         4: "QUNS_PRESENTATION_MODE", 5: "QUNS_ACCEPTS_NOTIFICATIONS", 6: "QUNS_QUIET_TIME", 7: "QUNS_APP"}


def janela_do_jogo():
    achada = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def conferir(h, _):
        if not user32.IsWindowVisible(h):
            return True
        classe = ctypes.create_unicode_buffer(64)
        titulo = ctypes.create_unicode_buffer(64)
        user32.GetClassNameW(h, classe, 64)
        user32.GetWindowTextW(h, titulo, 64)
        if classe.value == "UnrealWindow" and titulo.value.startswith("AION2"):
            achada.append(h)
            return False
        return True

    user32.EnumWindows(conferir, 0)
    return achada[0] if achada else None


print("Troque para o jogo agora; 10 leituras, uma a cada 2 s.")
for _ in range(10):
    time.sleep(2)
    estado = ctypes.c_int(0)
    hr = shell32.SHQueryUserNotificationState(ctypes.byref(estado))
    jogo = janela_do_jogo()
    frente = jogo is not None and jogo == user32.GetForegroundWindow()
    print(f"{time.strftime('%H:%M:%S')} hr=0x{hr & 0xFFFFFFFF:08X} estado={estado.value} "
          f"{NOMES.get(estado.value)} jogo_aberto={jogo is not None} na_frente={frente}")
```
