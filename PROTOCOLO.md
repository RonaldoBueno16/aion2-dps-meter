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
| `0x3805` | Dano periódico (DoT) | hipótese: não apareceu nas capturas (Ranger sem DoT) |
| `0x3801`, `0x3802`, `0x3803`, `0x3806` | Ciclo de uso de skill (autor, skill, posições em float). Sem dano | confirmado |
| `0x8D00` | HP atual de entidade | confirmado para mobs e para o jogador (seção 5) |
| `0x8D04` | Morte de entidade: morto, skill que matou, matador e nome dele | confirmado (seção 5) |
| `0x3641` | Spawn de entidade; tipo `0x5F` = invocação, pet ou armadilha | confirmado (seção 7) |
| `0x3633` | Seu personagem (id, nome, level, power), só no login | confirmado (seção 7b) |
| `0x561C` | Power mudou: `[varint entidade][u32 power]...` (o `0x561D` traz o mesmo valor duas vezes, sem entidade) | confirmado uma vez com o seu personagem (seção 7b) |
| `0x3645` | Outro jogador entrando no campo de visão (id, nome, level, power, equipamento; 1.300 a 1.500 bytes) | confirmado (seção 7b) |
| `0x3620`, `0x3649`, `0x382A`, `0x382B`, `0x3847`, `0x9702`, `0x3603` | vínculo de sessão, atributos, buffs, cooldown, grupo, hora | não usados |

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
  senão:      u8 flags (0x02 aparo, 0x04 perfeito, 0x08 dano duplo)
              u8 desconhecido
              u8 direção (0x01 costas, 0x02 frente)
              8 bytes (u32 que parece id da instância do cast + u8 índice do acerto + 3 bytes)
varint  desconhecido
varint  dano              (em escala reduzida: ver seção 6)
...     índice do acerto e terminador
```

O mesmo opcode leva os três sentidos, e o medidor separa pelo autor e pelo alvo:
jogador → mob = dano causado (aba DPS); mob → jogador = dano recebido (aba Tank);
jogador → jogador, inclusive em si mesmo, = cura (aba Healer) quando a skill é de cura.
Jogador → jogador sem ser cura (PvP, buff com valor) e mob → mob ficam de fora.

DoT `0x3805` (hipótese): `varint alvo, u8 efeito (bit 0x02 = dano), varint autor,
varint desconhecido, u32 skill*100, varint dano`.

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
então o medidor só aproveita o nome quando a skill é de classe ou o matador já é jogador
conhecido. A tabela de ameaça (aggro) fica no servidor e não aparece em nenhum pacote; o
pacote de troca de alvo do mob também não foi achado.

## 6. Escala do dano (medido)

O campo `dano` é o número que o jogo mostra ao bater: um golpe de 2.574 na tela é 2.574 no
campo (conferido em 2026-10-02, quando o medidor ainda multiplicava por 18,82 e mostrou
48,4K). O medidor exibe o campo como vem, nas três abas.

O que sai da barra de HP do mob é outra conta. Em 150 intervalos entre dois
`0x8D00` do mesmo mob com um único golpe no meio, **queda de HP / campo = 18,82
(mediana)** nos 8 mobs, com variação de 18,75 a 18,93 entre skills. Os valores abaixo
disso são golpes finais (a queda fica limitada pelo HP restante).

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
varint  tamanho + UTF-8   nome do DONO
...     (posição em float etc.)
bloco:  [u32 dono][u32 legião][u16 0][u16 servidor][u8 tamanho][nome da legião]
```

O bloco do dono é achado por varredura com todas as validações juntas (formato descrito
pelo A2Tools em `docs/summon-attribution.md`). Na captura: Explosion Trap `#57692`,
dono `A6 2B 00 00` = `#11174` (Yoshi), legião "Tubazai". Sem legião o bloco vem zerado e
sobra o nome do dono. Ids são reemitidos ao trocar de zona.

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

## 9. Riscos e licenças

- ToS: as buscas não acharam posição oficial da NCSoft sobre medidores. As sanções
  públicas (jan/2026) miram macro e farm de ouro. Programa de terceiro continua sendo
  risco da conta.
- Opcodes e layout mudam com patch: depois de cada atualização, rodar `capturar.ps1` e o
  replay (seção "Depois de um patch" no README).
- `dados/skills.json` veio do RATmeter (GPL-3.0). Uso pessoal sem problema; distribuir
  o medidor com esse arquivo obriga a distribuir sob GPL-3.0. O código deste repositório
  foi escrito a partir do formato descrito aqui, sem copiar código do RATmeter.
