# F7. Busca de item e lista de desejos

> Anexo do refinamento da 0.14.0 (2026-10-08), escrito por um agente e revisado na integração em
> `docs/0.14.0.md`. Os scripts e respostas citados em `<scratchpad>` ficaram na máquina do
> refinamento e não foram versionados; os pacotes e números que os testes usam estão neste texto.

Refinamento da frente F7 da 0.14.0 (docs/0.14.0.md). Cada afirmação sobre API ou código vem como
**conferido** (com a evidência: resposta salva, URL ou arquivo:linha) ou **hipótese**.

Pastas citadas, todas dentro do scratchpad da sessão
(`<scratchpad>`):

- `refino\baixado-F7\NN-*.json`: as 30 respostas cruas do questlog de 2026-10-08 (citadas pelo número).
- `refino\trabalho-F7\manifesto.jsonl`: URL, status HTTP, tempo, bytes e cabeçalhos de cache de cada uma.
- `refino\trabalho-F7\*.py`: os scripts (pedir, resumir, cruzar, gerar a fixture).
- `itens\` e `npcs\`: 121 `getItem` e os 24 `getNpc` dos chefes de Altgard, baixados em 2026-10-06 numa
  sessão anterior (só leitura aqui).
- `questlog-js\`: os bundles JS do site (só grep).

Pedido do dono no meio do refinamento (2026-10-08): a lista de desejos também mostra se o item sai de
craft (e que dá para craftar) e, quando cai de mob, qual mob é e onde ele fica. Isso entrou nas histórias
H2 e H5 e na seção "Onde conseguir".

---

## 1. Negócio

### Problema

Hoje o Axon vai do chefe para o item: clicar num chefe na tela Bosses abre o que ele deixa cair
(README.md:174-196). O caminho inverso não existe. O jogador sabe que quer as Luvas de Gartua e não
tem como perguntar ao Axon de onde elas vêm (chefe, baú, craft, missão, dungeon), nem como lembrar o
que procura, nem como ver de relance que o chefe que derruba o que ele quer renasce em 8 minutos.

### Histórias

- **H1. Buscar.** Como jogador, digito parte do nome ("luvas gartua", sem acento, minúsculas) e vejo os
  itens que batem, com ícone e cor da raridade, filtrando por categoria (armadura, arma, acessório,
  consumível, material).
- **H2. Onde conseguir.** Ao abrir a ficha de um item (da busca, da lista ou do painel de drops), vejo
  de onde ele vem: cai de qual monstro ou chefe (nível, chance, região), vem em qual baú e de quem cai
  o baú, se dá para craftar (profissão, maestria, ingredientes com quantidade), se é recompensa de
  missão, dungeon, conquista ou pedido de suprimento, se é vendido por NPC ou sai de coleta.
- **H3. Marcar.** Marco ★ no item (na busca, na ficha, no painel de drops) e ele entra na lista de
  desejos; ★ de novo tira.
- **H4. Ver a lista.** Abro a lista e vejo cada desejo com a melhor fonte resumida ("Profanador Newbold
  17,3%, Altgard, renasce 22:20" ou "Craft: Alquimia, iniciante 20") e a prioridade.
- **H5. Onde fica o mob.** Para cada monstro que derruba o item, vejo a região em que ele aparece
  (ex.: "Fada Contaminada, nv 47, Altgard").
- **H6. Bosses.** Na tela Bosses, o chefe que derruba algo da lista ganha ★, o mouse diz quais itens
  e com qual chance, e posso filtrar "só chefes com desejo".
- **H7. Alertas (F5).** Quero ser avisado quando um chefe com item da minha lista renasce (ou X min
  antes), só para os itens em que liguei o alerta.
- **H8. Configurar.** Ajusto prioridade e alerta por item, a chance mínima para um chefe contar como
  "derruba", e se o destaque aparece na tela Bosses.

### Critérios de aceite

1. Buscar "newb" mostra os 17 itens do Newbold (as 7 peças em duas variantes, a Obra-prima, o Título e o
   baú), na mesma ordem do questlog (conferido: `08-busca-parcial.json` = `01-busca-newbold.json`, 17
   itens). "bau de saque de newbold" traz o Baú de Saque de Newbold em primeiro (conferido:
   `07-busca-bau-sem-acento.json`).
2. A ficha da Luvas de Newbold 210540076 mostra "Cai de Profanador Newbold, nv 45, Altgard: 17,3%"; a da
   210540129 (mesmo nome) mostra "No Baú de Saque de Newbold: 3,6%" e que o baú cai do Profanador
   Newbold a 100% (conferido: `16`, `28` e `30-item-bau-newbold.json`).
3. A ficha da Pedra de Mana Intermediária mostra "Craft: Alquimia" com 5× Pedra de Mana Inferior,
   3× Pó de Pedra Espiritual, 2× Tinta Avançada (Vinculado) (conferido: `17-item-craft-pedra-mana.json`).
4. Com a Luvas de Newbold (qualquer das duas) na lista e a lista de Altgard em dia, o Profanador Newbold
   aparece com ★ na tela Bosses e o mouse diz o item e a chance.
5. Com o Peitoral da Fantasia na lista e a chance mínima em 0,5%, nenhum chefe de Altgard ganha ★ (os 20
   que derrubam dão de 0,025% a 0,081%: 5 chefes a 0,025%, 8 a 0,053%, 7 a 0,081%); com a chance mínima
   em 0, os 20 ganham (conferido no `getItem` de 2026-10-06, `itens\210130005.json`, cruzado com
   `21-regiao-altgard.json`).
6. Fechar e abrir o Axon mantém a lista. O `config.json` não tem nome, ícone, chance nem código de NPC
   (só o que a seção "Dados em disco" lista).
7. Sem internet: a lista abre com "Item 210540076" e um quadrado cinza no lugar do ícone, sem fonte, e a
   tela Bosses diz que os desejos não chegaram; reabrir a lista pede de novo.
8. Nenhuma tela mostra posição, distância ou quem está lutando com um chefe (regra do projeto e
   memória do dono de 2026-10-06). "Onde fica" é só o nome da região do questlog.

### Escopo

Dentro:

- Busca de itens por texto com filtro de categoria e páginas de 40.
- Seção "Onde conseguir" na ficha do item (`janela/item.rs`), para qualquer item, inclusive os do
  painel de drops.
- ★ na busca, na ficha, nas linhas e no conjunto do painel de drops.
- Tela da lista de desejos (nova `Tela::Desejos`) e ficha no painel lateral.
- Destaque e filtro na tela Bosses.
- O predicado "chefe com desejo" e as transições (renasceu, faltam X min) para a F5 consumir.
- Linha nova na tabela de Privacidade do README (o texto da busca vai ao questlog).

Fora:

- Posição, coordenada, subzona e distância de NPC: o `getNpc` só dá a região (conferido:
  `23-npc-fada-contaminada.json` não tem campo de coordenada; `npcIsFoundInRegions` traz
  `{"name":"Altgard","count":15}`), e a regra do projeto proíbe mostrar posição viva.
- Inventário e "já consegui": o Axon não lê o loot (conferido: o PROTOCOLO.md não tem "loot",
  "saque" nem "drop"; a única linha com "item" é cabeçalho de tabela, PROTOCOLO.md:10). Remover da
  lista é manual.
- Quantidade desejada (ex.: 5 lascas), nota de texto por item.
- Preço de mercado e casa de leilão.
- Busca de NPC, missão ou dungeon pelo nome (o `searchEntities` permitiria; fica para depois).
- Notificação em si (som, toast, piscar): é da F5.

### Configurações (todas)

Proposta do que é "100% configurável", com recomendação:

| Configuração | Onde | Padrão | Entra? |
|---|---|---|---|
| Itens da lista (códigos) | por item | vazia | entra |
| Prioridade: Alta, Média, Baixa (ordena a lista e a dica da Bosses) | por item | Média | entra (depende da pergunta P1) |
| Alertar este item (F5) | por item | ligado | entra (depende de P1) |
| Destacar chefes com desejo na tela Bosses | global | ligado | entra |
| Chance mínima para um chefe contar como "derruba" (0 = qualquer) | global | 0,5% | entra (valor: P5) |
| Filtro "só chefes com desejo" na Bosses, lembrado | global | desligado | entra |
| ★ nos itens do painel de drops | global | sempre | fica de fora (sem motivo para desligar) |
| Quais fontes mostrar (drop, baú, craft, missão...) | global | todas | fica de fora (a ficha mostra o que existir) |
| Lista por personagem ou por classe | perfil | lista única | pergunta P3 |
| Gatilhos de alerta (renasceu, X min antes, vivo ao entrar na região) | F5 | da F5 | entra na F5 |

### Perguntas ao dono

- **P1. Disco.** A decisão diz "só os códigos dos itens podem ir para o config.json". Lida ao pé da
  letra, prioridade e alerta por item ficam de fora e a lista vira `[210540076, 210540129]` com três
  opções globais. Proposta: códigos mais as preferências que o próprio jogador escolhe (prioridade,
  alertar), nenhum dado baixado. Qual das duas?
- **P2. Teclado.** O overlay não recebe teclado: `WS_EX_NOACTIVATE` é reaplicado a cada quadro
  (conferido: `crates/overlay/src/janela.rs:1615` chama `manter_sem_ativar`, `janela.rs:2151-2168`;
  README.md:50-51 e `janela/configuracoes.rs:171-172` dizem que por isso os atalhos se trocam no
  arquivo). Uma caixa de texto não funciona assim, e pior: com o jogo em foco, o que o jogador digitar
  vira comando no jogo. Opções:
  - **(a) Foco só enquanto digita.** Clicar no campo tira o `WS_EX_NOACTIVATE` e traz o overlay para o
    primeiro plano; Enter, Esc ou clicar fora devolvem o bit e o foco ao jogo (`SetForegroundWindow`
    na janela do jogo). Efeitos: enquanto digita, o jogo não recebe tecla (o que se quer) e os atalhos
    globais se desregistram, porque só valem com o jogo em primeiro plano (conferido:
    `crates/overlay/src/bandeja.rs:251`). Hipóteses a conferir no jogo: (1) enquanto o Axon tem o
    foco, nenhuma letra digitada chega ao jogo (se o jogo ler o teclado por Raw Input ou DirectInput
    registrado para segundo plano, as letras acionam skill mesmo sem foco, e aí (a) cai); (2) o Windows
    deixa o Axon devolver o foco (ele é o primeiro plano naquele instante); (3) o jogo em janela sem
    borda não pisca nem minimiza.
  - **(b) Teclado na tela.** Letras A-Z, espaço e ⌫ desenhados no overlay, por clique. Funciona com
    `WS_EX_NOACTIVATE` e sem mexer no foco. A busca aceita minúsculas, sem acento e palavra pela
    metade (conferido: `02`, `07`, `08`), então "gartua" ou "newb" bastam.
  - **(c) Sem texto.** Só navegar por categoria e marcar ★ no painel de drops dos chefes. Não cobre
    "busca" do pedido.
  - Recomendação: (a), com (b) como reserva se o teste no jogo falhar em qualquer das três hipóteses
    (a primeira é a que mais pesa: letra virando skill). Colar da área de
    transferência fica de fora: exige ler o clipboard (a feature `Win32_System_DataExchange` não está
    no `crates/overlay/Cargo.toml`) e o jogador teria de digitar o nome em outro lugar antes.
- **P3. Por personagem.** Lista por personagem grava o nome do personagem no `config.json`. Opções:
  lista única (recomendada), por classe (sem nome) ou por personagem (nome no disco; o
  `jogadores.json` já guarda nomes, README.md:135-138). Qual?
- **P4. Variantes com o mesmo nome.** "Luvas de Newbold" tem dois códigos: 210540076 cai do chefe
  (17,3%) e 210540129 vem do baú (3,6%) (conferido: `16` e `28`). A busca do Newbold tem 7 pares assim
  (conferido: `01`). Proposta: a busca agrupa nomes iguais numa linha ("2 variantes") e a ★ marca
  todas; a ficha mostra cada variante com a sua fonte. Ou marcar uma a uma?
- **P5. Chance mínima padrão.** 0,5% tira o ruído dos itens genéricos (Peitoral da Fantasia: 20 chefes
  de 0,025% a 0,081%) e mantém peça de conjunto (17,3%) e peça de baú (2,5% a 3,6%). Outro valor?
- **P6. Região do monstro.** Mostrar a região de cada monstro custa um `getNpc` por monstro (39,6 KB
  na Fada Contaminada, conferido: `23`). Proposta: só os 5 de maior chance, ao abrir "Onde conseguir",
  só na memória. Gravar a região junto do `npcs-pt.json` (que já vai para o disco,
  README.md:272-277) aceleraria, mas é dado baixado novo no disco. Memória só, ou pode?
- **P7. Privacidade.** O texto da busca vai para o questlog na URL. Pode, com a linha nova na tabela
  de Privacidade do README?
- **P8. Botão.** ★ como 9º ícone do cabeçalho (hoje são 8: ✕ ‹ ⚙ ☰ ♛ ▭ ⧉ ↺, `janela.rs:509-547`)
  ou entrada só pela tela Bosses? Largura: cada ícone ocupa 26 px (conferido: `janela/visual.rs:64`),
  a folha tem margem de 8 px dos dois lados e espaçamento zero (conferido: `janela.rs:1686,1694`), o
  que deixa 454 px. No pior caso (vendo luta passada, o ↺ vira o botão "● Ao vivo",
  `janela.rs:541-544`): logo 26 + 6 + 8 ícones (208) = 240 px, mais o texto "AXON" e o "● Ao vivo"
  (12 pt + 14 px). Hipótese: os dois textos somam menos de 140 px e sobram uns 70 px; conferir com
  `--replay ... --lutas`.

---

## 2. Técnico

### API do questlog

Base: `https://questlog.gg/aion-2/api/trpc/database.<proc>?input=<json url-encoded>`, GET, já usada em
`crates/nucleo/src/medicao/catalogo.rs:163,751-760`. Todas as respostas vêm em `result.data`.

#### Busca: `getItems` (conferido, `01` a `15`, `25` a `27`)

Entrada: `{"language":"pt","page":1,"searchTerm":"newb","mainCategory":"armor"}`.

| Campo | Comportamento | Evidência |
|---|---|---|
| `page` | obrigatório, número | `04`: 400 "Invalid input: expected number, received undefined" |
| `searchTerm` | texto; sem acento e sem caixa; palavra pela metade casa; 1 letra funciona; vazio lista tudo | `02`, `07`, `08`, `09` (568 itens), `10` (9.260 itens, 232 páginas) |
| código numérico no `searchTerm` | não casa | `11`: 0 itens |
| `mainCategory` | filtra (texto) | `12`: "Newbold" + "usable" = 2 itens (Título e Baú) |
| `subCategory` | existe (texto) | `05`: 2 erros "expected string, received number" para `mainCategory:1` e `subCategory:1`; `grade` não acusa erro (`13`) |
| `sortBy` | existe (texto); valores aceitos não conferidos | `27`: 400 "expected string, received object" |
| `grade`, `grades`, `level`, `minLevel`, `maxLevel`, `itemLevel` | ignorados (mesmo resultado, sem erro) | `13`, `14`, `26` |
| `hitsPerPage`, `limit`, `pageSize`, `raceId`, `itemTier`, `characterClass`, `exchangeType` | ignorados | `25` |

Saída (resumo de `01-busca-newbold.json`):

```json
{"pageData":[{"compoundId":"item-210140076","id":"210140076","name":"Peitoral de Newbold",
  "icon":"/assets/.../Icon_Equip_AR_L_0013_T03_Torso.Icon_Equip_AR_L_0013_T03_Torso","grade":31,
  "language":"pt","dbType":"item","mainCategory":"armor","subCategory":"torso",
  "createdAt":"2026-09-21","isDisabled":false}, ...17 itens],
 "pageCount":1,"currentPage":1,
 "facetDistribution":{"grade":{"21":1,"31":14,"41":1,"71":1},"createdAt":{"2026-09-21":17}}}
```

- 40 itens por página, sem repetição entre páginas (conferido: `02` e `03`, 40 + 40, "peitoral" tem
  384 itens e `pageCount` 10).
- Sem nível do item na listagem: só ícone, nome, raridade e categorias. Nível e fontes exigem `getItem`.
- Filtro de raridade e de nível: não existem no servidor (conferido acima). A `facetDistribution.grade`
  dá a contagem por raridade do resultado inteiro, que serve para mostrar "14 épicos"; filtrar por
  raridade só dá dentro das páginas já baixadas.
- Ordem: por relevância quando o termo é seletivo (o baú do Newbold veio primeiro em `07`); com termo
  amplo, por código crescente (`02`). Hipótese: motor tipo Meilisearch. Em `07` a soma da faceta (173)
  não bate com `pageCount` 2 (seria 5): não conferido, não influi no desenho.
- `isDisabled` vem em cada item. Hipótese: item desativado no jogo; a busca esconde os `true`.

#### Busca de entidades: `searchEntities` (conferido, `06` e `22`)

O que a caixa de busca do site chama (conferido: `questlog-js\ByJQ2KcU.js`,
`d.database.searchEntities.useQuery(()=>({searchTerm:f.value,language:l.value,extendSearch:!1})`).
Devolve uma lista plana misturando item, NPC, missão, conquista, pedido de suprimento e título, com o
nome marcado em HTML (`Profanador <span class='font-black'>Newbold</span>`), sem página: 18 entradas
para "Newbold", 23 com `extendSearch:true`. Para a lista de desejos o `getItems` serve melhor (só item,
página, filtro de categoria, a raridade de cada item e a contagem por raridade). Fica registrado para
uma busca de NPC no futuro.

#### Ficha e "onde conseguir": `getItem` (conferido)

O `getItem` que a ficha já pede (`catalogo.rs:473-485`) traz as listas reversas. Nomes das 19 relações
de item que o site sabe mostrar (conferido: `questlog-js\BjBI1uw7.js`, objeto de tabelas com
`label:e(\`a2.dbTableHeader...\`)`):

| Relação | O que é | Visto em resposta |
|---|---|---|
| `itemIsDroppedByNpcs` | NPCs que derrubam, com `level`, `chance`, `countMin/Max` | `16`, `30`; 121 arquivos de 2026-10-06 |
| `itemIsContainedInItems` | baús que contêm, com `chance` dentro do baú | `20`, `28`, `29` |
| `itemContainsItems` | o que o baú contém (já usado no painel de drops) | `30` |
| `itemIsOutputOfRecipes` | receitas que produzem, com `recipeInputItems` (id, nome, `quantity`, `craftableRecipe`) e `recipeOutputItems` | `17`, `18` |
| `itemIsInputOfRecipes` | receitas em que é ingrediente | `17`, `18` |
| `itemIsRewardOfQuests` | missões | `itens\..\item-guarda.json` (2026-10-06) |
| `itemIsRewardOfDungeons` | dungeons, com `chance` | `itens\610530001.json` (2026-10-06): 49 dungeons |
| `itemIsSoldByNpcs` | NPC vendedor; colunas `price`, `limitCount`, `conditions` | só no bundle |
| `itemIsObtainedFromGatherables` | coleta | só no bundle |
| `itemIsRewardOfAchievements`, `itemIsRewardOfSupplyRequests`, `itemIsRewardOfDaevaPasses` | conquista, pedido de suprimento, passe | só no bundle |
| `itemIsRequiredBySupplyRequests`, `itemExtractsToSkins`, `itemIsPartOfItemSets`, `itemGrantsStatusEffects`, `itemRewardsSkins/Pets/Wings` | usos do item, fora de "onde conseguir" | `16`, `18` |

Exemplo (`16-item-luvas-newbold.json`, só o que importa):

```json
{"id":"210540076","name":"Luvas de Newbold","grade":31,"mainCategory":"armor","subCategory":"gloves",
 "itemIsDroppedByNpcs":[{"id":"2400424","name":"Profanador Newbold","level":45,"chance":0.1734104,
   "countMin":1,"countMax":1,"grade":null,"subCategory":null,"mainCategory":"monster","dbType":"npc"}]}
```

`28-item-luvas-newbold-129.json`: sem `itemIsDroppedByNpcs`; `itemIsContainedInItems`:
`[{"id":"533700387","name":"Baú de Saque de Newbold (Vinculado)","chance":0.035714285}]`.
`30-item-bau-newbold.json`: o baú cai do `2400424` com `chance: 1`.

`17-item-craft-pedra-mana.json`, `itemIsOutputOfRecipes[0]`:

```json
{"id":"314046001","name":"Pedra de Mana Intermediária","dbType":"recipe","mainCategory":"alchemy",
 "recipeInputItems":[{"id":"511360001","name":"Pedra de Mana Inferior","quantity":5},
   {"id":"610740001","name":"Pó de Pedra Espiritual","quantity":3,"craftableRecipe":{...}},
   {"id":"630543008","name":"Tinta Avançada (Vinculado)","quantity":2}],
 "recipeOutputItems":{"productItem":{"id":"511360003","quantity":1},"comboProbability":2000,
   "comboProductItem":{"id":"511360004","name":"Pedra de Mana Superior"}}}
```

Fatos conferidos que mudam o desenho:

- **Consistência com o `getNpc`.** Nos 121 `getItem` × 24 `getNpc` de 2026-10-06, 1.937 pares
  (item, chefe) têm a mesma chance dos dois lados; 24 pares têm `null` nos dois; nenhum par aparece só
  de um lado (script `trabalho-F7\cruzar_salvos.py`). O índice reverso é desnecessário.
- **Tamanho da lista de NPCs.** De 1 a 1.357 NPCs por item, sem corte em número redondo (mesmo script).
  O Peitoral da Fantasia tem 281, dos quais 20 são chefes de Altgard (5 a 0,025%, 8 a 0,053%, 7 a
  0,081%) e os outros 261 vão do nível 40 ao 80.
- **O NPC na lista não diz se é chefe.** `grade` e `subCategory` vêm `null` e `mainCategory` é
  `"monster"` para todos (`16`; 29.835 entradas de 2026-10-06, todas `monster`/`npc`). Para saber se é
  chefe de campo da região, cruzar o código com `InfoRegiao.chefes` (já carregado pela tela Bosses).
- **Nomes iguais, códigos diferentes.** 7 pares no Newbold (`01`); a variante 076 cai do chefe, a 129
  só do baú (`16`, `28`). A Luvas de Gartua 210530100 vem só de dois baús de Gartua, 2,5% cada (`29`).
- **Raça.** Receitas em pares por raça: `qualificationRace: "light"` na 314046001 (`19`), e
  324046001 com o mesmo nome (`17`); lascas "(Elyseano)" e "(Asmodian)" no mesmo baú (`30`). Hipótese:
  o prefixo 31x/32x da receita é a raça. A ficha mostra as duas; filtrar pela raça do jogador fica de
  fora (o Axon não sabe a raça; hipótese).

#### Receita: `getRecipe` (conferido, `19`)

```json
{"id":"314046001","mainCategory":"alchemy","subCategory":"magicstone","qualificationRace":"light",
 "masteryGrade":"beginner","masteryLevel":20,"goldCost":0,"craftingFeeType":"goldcombined",
 "craftGauge":400,"learnType":"auto","isGuildCraft":false,"recipeInputItems":[...],"recipeOutputItems":{...}}
```

Os ingredientes já vêm no `getItem`; o `getRecipe` só acrescenta maestria, custo e raça. Proposta:
pedir ao expandir a receita na ficha.

#### Onde fica o monstro: `getNpc` (conferido, `23`) e `getRegion` (conferido, `21`)

- `getNpc` da Fada Contaminada: `npcSubType:"normalmonster"`, `isNamed:false`, `level:47`,
  `npcIsFoundInRegions:[{"id":"1110","name":"Altgard","count":15,"mainCategory":"dark"}]`. O chefe
  Newbold tem `count:1` (`npcs\2400424.json`). Hipótese: `count` é o número de pontos de nascimento;
  não mostrar até conferir.
- Nenhum campo de coordenada ou subzona no `getNpc` (conferido: chaves de `23`).
- `getRegion` 1110 (55,4 KB): `regionHasNpcs` (24, os chefes de campo), `regionSubzones` (72, só
  `name` e `isVillage`), `regionHasDungeons` (118), `regionHasQuests` (155), `regionHasGatherables` (13).
- Outras ligações de lugar que o site conhece (conferido só no bundle): `npcIsBossOfDungeons`,
  `dungeonIsLocatedInRegions`, `questIsLocatedInRegions`, `gatherableIsFoundInRegions`. Cada uma custa
  um `getDungeon`, `getQuest` ou `getGatherable` a mais.

#### Lote: `?batch=1` (conferido, `24`)

`database.getItem,database.getItem?batch=1&input={"0":{...},"1":{...}}` devolve uma lista com as duas
fichas (33.537 bytes, a soma das duas avulsas: 16.632 + 16.902 mais o envelope). Reduz requisições,
não bytes. Fica para depois, se a fila da abertura pesar.

#### Tempo, tamanho, cache e limite (conferido, `trabalho-F7\manifesto.jsonl`)

- 30 requisições, 2,5 s entre elas. Mediana 430 ms nas 200; mínimo 268 ms; 10 das 30 passaram de
  1 s, uma levou 7,3 s (`13`) e outra 15,3 s (`30`). O `ureq` do Axon desiste em 20 s (`catalogo.rs:507`).
- Tamanhos (sem gzip: o script não pediu compressão; o Axon pode trafegar menos, não conferido):
  busca 5,8 a 15,4 KB por página; `getItem` 1,1 KB (Néctar) a 40,7 KB (Peitoral do Perito) hoje, 68 a
  91 KB nos equipamentos genéricos de 2026-10-06 (centenas de NPCs); baú 4,8 KB; `getNpc` 39,6 KB;
  `getRegion` 55,4 KB; `getRecipe` 2,6 KB; `searchEntities` 4,8 a 6,2 KB.
- Cabeçalhos: `Cache-Control: max-age=1200, no-store` e `cf-cache-status` MISS/EXPIRED (Cloudflare,
  20 min). Nenhum cabeçalho de limite de taxa nas 30.

### Custo de rede

**Índice reverso pelos chefes (pergunta 2, para comparar).** Para os 24 chefes de Altgard: 24 `getNpc`
(883.714 bytes somados nos arquivos de 2026-10-06; 39 a 43 KB nos 20 de nível 45, 5,3 a 5,8 KB nos 4 de
48 e 51) mais 24 `getItem` de baú (4,8 a 5,9 KB cada) = 48 requisições, por volta de 1 MB. Na fila do
catálogo (400 ms entre pedidos, `catalogo.rs:170`, mais a mediana de 430 ms) leva uns 40 s. Só conhece
chefes de campo da região: não vê monstro comum, craft, missão nem dungeon.

**Recomendado: um `getItem` por desejo.** A lista reversa vem pronta e cobre todas as fontes.

| Ação | Requisições | Bytes (conferidos) | Quando |
|---|---|---|---|
| Buscar | 1 `getItems` por página | 5,8 a 15,4 KB | ao buscar e no "mais" |
| Abrir a ficha com "onde conseguir" | 1 `getItem` (o que a ficha já faz) | 1,1 a 91 KB | ao abrir |
| Item que vem em baú | +1 `getItem` por baú distinto | ~5 KB | ao abrir, em seguida |
| Região dos monstros | +1 `getNpc` para cada um dos 5 de maior chance (P6) | ~40 KB cada | ao expandir |
| Receita (maestria) | +1 `getRecipe` | 2,6 KB | ao expandir |
| Lista de desejos, por execução | N `getItem` + baús | N × 1 a 91 KB | ao abrir a lista, a tela Bosses ou (com alerta ligado) na abertura do Axon |

Com 20 desejos: 20 a 30 requisições, 0,3 a 2 MB, uns 20 s na fila. A tela Bosses não pede nada a
mais: o `getRegion` já está carregado (`chefes.rs:90-99`) e o cruzamento é na memória.

Prioridade na fila: a ficha aberta pelo jogador vai na fila urgente, como hoje (`catalogo.rs:383`);
os desejos da abertura vão na fila normal, depois das skills, para não atrasar o nome do chefe na luta.

### Sem internet e cache

- Tudo do questlog fica na memória (`Estado.itens`, `Estado.falhas`, `catalogo.rs:174-191`) e some ao
  fechar o Axon, como a ficha de hoje (README.md:195-196, 330-331).
- Falha marca o pedido em `falhas`; abrir a lista ou a tela Bosses chama `repetir_falhas()`, como o
  painel de drops faz ao abrir (`janela.rs:410-411`).
- Sem resposta: a busca diz "O questlog não respondeu" com "Tentar de novo"; a lista mostra
  "Item 210540076" com quadrado cinza, prioridade e alerta continuam editáveis; a Bosses mostra "Os
  drops dos seus desejos não chegaram" e nenhuma ★; os alertas de desejo não disparam (a F5 mostra o
  motivo).
- A busca guarda as últimas páginas em memória por (termo, categoria, página); o questlog também
  devolve do cache da Cloudflare por 20 min.

### Modelo de dados

No `nucleo` (`catalogo.rs`), só memória:

```rust
/// De onde vem um item, pelas listas reversas do getItem.
pub struct Fontes {
    /// NPCs que derrubam, da maior chance à menor (itemIsDroppedByNpcs).
    pub npcs: Vec<FonteNpc>,
    /// Baús que contêm o item, com a chance dentro do baú (itemIsContainedInItems).
    pub baus: Vec<(u32, String, Option<f64>)>,
    /// Receitas que produzem (itemIsOutputOfRecipes): profissão e ingredientes.
    pub receitas: Vec<Receita>,
    /// Missões, dungeons (com chance), conquistas, vendedores, coleta, pedidos de suprimento: nome e código.
    pub outras: Vec<(TipoFonte, u32, String, Option<f64>)>,
}
pub struct FonteNpc { pub codigo: u32, pub nome: String, pub nivel: i32, pub chance: Option<f64>, pub quantidade: Option<(u32, u32)> }
pub struct Receita { pub codigo: u32, pub profissao: String, pub entradas: Vec<(u32, String, u32)> }
pub struct ItemBuscado { /* o ItemDrop sem chance: código, nome, ícone, raridade, categorias */ }
pub struct ResultadoBusca { pub itens: Vec<ItemBuscado>, pub pagina: u32, pub paginas: u32, pub por_raridade: BTreeMap<u8, u32> }
```

- `DetalheItem` (`catalogo.rs:126-149`) ganha `fontes: Fontes`, lido em `ler_detalhe`
  (`catalogo.rs:635-672`) pelo mesmo `ler_item` (`catalogo.rs:612-631`).
- `CatalogoSkills::buscar_itens(termo, categoria, pagina) -> Busca<ResultadoBusca>` e
  `regioes_do_npc(codigo) -> Busca<Vec<String>>`, com chave de pedido escapada (o termo pode ter `:`).
- Chance pelo baú: chance do baú no chefe × chance do item no baú (1 × 0,0357 = 3,6% no Newbold).
  Hipótese: independentes; o painel mostra "3,6% (no baú)".

No `overlay`, função pura e testável (`janela/chefes.rs` ou módulo novo):

```rust
/// Para cada chefe da região: os desejos que ele derruba e a chance, direto ou pelo baú,
/// só com chance >= mínima.
fn chefes_com_desejo(regiao: &InfoRegiao, desejos: &[(u32, &Fontes)], baus: &HashMap<u32, Fontes>,
                     minima: f64) -> HashMap<u32, Vec<(u32, f64)>>
```

### Dados em disco (`config.json`)

Proposta (resposta a P1). Tudo o que vai para o disco, e nada mais:

```json
"desejos": [
  { "codigo": 210540076, "prioridade": 1, "alertar": true },
  { "codigo": 210540129, "prioridade": 1, "alertar": true }
],
"desejos_destacar": true,
"desejos_chance_minima": 0.5,
"desejos_so_com_desejo": false
```

| Campo | Tipo | Faixa | Padrão |
|---|---|---|---|
| `desejos[].codigo` | u32 | código do questlog | (sem padrão) |
| `desejos[].prioridade` | u8 | 1 Alta, 2 Média, 3 Baixa | 2 |
| `desejos[].alertar` | bool | | true |
| `desejos_destacar` | bool | | true |
| `desejos_chance_minima` | f32, em % | 0 a 100 (0 = qualquer) | 0,5 |
| `desejos_so_com_desejo` | bool | | false |

- Na leitura literal da decisão, `desejos` vira `[210540076, 210540129]` e somem `prioridade` e
  `alertar`.
- Não vai: nome, ícone, raridade, chance, código de NPC, região, receita, termo buscado, resultado.
- Lista por classe (se P3 escolher): `"desejos_por_classe": {"Ranger": [...]}`, sem nome de personagem.
- `dentro_das_faixas` (`config.rs:94-100`): tira código repetido (fica o primeiro), traz prioridade
  para 1 a 3, chance para 0 a 100, corta a lista em 200. Config antigo sem os campos abre com lista
  vazia (o `#[serde(default)]` de `config.rs:21` já faz).

### Onde encaixa no código

| Arquivo | Mudança |
|---|---|
| `crates/nucleo/src/medicao/catalogo.rs` | `Fontes` e parse em `ler_detalhe`; `buscar_itens` e `ler_busca`; pedido `busca:` e `regiao_npc:`; desejos da abertura na fila normal |
| `crates/nucleo/src/medicao/dados_jogo.rs` | `buscar_itens`, `regioes_do_npc` (como os de `dados_jogo.rs:127-145`) |
| `crates/overlay/src/config.rs` | `Desejo` e os 4 campos; faixas; teste de config antigo (`config.rs:122-148`) |
| `crates/overlay/src/janela.rs` | `Tela::Desejos` (`janela.rs:80-85`); ícone ★ no cabeçalho (`janela.rs:509-547`); painel lateral genérico: hoje `PainelDrops` é por chefe (`janela/drops.rs:30-39`, `janela.rs:398-424`), passa a abrir também a ficha de um item avulso; `--desejos` no debug |
| `crates/overlay/src/janela/desejos.rs` (novo) | tela da lista e da busca (campo ou teclado, conforme P2), categorias, páginas, ★ |
| `crates/overlay/src/janela/item.rs` | seção "Onde conseguir" na `ficha_do_item` (`item.rs:83-188`) e ★ no topo; "‹ Drops" vira "‹ Voltar" quando a ficha veio da lista |
| `crates/overlay/src/janela/drops.rs` | ★ na `linha_item` (`drops.rs:418-459`) e nas peças do conjunto (`drops.rs:289-327`) |
| `crates/overlay/src/janela/chefes.rs` | ★ e dica em `linha_chefe` (`chefes.rs:159-217`), filtro ao lado das abas Vivos/Mortos; só para `ChefeVisto.codigo` = Some (o nome confiável, `chefes.rs:58,64-68`) |
| F5 (alertas) | consome `chefes_com_desejo` e a troca morto → vivo do 0x9101 |
| `README.md` | seção da lista, linha da busca na tabela de Privacidade (`README.md:323-328`), o que vai ao `config.json` |

Desenho em 470 px (hipótese de layout, a conferir com `--desejos`):

```
Lista de desejos (5)                                   [Voltar]
[ campo de busca / teclado ]                           [Buscar]
Todos  Armadura  Arma  Acessório  Consumível  Material
★ ▣ Luvas de Gartua           Baú de Gartua 2,5%   renasce 22:20
★ ▣ Luvas de Newbold (2)      Profanador Newbold 17,3%     vivo
★ ▣ Pedra de Mana Interm.     Craft: Alquimia
```

O clique numa linha abre a ficha no painel lateral de 330 px (`drops.rs:20`), com:

```
Onde conseguir
Cai de    Profanador Newbold   nv 45   Altgard   chefe de campo      17,3%
No baú    Baú de Saque de Newbold → Profanador Newbold               3,6%
Craft     Alquimia: 5× Pedra de Mana Inferior, 3× Pó de..., 2× Tinta...
Missão    Draupnir
          e mais 275 monstros (0,0016% a 0,081%)  ▸
```

### Riscos

1. **Foco do teclado (P2).** Sem decisão, a busca por texto não sai. Com (a), o risco maior é o jogo
   ler o teclado mesmo sem foco (Raw Input ou DirectInput de segundo plano): cada letra digitada no
   Axon viraria skill ou janela no jogo. Depois vêm o foco não voltar ao jogo e o jogo reagir à perda
   de foco. Teste manual no jogo antes de lançar; se a primeira falhar, vale (b).
2. **API não documentada.** Campos podem mudar (o questlog é base comunitária, `catalogo.rs:4-5`).
   Parse tolerante: lista ausente = sem fonte daquele tipo; a fixture real mostra a quebra no teste.
3. **Ruído de chance minúscula.** Item genérico aparece em centenas de NPCs (até 1.357); sem chance
   mínima, quase todo chefe ganharia ★ (20 de 24 no Peitoral da Fantasia).
4. **Variantes de mesmo nome (P4).** Marcar a errada faz o destaque sumir (a 129 não cai do chefe).
5. **Nome do chefe pela contagem.** Sem `codigo` (região com outra contagem, `chefes.rs:58`), não há
   destaque; outras regiões além de Altgard não foram conferidas (README.md:421-422).
6. **Duas versões do chefe.** O baú de Gartua cai de 2101074 (Gartua da Eternidade) e 2400800 (Gartua
   Imortal) (`item-bau-gartua.json`, 2026-10-06); só o código da região atual casa. Hipótese: o outro é
   o do lado da outra raça.
7. **Custo na abertura.** 20 desejos = até 2 MB e 20 s de fila a cada execução, porque nada fica em
   disco. Com resposta lenta (15 s visto uma vez), o alerta F5 pode ficar sem dado nos primeiros
   minutos.
8. **Privacidade.** O termo buscado sai na URL; precisa estar no README (P7).

### Plano de verificação

Fixture gerada do baixado, nunca digitada:

- Script: `refino\trabalho-F7\gerar_fixture.py` lê `baixado-F7` (`01`, `16`, `17`, `19`, `21`, `23`,
  `28`, `30`) e o `getItem` do Peitoral da Fantasia de 2026-10-06, corta as listas que o teste não lê
  (encantamento, alma, NPCs fora de Altgard exceto os 3 comuns de maior chance) e grava
  `desejos-teste.json` (48.643 bytes, já gerado em `refino\trabalho-F7\desejos-teste.json`).
- No repositório, ao implementar: rodar o script de novo contra respostas novas e gravar em
  `crates/overlay/src/janela/desejos-teste.json`, no padrão do `drops-teste.json` e do `item-teste.json`
  (`include_str!`, `drops.rs:516-519`, `item.rs:252-254`). O campo `_origem` diz de onde veio. Para
  regenerar com dado do mesmo dia, baixar de novo também o `getItem` 210130005 (Peitoral da Fantasia):
  o segundo argumento do script hoje é o arquivo de 2026-10-06 de outra sessão.

Testes (valores tirados da conferência que o próprio script imprime):

1. `onde_conseguir_direto`: `luvas_newbold_do_chefe` → 1 NPC, 2400424 Profanador Newbold, nível 45,
   chance 0,1734104.
2. `onde_conseguir_pelo_bau`: `luvas_newbold_do_bau` → baú 533700387 a 0,035714285; com
   `bau_newbold`, `chefes_com_desejo` dá `{2400424: [(210540129, ~0,0357)]}`.
3. `craft`: `pedra_de_mana_craft` → 4 receitas de `alchemy`; a primeira com (Pedra de Mana Inferior, 5),
   (Pó de Pedra Espiritual, 3), (Tinta Avançada (Vinculado), 2); `receita_pedra_de_mana` → maestria
   `beginner` 20, raça `light`.
4. `regiao_do_monstro`: `npc_fada` → ["Altgard"].
5. `chance_minima`: `peitoral_fantasia` com a região de Altgard → 20 chefes com mínima 0; 0 chefes com
   0,5%.
6. `busca`: `busca_newbold` → 17 itens, 1 página; agrupado por nome: 7 grupos de 2 e 3 únicos.
7. `config`: código repetido e prioridade 9 → um só, prioridade 3; config sem os campos → lista vazia,
   `desejos_destacar` true, mínima 0,5.

Red check: escrever os testes antes, com `Fontes` vazio, e ver os 1 a 6 falharem; depois, já verdes,
trocar no parse `itemIsDroppedByNpcs` por `npcDropsItems` (o 1 tem de falhar) e tirar o caminho do baú
de `chefes_com_desejo` (o 2 tem de falhar). Se algum não falhar, o teste não mede o que diz.

Manual:

- `CARGO_TARGET_DIR=target/agente-F7 cargo test --workspace` (pela ferramenta Bash; o prefixo
  `VAR=valor` é POSIX).
- `cargo run -p overlay -- --replay captura-2026-10-06-gartua.pcapng --chefes` com
  `"desejos":[{"codigo":210530100,...}]` no `config.json` (o replay lê a config e não grava,
  README.md:350): Gartua Imortal com ★ (Luvas de Gartua 2,5% pelo baú 533700375, que cai do 2400800
  a 100%, `item-bau-gartua.json` de 2026-10-06).
- `--desejos` (novo): a tela abre, a ficha da Luvas de Newbold mostra as duas variantes.
- Sem rede (cabo fora ou firewall): critério 7.
- Se P2 = (a), no jogo, com o personagem parado num lugar seguro: clicar no campo e digitar
  "gartua" e números de 1 a 9; nenhuma letra ou número pode acionar skill, abrir janela ou mexer o
  personagem. Depois, Enter: a próxima tecla volta para o jogo e os atalhos (`Ctrl+H`) voltam a valer.

### Tamanho

**G** no total. Proposta de entrega em duas partes:

- **F7a (M):** "Onde conseguir" na ficha, ★ no painel de drops e na ficha, `Tela::Desejos` só com a
  lista, destaque e filtro na Bosses, config. Não depende de P2.
- **F7b (M):** a busca por texto, depois da decisão P2 e do teste de foco no jogo.

### Dependências

- **F4 (configurações com seções):** os 3 campos globais entram numa seção "Lista de desejos"; a
  prioridade e o alerta por item ficam na própria tela da lista.
- **F5 (alertas):** a F7 entrega `chefes_com_desejo` e `alertar` por item; a F5 decide o gatilho
  (renasceu, X min antes, vivo ao entrar na região), o aviso e onde ele aparece. Os gatilhos usam só o
  que o 0x9101 já dá e o mapa do jogo mostra (vivo/morto e hora), na região em que o jogador está; fora
  dela a lista é "provável" (`chefes.rs:17-19,57-74`).
- Nenhuma dependência de F1, F2, F3 ou F6.

---

## 3. URLs consultadas

Todas GET, em 2026-10-08, com espera de 2,5 s entre elas; resposta crua em `refino\baixado-F7\` com o
mesmo número; status, tempo e bytes em `refino\trabalho-F7\manifesto.jsonl`. Prefixo comum:
`https://questlog.gg/aion-2/api/trpc/` (o `input` aqui vai sem o escape de URL, para ler).

| # | URL | Status | ms | bytes |
|---|---|---|---|---|
| 01 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"Newbold"}` | 200 | 458 | 5.770 |
| 02 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"peitoral"}` | 200 | 323 | 13.123 |
| 03 | `database.getItems?input={"language":"pt","page":2,"searchTerm":"peitoral"}` | 200 | 289 | 13.181 |
| 04 | `database.getItems?input={"language":"pt","searchTerm":"Newbold"}` | 400 | 254 | 162 |
| 05 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"Newbold","grade":"x","mainCategory":1,"subCategory":1}` | 400 | 1.288 | 207 |
| 06 | `database.searchEntities?input={"searchTerm":"Newbold","language":"pt","extendSearch":false}` | 200 | 268 | 4.797 |
| 07 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"bau de saque de newbold"}` | 200 | 492 | 15.407 |
| 08 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"newb"}` | 200 | 276 | 5.770 |
| 09 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"n"}` | 200 | 621 | 13.785 |
| 10 | `database.getItems?input={"language":"pt","page":1,"searchTerm":""}` | 200 | 310 | 13.255 |
| 11 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"210540076"}` | 200 | 1.299 | 88 |
| 12 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"Newbold","mainCategory":"usable"}` | 200 | 289 | 869 |
| 13 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"Newbold","grade":71}` | 200 | 7.296 | 5.770 |
| 14 | `database.getItems?input={"language":"pt","page":1,"searchTerm":"Newbold","grade":"71"}` | 200 | 1.304 | 5.770 |
| 15 | `database.getItems?input={...,"grades":{},"minLevel":{},"maxLevel":{},"level":{},"itemLevel":{},"sort":{},"sortBy":{},"order":{},"hitsPerPage":{},"limit":{},"pageSize":{},"raceId":{},"itemTier":{},"characterClass":{},"exchangeType":{}}` | 400 | 376 | 159 |
| 16 | `database.getItem?input={"id":"210540076","language":"pt"}` | 200 | 2.478 | 16.632 |
| 17 | `database.getItem?input={"id":"511360003","language":"pt"}` | 200 | 297 | 16.902 |
| 18 | `database.getItem?input={"id":"210130013","language":"pt"}` | 200 | 333 | 40.722 |
| 19 | `database.getRecipe?input={"id":"314046001","language":"pt"}` | 200 | 3.342 | 2.556 |
| 20 | `database.getItem?input={"id":"530116008","language":"pt"}` | 200 | 2.721 | 1.079 |
| 21 | `database.getRegion?input={"id":"1110","language":"pt"}` | 200 | 517 | 55.355 |
| 22 | `database.searchEntities?input={"searchTerm":"Newbold","language":"pt","extendSearch":true}` | 200 | 1.296 | 6.201 |
| 23 | `database.getNpc?input={"id":"2400732","language":"pt"}` | 200 | 369 | 39.607 |
| 24 | `database.getItem,database.getItem?batch=1&input={"0":{"id":"210540076","language":"pt"},"1":{"id":"511360003","language":"pt"}}` | 200 | 380 | 33.537 |
| 25 | `database.getItems?input={...,"hitsPerPage":{},"limit":{},"pageSize":{},"raceId":{},"itemTier":{},"characterClass":{},"exchangeType":{}}` | 200 | 467 | 5.770 |
| 26 | `database.getItems?input={...,"grades":{},"minLevel":{},"maxLevel":{},"level":{},"itemLevel":{}}` | 200 | 292 | 5.770 |
| 27 | `database.getItems?input={...,"sortBy":{}}` | 400 | 2.390 | 159 |
| 28 | `database.getItem?input={"id":"210540129","language":"pt"}` | 200 | 401 | 16.690 |
| 29 | `database.getItem?input={"id":"210530100","language":"pt"}` | 200 | 302 | 25.427 |
| 30 | `database.getItem?input={"id":"533700387","language":"pt"}` | 200 | 15.345 | 4.843 |

Nos 15, 25, 26 e 27, `...` é `"language":"pt","page":1,"searchTerm":"Newbold"`.

Sem requisição nova (dados de 2026-10-06, sessão anterior): `scratchpad\itens\*.json` (121 `getItem`),
`scratchpad\npcs\*.json` (24 `getNpc` de Altgard), `scratchpad\item-bau-gartua.json`,
`scratchpad\item-210530100.json`, `scratchpad\item-guarda.json`. Bundles: `scratchpad\questlog-js\`
(`ByJQ2KcU.js` para o `searchEntities`, `BjBI1uw7.js` para as relações de item).
