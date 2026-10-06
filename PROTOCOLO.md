# AION 2 (Global/Steam): protocolo para o medidor de DPS

Ponto de partida: código do RATmeter (Kuroukihime/AIon2-Dps-Meter, último commit em
2026-10-01 07:33 UTC). Validado em 2026-10-01 contra duas capturas nossas feitas
**depois** do patch das 19:10 UTC: `captura.pcapng` (pktmon, 60 s) e uma sessão ao vivo
por raw socket (45 s). O que está marcado como **hipótese** ainda não foi conferido.

## 1. Ambiente observado

| Item | Valor |
|---|---|
| Instalação | `C:\Program Files (x86)\Steam\steamapps\common\AION2` |
| Motor | Unreal Engine, build Shipping (sem console, sem API de addon, sem log de combate) |
| Anti-cheat | NCGuard (`Aion2\Binaries\Win64\NCGuard\bb64.dll`, atualizado a cada launch) |
| Processos | `AION2.exe` launcher cria o processo real, que não expõe caminho nem linha de comando a um processo comum |
| Servidor de jogo | TCP `193.202.112.171:13328` (as demais conexões são HTTPS 443 e loopback) |
| Modo de tela | `FullscreenMode=1` (tela cheia em janela): overlay sempre no topo aparece por cima |
| Janela | título `AION2` (não traz nome do personagem) |
| Logs locais | `%LOCALAPPDATA%\AION2\Saved_Steam\Logs` só tem `cef3.log` (navegador embutido) |
| Runtimes | Rust 1.99 (MSVC, compila o medidor desde a 0.2.0), .NET 10 (SDK 10.0.302, a 0.1.0 era em C#), Node 24, JDK 17. O `python` do PATH é o atalho da Microsoft Store; há CPython 3.11/3.13 gerenciado pelo `uv` (usado pelos servidores MCP) |

Addon dentro do jogo exigiria injetar código no processo protegido pelo NCGuard: fora
de questão. O medidor é um programa externo que lê o tráfego de rede passivamente e
nunca abre handle no processo do jogo.

## 2. Transporte (confirmado)

- Tráfego **sem criptografia**: framing + compressão LZ4.
- Só a direção servidor → cliente interessa. A direção cliente → servidor não sincroniza
  no heartbeat (formato diferente, não investigado).
- O servidor manda o heartbeat `0E 00 36` (pacote de 11 bytes, opcode `0x3600`) cerca de
  20 vezes por segundo. Ele serve para achar o fluxo certo e para sincronizar o corte.

```
[varint tamanho][u16 opcode LE][corpo...]
tamanho_total_do_pacote = valor_varint + bytes_do_varint - 4
```

Pacote comprimido (opcode `0xFFFF`):

```
[varint tamanho][flag opcional 0xF? != 0xFF][FF FF][u32 LE tamanho_descomprimido][bloco LZ4 cru]
```

O conteúdo descomprimido é uma sequência de pacotes no mesmo framing (bytes `00` entre
eles são preenchimento). Na captura, o bloco descomprimiu com o tamanho exato declarado.

## 3. Opcodes

| Opcode | Conteúdo | Situação |
|---|---|---|
| `0x3804` | Dano direto | confirmado (seção 4) |
| `0x3805` | Dano periódico (DoT) | alvo e autor conferidos no world boss de 2026-10-03 (seção 4); valor não conferido na tela |
| `0x3801`, `0x3802`, `0x3803`, `0x3806` | Ciclo de uso de skill (autor, skill, posições em float). Sem dano | confirmado |
| `0x8D00` | HP atual de entidade | confirmado para mobs e para o jogador (seção 5) |
| `0x8D04` | Morte de entidade: morto, skill que matou, matador e nome dele | confirmado (seção 5) |
| `0x3641` | Spawn de entidade: código do NPC (nome e retrato pelo questlog); tipo `0x5F` = invocação, pet ou armadilha; dono também no marcador `07 02`; HP atual e máximo | confirmado (seções 7 e 7c) |
| `0x3633` | Seu personagem (id, nome, level, power), só no login | confirmado (seção 7b) |
| `0x561C` | Power mudou: `[varint entidade][u32 power]...` (o `0x561D` traz o mesmo valor duas vezes, sem entidade) | confirmado uma vez com o seu personagem (seção 7b) |
| `0x3645` | Outro jogador entrando no campo de visão (id, nome, level, power, equipamento; 1.300 a 1.500 bytes) | confirmado (seção 7b) |
| `0x8D21` | Estado de combate de entidade (mob ou jogador) | confirmado (seção 5b) |
| `0x382A`, `0x382B`, `0x382C` | Buff novo, renovado e removido | confirmado (seção 5c) |
| `0x382D` | Ligado a buffs (1.909 de 1.962 no world boss) | não fechou (seção 5c) |
| `0x610B` | Tickets de conteúdo no login, entre eles a Energia Odyle | confirmado (seção 7d) |
| `0x610C` | Um ticket de conteúdo mudou (ex.: essência OD usada) | confirmado com a Odyle (seção 7d) |
| `0x9101` | Chefes de campo da região: vivo ou morto e uma hora (de renascer ou de quando nasceu), a cada poucos segundos | confirmado (seção 7e) |
| `0x3603` | Hora, a cada 10 s: `[u16 0][u64 relógio][u64 hora do servidor em ms Unix]` | lido nas capturas, não usado |
| `0x3620`, `0x3649`, `0x3847`, `0x9702` | vínculo de sessão, atributos, cooldown, grupo | não usados |

Mais frequentes na captura e sem uso: `0x371D` (2494 em 60 s), `0x371C`, `0x371B`, `0x371A` (provavelmente movimento).

## 4. Dano direto `0x3804`

```
varint  alvo_id
varint  seletor           (nibble baixo 4..7; 0 = golpe sem dano, provável erro/esquiva)
varint  desconhecido
varint  autor_id          (autor == alvo acontece: cura em si mesmo)
u32     skill_code        (8 dígitos; 2 primeiros = classe)
u8      contador          (sobe a cada uso)
varint  tipo_dano         (3 = crítico, segundo o RATmeter: ver pendência no README)
bloco especial:
  seletor 4:  8 bytes
  senão:      u8 flags (0x02 aparo; os outros bits abaixo)
              varint valor   (0 em todo golpe, menos com a flag 0x20: ~20% do dano)
              u8 direção (0x01 costas, 0x02 frente)
              8 bytes (u32 que parece id da instância do cast + u8 índice do acerto + 3 bytes)
varint  desconhecido      (90 4E = 10.000 em todo golpe de mob)
varint  dano              (o número da tela: ver seção 6)
cauda   [lista, só se seletor & 0x20][u8 índice do acerto][u8 00]
```

O mesmo opcode leva os três sentidos, e o medidor separa pelo autor e pelo alvo:
jogador → mob = dano causado (aba DPS); mob → jogador = dano recebido (aba Tank);
jogador → jogador, inclusive em si mesmo, = cura (aba Healer) quando a skill é de cura.
Jogador → jogador sem ser cura (PvP, buff com valor) e mob → mob ficam de fora.

Bloco especial (medido em 2026-10-05 nas 6 capturas, 146.981 golpes; só as variantes 4 e 6
apareceram):

- O 2º campo é varint. Até a 0.7.1 era lido como u8 e, com a flag 0x20 (só golpe de mob em
  jogador, 36 de 36), o 10.000 fixo virava o dano: 27 golpes do world boss de 2026-10-03
  saíram com 10.000 em vez de 2.182, 4.550, 1.896.
- Direção 0x01 = costas: 66,5% dos golpes de jogador no world boss, cercado por centenas de
  jogadores; golpe de mob em jogador vem de frente em 95,6%. É o "pelas costas" do overlay.
  O medidor do TK (cliente coreano) lê costas no bit 0x01 das flags, que não aparece em
  nenhum golpe de jogador em mob (0 de 55.104).
- Flags 0x02 = aparo ou bloqueio: aparece nos dois sentidos e corta o dano (0,43 e 0,58 do
  dano mediano da mesma skill e tipo).
- Flags 0x04 (12,7% dos golpes de jogador, dano 1,09 vez o sem o bit), 0x08 (18 golpes) e
  0x80 (30%, 1,02 vez): significado não fechou. "Perfeito" e "dano duplo" ficam fora do
  overlay até serem conferidos na tela. 0x40: dano 1 em todos (golpe anulado, hipótese).

Cauda e parcelas (medido em 2026-10-05):

- A lista existe se e só se `seletor & 0x20` (0x24, 0x26, 0x34, 0x36): `[u8 n 1..=4][n
  varints iguais]`. Na variante 4 com o bit 0x10 há 1 byte antes da lista ou do índice.
  Depois vem sempre `[u8 índice][00]`, às vezes com 1 ou 2 bytes a mais (não fechou). A
  regra leu certo 19.052 golpes, com 0 erro. Golpe de mob em jogador nunca traz lista (0 de
  1.190); o seletor 0x46 traz `[01][u32]` antes do índice.
- Cada parcela vale ~10% do dano principal (mediana 0,100, n=16.155; ex.: dano 1.101 com
  `03 6E 6E 6E`, 3 × 110). É o "multi-hit" do TK, que lá vira dano × n.
- A lista não derruba o HP do mob: no mesmo mob, golpe com e sem lista dão a mesma razão
  queda/dano (18,830 e 18,824; 12,000 e 12,000), e (queda - F × dano) / (F × soma da lista)
  = 0,000 na mediana (n=136), no intervalo do golpe e no seguinte. Por isso o medidor não
  soma a lista: somar poria o total ~5,8% acima do HP que os mobs perderam. Pendente: conferir
  na tela um golpe isolado, se o número grande é o dano do pacote ou o dano menos as parcelas.

DoT `0x3805`: `varint alvo, u8 efeito (bit 0x02 = dano), varint autor,
varint desconhecido, u32 skill*100, varint dano`. A skill do DoT não diz quem é o autor:
no world boss de 2026-10-03, o Círculo de Proteção do Chanter (18730002) chegou 265 vezes
com autor = boss (#21799) e alvo em jogadores de várias classes (34 alvos, 20 Chanters).
Por isso o medidor só reconhece jogador pela skill no golpe direto (`0x3804`).

## 5. HP `0x8D00` e morte `0x8D04`

```
varint  entidade
varint, varint, varint    (mobs: 02 01 00)
u64     HP atual
```

Para o jogador o formato é outro e a leitura acima dá lixo:

```
varint  entidade
u8      tipo
u8      n
n ×     [u8 chave][u32 valor]
```

Chave 0 = HP (cerca de 3.300 no personagem testado). Sem identificação: chave 1 (oscila
entre 1.500 e 1.800), chave 3 (cai 500 a cada 0,1 s) e chave 6 (sobe cerca de 4.000/s).

Morte `0x8D04`:

```
varint  morto
u32     skill que matou
varint  matador
u16     servidor
u8      tamanho + UTF-8   nome do matador
u8      tamanho + UTF-8   legião
```

Sem matador (armadilha que expirou) vem tudo zerado. Mob que mata também pode trazer nome,
então o medidor só aproveita o nome quando a skill é de classe e o abate traz servidor, ou
quando o matador já é jogador conhecido. O servidor veio 1000..9999 nos 73 abates de jogador
das capturas (2401) e 0 no abate do world boss. A tabela de ameaça (aggro) fica no servidor e não aparece em nenhum pacote; o
pacote de troca de alvo do mob também não foi achado.

## 5b. Estado de combate `0x8D21` (confirmado)

```
varint  entidade
varint  0          (nos 901 pacotes vistos)
varint  estado     (1 = entrou em combate, 0 = saiu)
```

Pista do medidor do TK, conferida em 2026-10-05 nas 6 capturas do cliente global (901 de 901
sem byte sobrando). Vem para mobs e para jogadores.

- Mob: o 0 chega no instante da morte em 73 de 84 casos (no máximo 0,06 s depois do último
  golpe); os outros vêm em pares 1 → 0 sem golpe nenhum (aggro que resetou, hipótese). O 1
  chega 0,05 s depois do primeiro golpe quando o jogador puxa, ou antes do dano (até 22 s)
  quando o mob puxa.
- World boss de 2026-10-03: 0 no instante da morte e 1 de novo 0,04 s depois, sem golpe
  novo. O medidor ignora o 1 que chega até 5 s depois da morte.
- Mob que sai da visão em combate não manda o 0 (19 no world boss). O medidor só espera os
  mobs com golpe ou entrada em combate nos últimos 5 s.
- O seu estado cai 1,5 a 3 s depois do último golpe e quebrou em 4 pedaços a luta contínua
  do world boss. O estado de jogador não entra no fim da luta.

Uso (desde a 0.8.0): a luta acaba quando todos os mobs dela saíram de combate (0 ou morte).
O que chega até 1 s depois ainda conta nela, cura e morte de jogador depois disso não abrem
luta nova, e os 15 s sem dano continuam valendo.

## 5c. Buffs `0x382A`, `0x382B` e `0x382C` (confirmado)

```
0x382A (novo):      [varint alvo][u8 01][u8 tipo 0x13|0x11][varint instância][u32 código]
                    [u32 duração ms][u32 0][u64 hora do servidor ms][varint autor]...
0x382B (renovado):  igual, sem o u8 01
0x382C (removido):  [varint alvo][u8 n] e n × [u8 tipo 0|7][varint instância][u8 motivo],
                    com mais 10 bytes quando o tipo é 7
```

Pista do TK, conferida na captura do world boss (12.727 `0x382A`, 40.937 `0x382B`; os 7.317
`0x382C` fecharam no byte exato). O layout do TK (pular 2 bytes depois do alvo) só acerta o
`0x382B` quando a instância ocupa 2 bytes.

- Chave: (alvo, instância). O `0x382A` abre; o `0x382B` renova cerca de 1 vez por segundo
  enquanto o buff dura, com o mesmo código em 99,5% (216 de 40.937 trocaram); o `0x382C`
  fecha. Sem remoção, o buff vale até a última renovação + duração.
- Duração 0xFFFFFFFF (63 casos) = permanente, com cauda de outro formato; duração 0 (501) =
  instantâneo.
- Código = skill × 10 + dígito do efeito na maioria (174100011 vem da skill 17410001):
  código/10 bateu com a skill usada pelo autor nos 3 s anteriores em 8.586 de 11.657
  aplicações (74%). O pacote não traz nome de buff. Códigos fora da faixa de classe (200,
  231, 10002) parecem efeitos do sistema (hipótese).
- `0x382D` (1.962, 1.909 deles no world boss): `[varint alvo][u8 01][u8 0A][varint]`. Não fechou.

Uso (desde a 0.8.0): "Buffs recebidos" do jogador expandido, com a parte da luta em que cada
buff de classe esteve ativo nele.

## 6. Escala do dano (medido)

O campo `dano` é o número que o jogo mostra ao bater: um golpe de 2.574 na tela é 2.574 no
campo (conferido em 2026-10-02, quando o medidor ainda multiplicava por 18,82 e mostrou
48,4K). O medidor exibe o campo como vem, nas três abas.

O que sai da barra de HP do mob é outra conta. Em 150 intervalos entre dois
`0x8D00` do mesmo mob com um único golpe no meio, **queda de HP / campo = 18,82
(mediana)** nos 8 mobs, com variação de 18,75 a 18,93 entre skills. Os valores abaixo
disso são golpes finais (a queda fica limitada pelo HP restante).

Esse fator muda de mob para mob (medido em 2026-10-05): 18,82 nos 8 mobs daquela captura,
12,000 nos 22 da captura de combate, 12,904 nos 4 do teleporte, 10,63 e 4,50 no login, 4,50 e
9,49 no world boss. Dentro do mesmo mob ele é estável: golpe isolado sem lista dá queda / (F ×
dano) - 1 = 0,000 na mediana (n=424).

- Dano em **jogador** não tem esse fator: um golpe de mob com campo 166 tirou exatamente
  166 do HP do jogador (chave 0 do `0x8D00`), conferido nesse golpe só.
- Até a 0.5.1 o medidor multiplicava o campo por `18,82` para mostrar HP do mob. Saiu
  porque não bate com o número da tela; como era igual para todos, ranking e
  porcentagens não mudam.
- A queda de HP **não** é usada para atribuir dano: num alvo com vários jogadores ela
  mistura o dano de todos. A atribuição vem do autor e da skill de cada `0x3804`.
- Medido só com Ranger. Conferir com outras classes numa captura em grupo
  (`--procurar-quedas` e o bloco "Razão queda/dano" do replay).
- Nenhum pacote do intervalo contém o valor exato da queda (procurado como varint, u16,
  u32 e float). Ou o fator é aplicado no cliente, ou o valor exato vem por outro caminho.

## 7. Invocações, pets e armadilhas (confirmado)

Dão dano com id próprio. O spawn `0x3641` com byte baixo da máscara = `0x5F` traz:

```
varint  entidade
u16     máscara (byte baixo 0x5F = invocação)
u8      flags (bit 0 = tem nome)
varint  tamanho + UTF-8   nome do DONO (só com o bit 0)
u32     código do NPC     (7 dígitos, em qualquer tipo de spawn)
...     (posição em float etc.)
bloco:  [u32 dono][u32 legião][u16 0][u16 servidor][u8 tamanho][nome da legião]
```

Código do NPC (medido em 2026-10-05): veio com 7 dígitos em 1.229 de 1.229 spawns das 6
capturas (mob, cidadão e invocação, com e sem nome), seguido de `00 02` ou `40 02`. É o id
do NPC no questlog (`database.getNpc`, com nome em português, level, `isNamed`,
`npcSubType` e às vezes o retrato): 2400425 = "Arconte da Alma Perdida Axios" (o world boss,
Nv 45, `heromonster`, retrato `UT_256_MOB_DstrArchonE_01`), 2920620 = "Armadilha de
Explosão" (a armadilha do Ranger), 2920110 = "Espírito do Fogo" (o do Elementalist),
2701341 = "Seguidor de Zikel". O medidor do TK acha o mesmo código procurando `00 40 02`
ou `00 00 02` no pacote; aqui ele é lido na posição.

O bloco do dono é achado por varredura com todas as validações juntas (formato descrito
pelo A2Tools em `docs/summon-attribution.md`). Na captura: Explosion Trap `#57692`,
dono `A6 2B 00 00` = `#11174` (Yoshi), legião "Tubazai". Sem legião o bloco vem zerado e
sobra o nome do dono. Ids são reemitidos ao trocar de zona.

Marcador do dono, em qualquer tipo de spawn: depois de `FF×8`, o primeiro `07 02 xx` com
`xx` = `06` (armadilha, tipo `0x5F`) ou `01` (espírito do Elementalist, tipo `0x1F`) é
seguido do u32 do dono. No world boss de 2026-10-03, 428 de 553 invocações com dano tinham
o dono assim num jogador com nome, e a classe da invocação bateu com a do dono em 444 de
445. Outros tipos trazem lixo no lugar (ex.: `07 02 01` com dono 1.918.044.167), então o
medidor só aceita o vínculo quando o dono já é jogador conhecido. O spawn chegou depois
dos primeiros golpes em 88 de 92 invocações (mediana 0,6 s): o que a invocação somou antes
passa para a linha do dono.

## 7b. Jogadores: `0x3645` (outros) e `0x3633` (você)

Os dois começam igual:

```
varint  entidade
4 bytes desconhecido
u8      flags (bit 0 = tem nome)
varint  tamanho + UTF-8   nome
```

O u32 logo depois do nome do `0x3645` **não** é o level: deu 32 no Dacura (45 no jogo) e
12 na Vallaina (32 no jogo). O level fica cerca de 1.000 bytes adiante, depois do
equipamento, num bloco achado por varredura:

```
u16     servidor          (1000 a 9999; 2401 em todos os pacotes vistos)
u8      n                 (1 a 8)
n ×     [u16 tag][u32 valor]   (1ª tag 0x00CD ou 0x00CE)
u32     level
u32     0
u32     power
```

Achou um, e só um, bloco em cada um dos 81 pacotes de 2026-10-02. Conferido no jogo:
Dacura level 45; Nxhunter level 43 e power 909; LaReini level 31 e power 579.

No `0x3633`, que chega no login e não veio no teleporte, o level vem logo depois do nome:
`[u16 servidor][u32 desconhecido][u8 desconhecido][u32 level][u32 power]` (31 e 355,
conferido uma vez). No mesmo login o power subiu para 361 pelo `0x561C`, que é o valor que
o jogo mostrava. O
bloco do servidor também existe no `0x3633`, mas ali o u32 seguinte não é o level.

Nenhuma captura pegou um level up, então o pacote que atualiza o level no meio da sessão
não é conhecido. O id do personagem muda a cada sessão (`#11174`, `#7301` e `#11179` para
o mesmo personagem). O spawn de invocação (`0x3641`) termina com o mesmo bloco sem o
servidor, e o level ali acompanhou o do dono (24, 30 e 31), mas o medidor não usa.

## 7c. HP no spawn `0x3641` (confirmado)

Depois do u32 do código do NPC:

```
u8      flags     (vistos 0x00, 0x40, 0x08 e 0x48)
u8      2
4 × f32           (posição etc.)
12 bytes          (só com flags & 0x08)
3 bytes           (tamanho fixo; lidos como varint, desalinham o resto)
varint  HP atual
varint  HP máximo
u32, u32
```

Medido em 2026-10-05: 889 spawns com código nas 5 capturas, todos lidos. Nas 416 entidades
que também tiveram `0x8D00`, todo HP lido nele ficou dentro do máximo do spawn: mesma escala,
sem exceção. O world boss (Arconte da Alma Perdida Axios, Nv 45)
vem com máximo 160.000.000. HP atual 0 é mob que já nasceu morto. O medidor recusa flags fora
de `0x48` e HP atual acima do máximo. O % de HP do overlay é o último HP do `0x8D00` sobre
este máximo; sem o spawn (Axon aberto com o mob já na tela), não há máximo nem %.

"Derrota em": o HP do `0x8D00` dos últimos 30 s, por mob; com pelo menos 5 s entre a leitura
mais velha e a mais nova e o HP caindo, a queda por segundo nesse trecho dá o tempo até zerar.
É HP sobre HP: a relação entre HP e dano muda de mob para mob (seção 6).

## 7d. Tickets de conteúdo `0x610B` e `0x610C` (Energia Odyle)

`0x610B` chega no login com a lista inteira:

```
varint  n
n ×     u8      tipo
        u32     id
        8 bytes (só com tipo & 0x01; sem uso)
        varint  valor (só com tipo & 0x04)
        varint  extra (só com tipo & 0x08)
```

Nas duas listas capturadas (logins de 2026-10-02 e 2026-10-05, 73 entradas cada) a leitura
fecha no último byte. Tipos vistos: `0x00`, `0x01`, `0x04` e `0x0C`; outro bit deixa o tamanho
da entrada desconhecido, e o medidor descarta a lista. Os ids vêm em faixas (1 a 15 sem o 2 e
o 5, 101 a 103, 201 a 206, 10.000.001 a 10.000.012, 60.000.001 a 60.015.101, 70.000.001, 80.000.001,
90.000.001 a 90.000.006); os de 60.000.002 em diante têm valores pequenos (1 a 35), com cara de
entradas de conteúdo; vários passaram de 2 para 10 entre os dois logins.

Energia Odyle = ticket 60.000.001. No login de 2026-10-05 a tela mostrava 550(+270)/840 e o
ticket veio com tipo `0x0C`, valor 550 e extra 270. No de 2026-10-02 veio com tipo `0x04` e
valor 155, sem extra. O máximo (840) não vem no pacote. Em 2026-10-05, um teleporte no mesmo
servidor não mandou nem `0x610B` nem `0x610C`.

`0x610C` = `[u8 ?][uma entrada como acima][resto]`. Visto duas vezes:

- 2026-10-03: `00`, ticket 10 com valor 14 (o mesmo do login), resto `02`;
- 2026-10-05, no uso de uma essência OD (tela: carregada de 300 para 310): `01`, tipo `0x0C`,
  ticket 60.000.001, valor 550 e extra 310, resto `01 0A000000`.

O primeiro byte não é contagem (com `00` ainda vem uma entrada), e o medidor ignora o valor dele
e o resto. O gasto de Odyle em dungeon ainda não foi capturado.

O `0x3656` (`[u64][u64]`, chega no login junto com o `0x610B`) não é a Odyle: o primeiro número
sobe até igualar o segundo (15.578 → 19.501; 33.762 → 33.862) e não bate com a tela.

## 7e. Chefes de campo `0x9101` (confirmado)

O servidor manda a lista dos chefes de campo da região a cada poucos segundos (de 1,0 a 6,3 s
entre um e outro nas capturas), com o mapa aberto ou não:

```
u16     0
u32     região (1110 = Altgard)
u8      n
n ×     u8      vivo (0 ou 1)
        varint  id = região × 100 + número do chefe (111001 a 111024)
        3 × f32 posição (só vivo)
        u8      máscara (só na 1ª entrada de cada grupo de 8: o vivo das 8, um bit cada, do bit 0)
        u64     hora em ms Unix
3 bytes 00 00 00, sem uso
```

A hora é a de renascer (morto) ou a hora marcada em que nasceu (vivo). Vivo com hora 0
provavelmente não morreu desde que o servidor reiniciou (hipótese: 111001 a 111004 tinham hora
em 2026-10-03, vieram com 0 em 2026-10-05 e com hora de novo em 2026-10-06).

- 254 pacotes em 5 capturas (2026-10-03 a 2026-10-06), todos de Altgard com 24 chefes: a
  leitura fecha nos 3 bytes de sobra em todos, e a máscara confere as 6.096 entradas. O
  medidor recusa o pacote em que a máscara não bate.
- Gartua Imortal = 111021. Na captura de 2026-10-06 veio morto com hora 1.791.336.040.627
  (22:20:40 de Brasília); a tela do jogo às 17:22:36 mostrava 4h 58min 5s (22:20:41).
- Em 2026-10-05, 111009, 111014 e 111018 estavam mortos às 14:25 (renascer às 14:27:45,
  14:35:01 e 14:46:39) e vivos às 14:54 com as mesmas horas. O 111009 passou a vivo às
  14:25:29, 136 s antes da hora.
- Nomes: o pacote não traz. O NN-ésimo chefe é o NN-ésimo NPC do `getRegion` do questlog
  (`regionHasNpcs` em ordem de código); Altgard tem lá 24 NPCs nomeados, os 24 chefes.
  Conferido em 3: Gartua Imortal (21º, pelo timer) e Profanador Newbold e Arconte da Alma
  Perdida Axios (12º e 13º: a posição do `0x9101` é igual, em float, à do `0x3641` deles). Sem
  a mesma contagem, o medidor mostra "Chefe 21".

O medidor mostra vivo ou morto e a hora; a posição não aparece. Só Altgard foi capturada.

## 8. Captura ao vivo (confirmado)

Raw socket do Windows (`SIO_RCVALL`), um por IPv4 local, processo elevado. Não precisa
de Npcap. No teste: servidor detectado em menos de 5 s, 0 lacunas, 0 dessincronizações,
cerca de 5.400 pacotes do jogo em 45 s.

Troca de servidor (visto em 2026-10-02: `193.202.112.171` → `.195`, porta 13328 nas duas):
o jogo abre outra conexão e todos os ids de entidade mudam. A `Sessao` guarda os segmentos
de cada fluxo que ainda não é o do jogo (até 4 MB, fora das portas 443 e 80) e, quando um
fluxo aberto com SYN depois do atual manda 3 heartbeats, troca na hora e reprocessa o
começo dele. Depois do SYN o enquadrador já começa sincronizado, porque o stream abre na
fronteira de um pacote. Sem SYN visto (overlay aberto com a conexão já em andamento), a
troca só acontece depois de 5 s de silêncio do fluxo atual. Na troca, o `Medidor` zera a
luta e tudo o que é indexado por id.

## 8b. Ping (medido)

Nenhum pacote do jogo traz o ping. O medidor usa o TCP do fluxo travado: da hora em que o PC
manda um segmento com dados até o primeiro segmento do servidor cujo ACK cobre o fim dele
(seq + tamanho). O servidor às vezes segura o ACK (ACK retardado), então o número mostrado é
o menor tempo dos últimos 10 s. Nas capturas de 2026-10-02 e 03: mínimo de 10 a 14 ms em
toda janela de 10 s, mediana ~20 ms.

- A captura por raw socket entrega o mesmo segmento de saída duas vezes, 0,035 ms depois
  (3.519 de 3.604 repetições nas capturas). Só repetição mais de 1 ms depois conta como
  retransmissão, e retransmissão não vira amostra (regra de Karn).
- Segmento repetido que já foi confirmado é ignorado.
- Até 256 segmentos esperando ACK; passou disso, os mais velhos saem.

Não foi comparado ao vivo com o número que o jogo mostra.

## 9. Riscos e licenças

- ToS: as buscas não acharam posição oficial da NCSoft sobre medidores. As sanções
  públicas (jan/2026) miram macro e farm de ouro. Programa de terceiro continua sendo
  risco da conta.
- Opcodes e layout mudam com patch: depois de cada atualização, rodar `capturar.ps1` e o
  replay (seção "Depois de um patch" no README).
- `dados/skills.json` veio do RATmeter (GPL-3.0). Uso pessoal sem problema; distribuir
  o medidor com esse arquivo obriga a distribuir sob GPL-3.0. O código deste repositório
  foi escrito a partir do formato descrito aqui, sem copiar código do RATmeter.
- Do Aion2-Dps-Meter do TK (MIT, cliente coreano) vieram as pistas do `0x8D21` e dos buffs
  `0x382A/B/C`. Nada dele foi usado sem conferir em captura do cliente global: o layout de
  dano dele não vale aqui (seção 4). Nenhum código foi copiado.
