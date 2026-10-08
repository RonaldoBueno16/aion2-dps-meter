# F6: tickets e semanais (Axon 0.14.0), refinamento

> Anexo do refinamento da 0.14.0 (2026-10-08), escrito por um agente e revisado na integração em
> `docs/0.14.0.md`. Os scripts e respostas citados em `<scratchpad>` ficaram na máquina do
> refinamento e não foram versionados; os pacotes e números que os testes usam estão neste texto.

Legenda: **conferido** = evidência citada (dump, contagem, arquivo:linha, URL); **hipótese** = dedução
ainda sem confirmação na tela do jogo. Horários em Brasília (UTC-3), como em `eventos.rs:9-10`.
Levantamento feito em 2026-10-08, só com os 11 dumps e o questlog; repositório não foi alterado.

## Respostas diretas às cinco perguntas

1. **Ids e valores.** Os dumps têm **três** `0x610B` (o PROTOCOLO.md:378 fala em duas), todas com 73
   entradas e leitura fechando no último byte (**conferido**): `captura-2026-10-02-login` (10-02
   00:57:22), `captura-2026-10-03-b` (o arquivo tem nome de 10-03, mas os ticks dão 10-02 01:25:02) e
   `captura-2026-10-05-login-odyle` (10-05 14:27:29). Tipos: `00` ×13, `01` ×9, `04` ×51 nas de 10-02;
   `00` ×13, `01` ×9, `04` ×50, `0C` ×1 na de 10-05. Entre as duas de 10-02 só a Odyle mudou (155 → 130);
   entre 10-02 01:25 e 10-05 mudaram 21 ids. `0x610C`: dois no total, ticket 10 (valor 14, igual ao do
   login) e a Odyle (550, +310). Tabela completa na seção 2.1.
2. **Nomes.** Para 21 dos 73 ids há fonte: a regra `id ÷ 100` leva cada ticket `600XXY01` a uma
   dungeon de grupo do questlog (`getDungeons`/`getDungeon`), 21 de 21, e a dificuldade do questlog
   separa o comportamento dos valores sem exceção (seção 2.1.3). O nome é **conferido** como "o nome que
   o questlog dá ao id da dungeon"; a ligação ticket → dungeon é **hipótese** até o print. Tirando
   esses 21 e a Odyle (60.000.001, já conferida), para os outros 51 ids não há fonte (questlog `getQuest`, `getAchievement`, `getItem` e `searchEntities`
   não acharam nada): ficam sem nome. O pedido de print e captura está na seção 1.6.
3. **Reset.** O servidor manda a hora do próximo reset diário (04:00) no `0xE223` e no `0x8D1C`, e a
   janela semanal (quarta 04:00:01 até quarta 04:00:00,999) no `0xE256` (**conferido**, seção 2.1.5).
   Isso confirma com dado do servidor a tabela de `eventos.rs:119-132`. O `0x610B` não traz reset por
   ticket. Entre 10-02 01:25 e 10-05 14:27 houve 4 resets diários e nenhum semanal; nesse intervalo os
   tickets de dificuldade normal (tier 1 a 3) foram de 2 para 10 e os de difícil de 2 para 4 (**conferido**).
   Que cada normal ganha 2 por reset diário é **hipótese** (8 = 4 × 2); o reset de cada ticket fica
   para o experimento (c) da seção 1.6.
4. **Semanais.** Nos pacotes já decodificados (`0x610B`, `0x610C`) não há contador semanal. Em pacotes
   ainda não usados há dois candidatos: o grupo tipo 2 do `0xE256` (10 ids com janela semanal,
   contadores zerados nos 3 logins) e o `0x568E` (5 ids com prazo na quarta 04:00). Sem nome e sem
   saber o que contam, nenhum dos dois entra na tela. Proposta: checklist manual que zera no reset
   pela hora gravada da marcação; campos em disco listados na seção 1.5.
5. **Onde mostrar.** Tela própria "Entradas" (ícone no topo, como ♛ Chefes em `janela.rs:530`) e uma
   linha-resumo nos eventos expandidos que abre a tela, no mesmo padrão dos chefes de campo
   (`janela.rs:1196-1198`). O rodapé não tem espaço (a Odyle já só aparece se couber, README.md:231).
   Configuração: quais tickets esconder, checklist; ordem fixa na primeira versão; alerta de reset fica
   com a F5.

## 1. Negócio

### 1.1 Problema

O jogador quer saber, sem abrir a tela de dungeons do jogo, quantas entradas ainda tem em cada dungeon
e quando elas voltam. O Axon já recebe esses números no login (`0x610B`) e a cada mudança (`0x610C`),
mas hoje só usa a Energia Odyle (`medidor.rs:511-516`). Os nomes não vêm no pacote, e a regra do
projeto proíbe nome inventado.

### 1.2 Histórias

- **H1.** Como jogador, quero ver numa tela do Axon as entradas que tenho em cada dungeon de grupo, por
  dificuldade, com o mesmo número que o jogo mostra, para escolher onde ir sem abrir a tela do jogo.
- **H2.** Como jogador, quero saber quanto falta para o próximo reset diário e semanal ao lado das
  entradas, para não perder entrada acumulada.
- **H3.** Como jogador, quero perceber quando o número pode estar desatualizado (chegou antes do
  último reset), para não confiar num valor velho.
- **H4.** Como jogador, quero marcar à mão conteúdos semanais e diários que o Axon não lê do pacote
  e vê-los desmarcados sozinhos no reset.
- **H5.** Como dono, quero que nada do jogo fique gravado no PC por causa dessa tela.

### 1.3 Critérios de aceite

1. Com o Axon aberto antes de entrar no personagem, ao chegar o `0x610B` a tela "Entradas" lista cada
   ticket da tabela confirmada pelo print, com o valor igual ao da tela do jogo no mesmo instante.
2. Ao chegar um `0x610C` de ticket da tabela, o valor muda na tela na releitura seguinte
   (`atualizacao_ms`, padrão 500 ms).
3. Ticket sem nome confirmado não aparece, nem como id cru.
4. Valor recebido antes do último reset diário aparece apagado e com dica: "Chegou antes do reset das
   04h; atualiza no próximo login ou ao entrar na dungeon".
5. Com o Axon aberto depois do login, a tela diz "Os valores chegam no login: abra o Axon antes de
   entrar no personagem", como a Odyle hoje (README.md:230-231).
6. A tela mostra a contagem até o reset diário e o semanal usando `eventos.rs` (os mesmos números da
   linha de eventos).
7. Os tickets não são gravados em disco. O `config.json` só ganha campo se o jogador esconder um
   ticket ou usar o checklist, e só com os campos da seção 1.5.
8. Checklist: item semanal marcado na terça aparece desmarcado a partir de quarta 04:00; item diário
   marcado às 03:59 aparece desmarcado às 04:00.
9. Nenhuma requisição nova à internet (nomes em tabela fixa no código, com fonte e data).

### 1.4 Escopo

**Dentro**
- Tickets `600XXY01` confirmados pelo print (até 21), com nome e dificuldade do questlog.
- Marca de valor velho pela hora de chegada contra o último reset diário.
- Tela "Entradas" e linha-resumo nos eventos expandidos.
- Checklist manual diário/semanal, se o dono aprovar a gravação (seção 1.5).
- PROTOCOLO.md §7d corrigido (três listas, regra `÷ 100` como hipótese) e §7f nova (pacotes de reset);
  README com a tela nova.

**Fora**
- Os 51 ids sem fonte de nome (1 a 15, 101 a 103, 201 a 206, 10.000.001 a 10.000.012, 60.000.002 a
  60.000.010, 70.000.001, 80.000.001, 90.000.001 a 90.000.006), até haver print que os identifique.
- O máximo de entradas ("Y" de um "X/Y"), a menos que o jogo mostre e o print traga. O campo
  `ceilingCount` do questlog não serve como máximo: nos fáceis ele vale 1 ou 3 e o ticket vale 7.
- Mostrar `0xE256` e `0x568E` (sem nome, sem saber o que contam).
- Alertas de reset (F5).
- Gravar tickets em disco, consultar o questlog em tempo de execução, ler memória do jogo.
- Mudança na Odyle do rodapé.

### 1.5 Configurações (e o que vai para o disco)

Os tickets ficam só na memória: o login manda a lista inteira de novo. Dois campos novos no
`config.json` (`%LOCALAPPDATA%\Aion2Meter\config.json`, `config.rs:1`), ambos com
`skip_serializing_if` vazio, para o arquivo não mudar enquanto o jogador não usar:

| Campo | Tipo | Conteúdo | Quando é gravado |
|---|---|---|---|
| `entradas_ocultas` | lista de `u32` | ids de ticket que o jogador escondeu (ex.: `[60001101]`) | Ao esconder um ticket |
| `checklist` | lista de itens | um item por linha do checklist | Ao criar, marcar ou apagar item |
| `checklist[].nome` | texto | o que o jogador digitou (ex.: "Nahma") | idem |
| `checklist[].periodo` | `"diario"` ou `"semanal"` | quando zera (04:00 todo dia ou quarta 04:00) | idem |
| `checklist[].feito_em` | inteiro ou `null` | hora Unix em segundos da última marcação | Ao marcar ou desmarcar |

"Feito" = `feito_em` depois do início do período atual (último reset pelo `eventos.rs`). Zera sem tarefa
em segundo plano e sem apagar nada: a hora antiga continua no arquivo até a próxima marcação. Nenhum
dado do jogo vai para esses campos (nada de personagem, servidor, valor de ticket). Alternativa para o
dono: checklist só na memória, que some ao fechar o Axon (aí o semanal perde o sentido).

Ordem: fixa na primeira versão, a da tela do jogo no print. Ordem configurável só se o dono pedir.
Alerta de reset: configurado na F5.

### 1.6 Perguntas ao dono

1. **Print + captura de login (experimento a).** Pedido exato:
   - Ligue a captura do jeito que gerou `captura-2026-10-05-login-odyle` **antes** de escolher o
     personagem; entre no jogo.
   - Sem entrar em dungeon e sem usar item, abra a tela do jogo que lista as dungeons com as entradas.
     Tire print de **todas** as abas e páginas, com todas as dificuldades das dungeons de grupo
     (Caverna de Krao, Desfiladeiro de Urugugu, Templo Divino do Fogo, Draupnir, Ninho de Dramata
     Morto, Base de Pesquisa Deus, Arcanis Despedaçada, Ilha Aérea de Vakron, Gruta do Chifre Feroz,
     Berço do Vazio, Corredor da Ilusão, Ilha do Sopro Azul, Cidadela do Daeva Caído, nomes do
     questlog) e o rótulo de dificuldade que o jogo usa. Se aparecer "X/Y" ou um relógio de recarga,
     que fique visível no print.
   - Faça o mesmo em qualquer outra tela de conteúdo com contador (arena, abismo, missões
     semanais), para tentar identificar os ids de 1 a 206 e de 70.000.001 a 90.000.006.
   - Pare a captura depois dos prints. Mande os prints e a captura juntos.
2. **Entrada numa dungeon (experimento b).** Com a captura ligada, print da tela de entradas, entrar
   uma vez numa dungeon de grupo de dificuldade conhecida (ex.: Desfiladeiro de Urugugu normal), sair,
   print de novo. O `0x610C` mostra qual id mudou, e a queda da Odyle comparada ao `odyleEnergyCost`
   do questlog (20, 30 ou 40) confere a ligação daquele ticket.
3. **Reset com o jogo aberto (experimento c).** Captura ligada de 03:55 a 04:05, print da tela de
   entradas antes e depois das 04:00. Mostra se o servidor manda `0x610C` no reset para quem está
   online e quanto cada ticket ganha. Se der, repetir numa quarta (reset semanal).
4. "Semanais": você quer acompanhar as missões semanais do jogo ou uma lista sua (ex.: "fiz o Nahma",
   "fiz o cerco")? A primeira depende de nome para o `0xE256`, que não existe; a segunda é o checklist.
5. Aceita os campos da seção 1.5 no `config.json`, ou prefere o checklist só na memória?
6. Tickets parados em 7 (todos os fáceis e os normais de tier 4): mostrar ou esconder por padrão?
   O print diz se o 7 é "entradas" ou outra coisa.
7. Nomes em tabela fixa no código (zero requisição nova, revisada a cada patch) ou `getDungeon` em
   tempo de execução (21 requisições com cache vazio, nova linha na tabela de privacidade do README)?
   Recomendação: tabela fixa.

## 2. Técnico

### 2.1 Dados

#### 2.1.1 Onde os pacotes aparecem (conferido)

Script `refino/trabalho-F6/tickets.py`, leitura com `base_tempo.carregar`. Nenhum `0xFFFF` nos dumps.

| Dump | Início (Brasília) | `0x610B` | `0x610C` | `0xE223` | `0x8D1C` | `0xE256` | `0x568E` |
|---|---|---|---|---|---|---|---|
| captura-2026-10-01 | 10-01 22:32 qui | 0 | 0 | 0 | 0 | 0 | 0 |
| captura-2026-10-02-combate | 10-02 00:51 sex | 0 | 0 | 0 | 0 | 0 | 0 |
| captura-2026-10-02-login | 10-02 00:57 sex | 1 | 0 | 1 | 1 | 1 | 0 |
| captura-2026-10-02-teleporte | 10-02 00:41 sex | 0 | 0 | 0 | 0 | 0 | 0 |
| captura-2026-10-03-b | 10-02 01:24 sex | 1 | 0 | 1 | 1 | 1 | 1 |
| captura-2026-10-03-boss | 10-03 13:30 sáb | 0 | 1 | 0 | 0 | 0 | 0 |
| captura-2026-10-05-essencia-od | 10-05 14:53 seg | 0 | 1 | 0 | 0 | 0 | 1 |
| captura-2026-10-05-login-odyle | 10-05 14:24 seg | 1 | 0 | 1 | 0 | 1 | 0 |
| captura-2026-10-05-teleporte-odyle | 10-05 14:17 seg | 0 | 0 | 0 | 0 | 0 | 0 |
| captura-2026-10-06-gartua | 10-06 17:25 ter | 0 | 0 | 0 | 0 | 0 | 0 |
| captura-2026-10-06-tempo-boss | 10-06 23:32 ter | 0 | 0 | 0 | 0 | 0 | 0 |

Resets entre as listas (pela tabela de `eventos.rs`, agora conferida pelo servidor): de 10-02 00:57 a
10-02 01:25, nenhum; de 10-02 01:25 a 10-05 14:27, quatro diários (sex, sáb, dom e seg às 04:00) e
nenhum semanal (o próximo era quarta 10-07 04:00).

#### 2.1.2 Os 73 ids do `0x610B` por dump (conferido)

"(mudou)" marca diferença entre as listas. Tipo `01`: os 8 bytes valem 25.200.000 como u64 LE, iguais
em todas as entradas e nos três logins. Tipo `00`: só o id. Coluna questlog: dungeon `id ÷ 100`
(seção 2.1.3), com dificuldade, tier e custo de Odyle do `getDungeon`.

| id | tipo | 10-02 00:57 | 10-02 01:25 | 10-05 14:27 | questlog (id ÷ 100) |
|---|---|---|---|---|---|
| 1 | 04 | 2 | 2 | 10 (mudou) | |
| 3 | 01 | 8 bytes | 8 bytes | 8 bytes | |
| 4 | 04 | 2 | 2 | 6 (mudou) | |
| 6 | 00 | sem valor | sem valor | sem valor | |
| 7 | 04 | 4 | 4 | 32 (mudou) | |
| 8 | 04 | 1 | 1 | 3 (mudou) | |
| 9 | 01 | 8 bytes | 8 bytes | 8 bytes | |
| 10 | 04 | 14 | 14 | 14 | |
| 11 | 04 | 2 | 2 | 2 | |
| 12 | 04 | 2 | 2 | 14 (mudou) | |
| 13 | 01 | 8 bytes | 8 bytes | 8 bytes | |
| 14 | 04 | 14 | 14 | 14 | |
| 15 | 04 | 14 | 14 | 14 | |
| 101 | 04 | 14 | 14 | 14 | |
| 102 | 04 | 1 | 1 | 5 (mudou) | |
| 103 | 04 | 1 | 1 | 5 (mudou) | |
| 201 a 206 | 01 | 8 bytes | 8 bytes | 8 bytes | |
| 10000001 a 10000012 | 00 | sem valor | sem valor | sem valor | |
| 60000001 | 04/0C | 155 | 130 (mudou) | 550 (+270) (mudou) | Energia Odyle (já conferida) |
| 60000002 | 04 | 2 | 2 | 6 (mudou) | |
| 60000003 | 04 | 2 | 2 | 10 (mudou) | |
| 60000004 | 04 | 2 | 2 | 10 (mudou) | |
| 60000005 | 04 | 2 | 2 | 10 (mudou) | |
| 60000006 | 04 | 3 | 3 | 9 (mudou) | |
| 60000007 | 04 | 35 | 35 | 35 | |
| 60000008 | 04 | 28 | 28 | 28 | |
| 60000009 | 04 | 10 | 10 | 10 | |
| 60000010 | 04 | 7 | 7 | 7 | |
| 60000101 | 04 | 2 | 2 | 10 (mudou) | 600001 Caverna de Krao, normal, tier 1, Odyle 40 |
| 60000201 | 04 | 7 | 7 | 7 | 600002 Caverna de Krao, easy, Odyle 20 |
| 60001101 | 04 | 7 | 7 | 7 | 600011 Desfiladeiro de Urugugu, easy, Odyle 20 |
| 60001201 | 04 | 2 | 2 | 10 (mudou) | 600012 Desfiladeiro de Urugugu, normal, tier 2, Odyle 40 |
| 60002101 | 04 | 7 | 7 | 7 | 600021 Templo Divino do Fogo, easy, Odyle 30 |
| 60002201 | 04 | 2 | 2 | 10 (mudou) | 600022 Templo Divino do Fogo, normal, tier 3, Odyle 40 |
| 60003101 | 04 | 7 | 7 | 7 | 600031 Draupnir, easy, Odyle 40 |
| 60003201 | 04 | 2 | 2 | 10 (mudou) | 600032 Draupnir, normal, tier 1, Odyle 40 |
| 60004101 | 04 | 7 | 7 | 7 | 600041 Ninho de Dramata Morto, easy, Odyle 40 |
| 60004201 | 04 | 7 | 7 | 7 | 600042 Ninho de Dramata Morto, normal, tier 4, Odyle 40 |
| 60005301 | 04 | 2 | 2 | 4 (mudou) | 600053 Base de Pesquisa Deus, hard, Odyle 40 |
| 60006301 | 04 | 2 | 2 | 4 (mudou) | 600063 Arcanis Despedaçada, hard, Odyle 40 |
| 60007101 | 04 | 7 | 7 | 7 | 600071 Ilha Aérea de Vakron, easy, Odyle 40 |
| 60007201 | 04 | 2 | 2 | 10 (mudou) | 600072 Ilha Aérea de Vakron, normal, tier 2, Odyle 40 |
| 60009101 | 04 | 7 | 7 | 7 | 600091 Gruta do Chifre Feroz, easy, Odyle 40 |
| 60009201 | 04 | 2 | 2 | 10 (mudou) | 600092 Gruta do Chifre Feroz, normal, tier 3, Odyle 40 |
| 60012101 | 04 | 7 | 7 | 7 | 600121 Berço do Vazio, easy, Odyle 40 |
| 60012201 | 04 | 7 | 7 | 7 | 600122 Berço do Vazio, normal, tier 4, Odyle 40 |
| 60013101 | 04 | 7 | 7 | 7 | 600131 Corredor da Ilusão, easy, Odyle 40 |
| 60014101 | 04 | 7 | 7 | 7 | 600141 Ilha do Sopro Azul, easy, Odyle 40 |
| 60015101 | 04 | 7 | 7 | 7 | 600151 Cidadela do Daeva Caído, easy, Odyle 40 |
| 70000001 | 04 | 3 | 3 | 3 | |
| 80000001 | 04 | 3 | 3 | 3 | |
| 90000001 a 90000006 | 04 | 4, 1, 4, 2, 4, 2 | iguais | iguais | |

Observação: a Odyle caiu 25 entre 00:57 e 01:25 de 10-02 sem nenhum ticket de dungeon mudar; 25 não
bate com nenhum `odyleEnergyCost` (20, 30, 40). O gasto foi em outra coisa (**hipótese**).

**`0x610C` (conferido, dois no total):**

| Dump | Hora | Corpo | Leitura |
|---|---|---|---|
| captura-2026-10-03-boss | 10-03 13:30:56 sáb | `00 04 0a000000 0e 02` | 1º byte 00; tipo 04, ticket 10, valor 14 (igual aos logins); resto `02` |
| captura-2026-10-05-essencia-od | 10-05 14:54:04 seg | `01 0c 01879303 a604 b602 01 0a000000` | 1º byte 01; tipo 0C, ticket 60.000.001, valor 550, extra 310; resto `01 0A000000` |

O resto do segundo traz u32 10, a quantidade que a essência somou (300 → 310 na tela): resto =
`[u8 motivo][u32 quantidade]` é **hipótese** com uma amostra.

#### 2.1.3 Ligação ticket → dungeon do questlog (hipótese forte)

Regra: dungeon = `ticket ÷ 100` (60001201 → 600012). Script `trabalho-F6/casar.py`.

- **Conferido:** os 21 tickets de 60.000.101 a 60.015.101 caem em 21 ids que existem no `getDungeons`
  (733 dungeons, 19 páginas), todos `mainCategory: party`. Controle: um id sorteado entre 600.000 e
  600.160 existe na lista em 25% das vezes; o controle é fraco porque cada dungeon tem variantes com
  ids seguidos.
- **Conferido (a prova mais forte):** a dificuldade que o `getDungeon` dá separa o comportamento dos
  valores entre 10-02 e 10-05 sem exceção:

| Grupo (questlog) | Tickets | 10-02 → 10-05 |
|---|---|---|
| `easy` | 11 | 7 → 7, todos |
| `normal`, tier 1 a 3 | 6 | 2 → 10, todos |
| `normal`, tier 4 | 2 | 7 → 7, os dois |
| `hard` | 2 | 2 → 4, os dois |

  Vale também na Caverna de Krao, onde o dígito Y vem trocado (60000101 é normal e 60000201 é easy):
  os valores seguem a dificuldade do questlog mesmo com o dígito trocado.
- **Hipótese:** o valor é o número de entradas que o jogo mostra para aquela dungeon e dificuldade.
  O print (1.6, item 1) decide; o experimento (b) confere a ligação de um ticket.
- Os casamentos de id pequeno (ticket 7, 8, 10, 12, 14 com as arenas de mesmo id; 103; 10.000.0xx e
  90.000.002 por resto) são ruído: ids de 1 a 14 existem de qualquer jeito na lista. Descartados.
- O `ceilingCount` do questlog (1 e 3 nos fáceis; 14, 21, 28 nos normais por tier) não bate com o
  valor do ticket. Significado desconhecido; não usar.

#### 2.1.4 Faixas sem fonte

Ids 1 a 15, 101 a 103, 201 a 206, 10.000.001 a 10.000.012 (tipo 00), 60.000.002 a 60.000.010,
70.000.001, 80.000.001 e 90.000.001 a 90.000.006. Testados no questlog sem resultado: `getQuest`
(2011008 e 6401221: `null`), `getAchievement` (2012033: `null`), `getItem` (60000001: erro 500),
`searchEntities` "Odyle" (lista vazia). Vários sobem em múltiplos de 4 entre 10-02 e 10-05 (+8, +4,
+28, +12), o que combina com recarga por reset diário (**hipótese**); 8 (1 → 3), 60000006 (3 → 9) e
os difíceis (2 → 4) não fecham com recarga inteira por dia e podem ter teto ou uso (**hipótese**).
Ficam fora até o print.

#### 2.1.5 Reset no pacote (conferido)

Script `trabalho-F6/horas.py`: todo u64 que, lido como ms Unix, cai entre 2026-09-01 e 2026-11-30.

| Opcode | Visto | O que traz |
|---|---|---|
| `0xE223` | 3 logins | `[u64 ms][4 bytes 00]`: 10-02 04:00:00,000 (dois logins de 10-02) e 10-06 04:00:00,000 (login de 10-05 14:27). O próximo reset diário. |
| `0x8D1C` | 2 logins (10-02) | 26 bytes: 9 bytes 00, u64 ms, 1 byte 00, u64 ms; os dois u64 = 10-02 04:00:00,000. Não veio no login de 10-05. |
| `0xE256` | 3 logins | Grupos com janela (abaixo): diária de 04:00:01 a 04:00:00,999 do dia seguinte; semanal de qua 09-30 04:00:01 a qua 10-07 04:00:00,999. |
| `0x568E` | 2 vezes, fora do login (10-02 01:26:21 e 10-05 14:55:04), corpos iguais | 7 entradas com hora: 5 em qua 10-07 04:00, uma em dom 11-01 04:00, uma em 2099-12-31. |

Com isso o reset diário (04h) e o semanal (quarta 04h) de `eventos.rs:119-132`, que vinham do
shugo.gg, passam a ter confirmação do servidor. O `0x568E` com 11-01 04:00 (dia 1 do mês) sugere um
reset mensal às 04h (**hipótese**, uma amostra).

Os 8 bytes do tipo `01` no `0x610B` valem 25.200.000: em ms, 7 h, a hora UTC do reset (07:00 UTC =
04:00 Brasília). **Hipótese**: iguais em todas as 9 entradas e nos 3 logins, sem variação para testar.

Não há, em nenhum pacote visto, reset por ticket nem envio de `0x610C` no reset com o jogador online
(nenhuma captura cruza as 04:00). Daí a marca de valor velho no modelo (2.3).

#### 2.1.6 Candidatos a "semanais" (conferido o formato, hipótese o significado)

`0xE256` (1685 bytes nos 3 logins; a leitura abaixo fecha no último byte nos 3):

| Grupo | tipo | Janela | Ids |
|---|---|---|---|
| 6401212 (10-02) / 6401216 (10-05) | 1 | 04:00:01 do dia até 04:00:00,999 do dia seguinte | 10 (2011008 a 2011158 em 10-02; 2011004 a 2011158 em 10-05) |
| 6401221 | 2 | qua 09-30 04:00:01 até qua 10-07 04:00:00,999 | 10 (2012033 a 2012345), iguais nos 3 logins |
| 6403141 | 3 | 2026-07-15 04:00:01 até 2099-01-20 04:00:00,999 | 54 (2013001 a 2013054) |
| 6403131 | 4 | idem | 54 (2014001 a 2014054) |
| 0 | 5 e 6 | horas 0 | nenhum |

Todos os contadores e máscaras valem 0 nos 3 logins. Grupo diário que troca de id a cada dia, grupo
semanal com janela de quarta a quarta: cara de missões diárias e semanais (**hipótese**; sem nome no
questlog).

`0x568E`: ids 1169, 1172, 1209, 1210, 1211 (prazo qua 10-07 04:00), 1254 (2099-12-31 12:00), 1262
(dom 11-01 04:00); o u32 depois de cada id vale 0. Chegou no meio da sessão, entre `0x371C`/`0x371D`;
o que dispara é desconhecido.

### 2.2 Layout para o PROTOCOLO.md

**§7d revisada (trechos a trocar):**

```
## 7d. Tickets de conteúdo `0x610B` e `0x610C` (Energia Odyle e entradas de dungeon)

`0x610B` chega no login com a lista inteira:

varint  n
n ×     u8      tipo
        u32     id
        8 bytes (só com tipo & 0x01; u64 LE 25.200.000 nas 9 entradas desse tipo, nos 3 logins)
        varint  valor (só com tipo & 0x04)
        varint  extra (só com tipo & 0x08)

Nas três listas capturadas (logins de 2026-10-02 00:57 e 01:25, este no arquivo
captura-2026-10-03-b, e de 2026-10-05 14:27; 73 entradas cada) a leitura fecha no último byte.
Tipos: 0x00 ×13, 0x01 ×9, 0x04 ×51 (×50 e um 0x0C na de 10-05).

25.200.000 ms = 7 h = 07:00 UTC, a hora do reset (hipótese).

Entradas de dungeon de grupo (hipótese, conferir com a tela): id ÷ 100 = id da dungeon no questlog
(60001201 → 600012, Desfiladeiro de Urugugu, normal). Os 21 ids de 60.000.101 a 60.015.101 caem em
dungeons party do getDungeons, e a dificuldade separa os valores entre 10-02 e 10-05: easy 7 → 7
(11), normal tier 1 a 3 2 → 10 (6), normal tier 4 7 → 7 (2), hard 2 → 4 (2). Entre essas listas
houve 4 resets diários e nenhum semanal.

0x610C = [u8 ?][uma entrada como acima][resto]. Resto `01 0A000000` no uso de essência OD
(+10 na carregada): [u8 motivo][u32 quantidade] (hipótese, uma amostra).
```

**§7f nova:**

```
## 7f. Horas de reset `0xE223`, `0x8D1C`, `0xE256`, `0x568E` (conferido nos logins)

0xE223 (login): [u64 ms Unix do próximo reset diário][4 bytes 00]. 3 logins, sempre 04:00 Brasília.
0x8D1C (login de 10-02): [9 bytes 00][u64 ms][u8 00][u64 ms], os dois = próximo reset diário.
0xE256 (login):
  u8      n grupos
  n ×     u32 grupo, u8 tipo, u64 início ms, u64 fim ms, u8 m
          m × u32 grupo (repete), u32 id, [u8 máscara], u32 (0 nos 3 logins)
  A máscara vem na entrada de índice múltiplo de 8 contando o pacote inteiro (0, 8, 16, ...);
  antes ou depois do último u32 não dá para saber (tudo 0). Tipo 1: janela diária; tipo 2:
  semanal (qua 04:00:01 a qua 04:00:00,999); 3 e 4: 2026-07-15 a 2099.
0x568E (fora do login): [u16 0][u8 n] n × [u32 id][u32 0][u64 ms]. Prazos na quarta 04:00,
  no dia 1 do mês 04:00 e em 2099.
```

### 2.3 Modelo

Núcleo (`crates/nucleo`):

```rust
// medicao/medidor.rs
pub struct TicketVisto { pub valor: Option<u64>, pub extra: Option<u64>, pub chegou: Hora }

pub struct Medidor {
    // ...
    pub odyle: Option<(u64, Option<u64>)>,          // fica como está
    /// Último valor de cada ticket e quando chegou. Não zera na troca de conexão nem no Zerar:
    /// é do personagem, e o login manda a lista inteira de novo.
    pub tickets: BTreeMap<u32, TicketVisto>,
}

/// 0x610B: troca a lista inteira (troca de personagem incluída).
pub fn registrar_tickets(&mut self, lista: Vec<Ticket>, hora: Hora);
/// 0x610C: troca uma entrada; a Odyle continua como hoje.
pub fn registrar_ticket(&mut self, ticket: Ticket, hora: Hora);
```

Overlay (`crates/overlay`):

```rust
// entradas.rs (novo, no molde de eventos.rs: tabela fixa com fonte e data no cabeçalho)
pub struct Entrada { pub ticket: u32, pub dungeon: &'static str, pub dificuldade: &'static str }
pub const ENTRADAS: &[Entrada] = &[ /* só os ids confirmados pelo print, na ordem da tela do jogo */ ];

/// Chegou antes do último reset diário: pode ter mudado.
pub fn velho(chegou: Hora, agora: Hora) -> bool;
```

`velho` usa o `Evento` "Reset diário" de `eventos.rs` (início do período = agora menos o tempo
desde o último 04:00). O checklist usa a mesma conta com o "Reset semanal".

### 2.4 Onde encaixa no código

| Arquivo | Linha | Mudança |
|---|---|---|
| `crates/nucleo/src/protocolo/combate.rs` | 222-269 | Nada no parser; só a doc de `Ticket`/`tickets` (três listas). |
| `crates/nucleo/src/medicao/sessao.rs` | 254-268 | `TICKETS` chama `registrar_tickets(lista, hora)`; `TICKET_MUDOU` passa `hora`. |
| `crates/nucleo/src/medicao/medidor.rs` | 268-270, 511-516 | Campo `tickets` e as duas funções; Odyle sem mudança. |
| `crates/overlay/src/eventos.rs` | 119-132, 141 | Sem mudança na tabela; `velho` usa `estado()`. Dica dos resets pode citar o `0xE223`. |
| `crates/overlay/src/entradas.rs` | novo | Tabela fixa e `velho`. |
| `crates/overlay/src/janela.rs` | 80-85, 526-532, 1196-1198 | `Tela::Entradas`, botão no topo, linha-resumo nos eventos expandidos. |
| `crates/overlay/src/janela/entradas.rs` | novo | A tela (no molde de `janela/chefes.rs`). |
| `crates/overlay/src/config.rs` | 22-52, 54-72 | `entradas_ocultas` e `checklist` com padrão vazio. |
| `crates/nucleo/tests/parsers.rs` | 265-313 | Fixture da terceira lista (10-02 01:25, Odyle 130). |
| `crates/nucleo/tests/medidor.rs` | 557-565 | Teste de `registrar_tickets` e da hora. |
| `PROTOCOLO.md` | 365-400 | §7d revisada e §7f nova (2.2). |
| `README.md` | 222-256 | Tela Entradas, linha nos eventos, campos novos do config. A tabela de privacidade (316-331) só muda se o dono escolher `getDungeon` em tempo de execução. |

### 2.5 Riscos

- **Ligação ticket → dungeon é inferência.** Mitigação: só entra na tabela o id confirmado por print
  e, de preferência, pelo experimento (b).
- **O 7 parado pode ter outro sentido** (padrão de quem nunca entrou, conteúdo travado por tier ou
  outra coisa). O print decide se mostra.
- **Sem `0x610C` no reset**, o valor fica velho até o próximo login ou entrada. Mitigação: marca de
  velho (critério 4). O experimento (c) diz se o risco existe.
- **Axon aberto depois do login** não tem lista (mesmo caso da Odyle). Mitigação: aviso na tela.
- **Bit de tipo novo** descarta a lista inteira (`combate.rs:259`). Hoje só `00`, `01`, `04`, `0C`.
- **Patch do jogo** pode trocar opcode ou ids (`opcodes.rs:1-4` já avisa). A tabela fixa precisa de
  revisão a cada patch; o questlog muda nome sem aviso.
- **Amostra pequena:** um personagem, três listas, nenhum reset semanal entre elas.
- **Troca de personagem** sem captura: não se sabe se o `0x610B` vem de novo na seleção (**hipótese**
  que sim, por ser enviado no login). `registrar_tickets` troca a lista inteira para não misturar.

### 2.6 Plano de verificação

1. Fixture da terceira lista em `tests/parsers.rs` → verifica: `CARGO_TARGET_DIR=target/agente-F6
   cargo test -p nucleo --test parsers` (rodar pela ferramenta Bash).
2. `registrar_tickets` troca a lista e guarda a hora; `registrar_ticket` troca uma → verifica: teste
   novo em `tests/medidor.rs`.
3. `velho` com instantes conferidos fora do Rust: chegou seg 10-05 14:27 → falso às ter 10-06
   03:59:59 e verdadeiro às 04:00:00 → verifica: teste em `entradas.rs`, no molde de
   `eventos.rs:220-306`.
4. Checklist: item semanal marcado ter 10-06 → feito até qua 10-07 03:59:59, desfeito às 04:00:00 →
   verifica: teste.
5. Replay de `captura-2026-10-05-login-odyle` → verifica: a tela mostra os valores da tabela 2.1.2.
6. Prints do dono (1.6) → verifica: cada linha da tela igual à do jogo no mesmo instante.

### 2.7 Tamanho

**M.** Núcleo P (um mapa e duas funções). Overlay M (tela nova, tabela, linha nos eventos, dois
campos de config, testes). Docs P. A tabela de nomes fica bloqueada até o print; o resto pode ser
feito antes com a tabela vazia.

### 2.8 Dependências

- **F4 (configurações):** onde ficam "esconder ticket" e o checklist na tela de configurações
  (`janela/configuracoes.rs`). F6 só define os campos.
- **F5 (alertas de reset):** usa os resets de `eventos.rs` (agora conferidos pelo `0xE223`/`0xE256`)
  e pode usar `Medidor::tickets` para o texto do alerta (ex.: quantas entradas restam). F6 funciona
  sem F5.
- **Dono:** prints e capturas da seção 1.6, antes de preencher `ENTRADAS`.

## 3. URLs consultadas (2026-10-08)

- `https://questlog.gg/aion-2/database` e `https://questlog.gg/aion-2/database/dungeons` (HTML, para
  achar os scripts).
- `https://cdn.questlog.gg/_static/aion-2/_nuxt/*.js`: os 100 scripts citados na página, mais
  `CsS2a61R.js` (lista de dungeons, usa `database.getDungeons`) e `kLsxZ73-.js`. Lidos como texto,
  não executados. Procs achados: `getDungeons`, `getDungeon`, `getQuest`, `getAchievement`,
  `getDaevaPass`, `getRegion`, `searchEntities` e outros.
- `https://questlog.gg/aion-2/api/trpc/database.getDungeons?input={"language":"pt","page":N}`,
  N = 1 a 19 (733 dungeons).
- `https://questlog.gg/aion-2/api/trpc/database.getDungeon?input={"id":"<id>","language":"pt"}` para 10,
  600001, 600002, 600011, 600012, 600013, 600021, 600022, 600031, 600032, 600041, 600042, 600053,
  600063, 600071, 600072, 600091, 600092, 600121, 600122, 600131, 600141, 600151.
- `.../database.getQuest` com id 2011008 e 6401221 (`null`); `.../database.getAchievement` com 2012033
  (`null`); `.../database.getItem` com 60000001 (erro 500); `.../database.searchEntities` com
  "Odyle" (vazio).

Arquivos baixados em `scratchpad\refino\baixado-F6\`; scripts em `scratchpad\refino\trabalho-F6\`
(`tickets.py`, `horas.py`, `e256g.py`, `outros.py`, `vizinhos.py`, `dungeons.py`, `casar.py`,
`tabela.py`, `tabela_final.py`, saída `tabela_tickets.md`).
