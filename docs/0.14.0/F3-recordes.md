# F3. Recordes e comparação de lutas (refinamento 0.14.0)

> Anexo do refinamento da 0.14.0 (2026-10-08), escrito por um agente e revisado na integração em
> `docs/0.14.0.md`. Os scripts e respostas citados em `<scratchpad>` ficaram na máquina do
> refinamento e não foram versionados; os pacotes e números que os testes usam estão neste texto.

Estado: refinamento. Nada codado; o repositório foi só lido (ramo `feat/0.14.0`, commit `6d5440c`).
Marcação: **conferido** (com a evidência) ou **hipótese**. Scripts da análise em
`scratchpad\refino\trabalho-F3\` (`chefes.py`, `hp35518.py`, `reconhecimento.py`, saída em
`chefes-saida.txt`).

**Primeiro o fato que pesa no plano: nenhum dos 11 dumps tem um kill de chefe que viraria recorde
pelas regras propostas abaixo.** Há 3 chefes nas capturas; o único que morre (Arconte da Alma
Perdida Axios) foi visto pela primeira vez com 86,3% do HP e sem "você" reconhecido até 34 s depois
da morte. As regras de validade foram desenhadas a partir desses casos negativos e do código; o caso
positivo (kill inteiro de chefe de dungeon) só vai ser conferido com uma captura nova (pergunta P1).

---

## 1. Negócio

### 1.1 Problema

O Axon guarda as 20 últimas lutas só na memória (`LUTAS_GUARDADAS = 20`, `LutaPassada` em
`medidor.rs`; o README diz "fechar o Axon apaga"). Quem repete a mesma dungeon não tem como saber se
o seu DPS ou o tempo de kill do grupo melhorou desde a semana passada. O dono aprovou disco para
isso (decisão de 2026-10-08 em `docs/0.14.0.md`), com a condição de que todo arquivo e todo campo
gravado seja listado e mínimo.

### 1.2 Histórias

1. Como jogador que repete a mesma dungeon, quero ver o meu melhor DPS e o melhor tempo de kill de
   cada chefe, para saber se estou melhorando.
2. Durante a luta com um chefe que já tem recorde, quero ver se estou à frente ou atrás dele: a
   previsão de kill contra o melhor tempo e o meu DPS contra o melhor DPS.
3. No fim da luta, quero saber se bati o recorde e por quanto.
4. Abrindo uma luta passada (☰), quero ver como ela ficou contra o melhor que existia antes dela.
5. Quero saber exatamente o que fica no PC, apagar um recorde ou todos, e desligar a função.
6. Quando uma luta não conta (chefe sem código, luta pela metade, chefe não morreu), quero ver o
   motivo, para não achar que o Axon errou.

### 1.3 Critérios de aceite

1. Com recordes ligados e nenhum kill válido ainda, nenhum arquivo novo aparece em
   `%LOCALAPPDATA%\Aion2Meter` (o arquivo nasce no primeiro recorde).
2. Um kill válido de chefe com código cria `recordes.json` com exatamente os campos da seção 2.3; um
   teste compara o JSON gerado com um esperado, campo a campo.
3. Nenhum nome de jogador (seu ou dos outros) vai para o arquivo; um teste gera recordes a partir de
   uma luta com nomes conhecidos e procura cada nome no JSON gravado.
4. Na guerra com chefe, com recorde existente para (chefe, sua classe), aparece uma faixa abaixo do
   card do alvo com o melhor tempo, a previsão de kill e o seu DPS contra o melhor.
5. No fim de um kill que bate recorde, a faixa e a linha da luta em ☰ marcam "Novo recorde" com o
   valor anterior; kill que não bate mostra a diferença.
6. Luta que não vale (parcial, sem morte, sem código, você não reconhecido, você sem golpe no chefe)
   deixa o arquivo byte a byte igual e mostra o motivo no mouse da linha em ☰.
7. Arquivo corrompido: o Axon abre normal, os recordes começam vazios, o original fica guardado
   como `recordes.json.corrompido` e a tela de recordes avisa.
8. Arquivo de versão mais nova (downgrade do Axon): os recordes aparecem, nada é gravado, e a tela
   avisa "só leitura".
9. "Apagar" num chefe tira só ele; "Apagar todos" remove o arquivo do disco.
10. Recordes desligados: o arquivo não é lido nem gravado, e a faixa do card some.
11. `--replay` nunca grava recordes (como a memória e a config hoje).

### 1.4 Escopo

Dentro:

- Recorde por (código do NPC, sua classe): melhor DPS seu e melhor tempo de kill.
- Comparação em três lugares: faixa abaixo do card do alvo (ao vivo e na luta passada aberta),
  marca na lista ☰ e uma tela de recordes.
- Um arquivo versionado, `recordes.json`, com gravação por troca e tratamento de corrompido.
- Configuração: ligar/desligar, apagar um, apagar todos.

Fora (e por quê):

- Curva de ritmo ao longo da luta ("à frente 6 s aos 1:30"): exige guardar uma série por recorde
  (até ~120 números por kill de 10 min em passos de 5 s). A previsão pelo "derrota em" cobre a
  pergunta "vou bater o recorde?" sem gravar série. Fica para depois, se o dono pedir.
- Persistir as 20 lutas: o histórico continua só na memória.
- Recordes de mob comum ou elite, e de Tank (DTPS) ou Healer (HPS).
- Luta com mais de um chefe ao mesmo tempo: nenhum caso nas capturas para conferir a regra.
- Chefe sem código do spawn: sem chave estável (ver 2.1).
- Composição do grupo (classes dos outros), nomes, servidor, legião, ranking online, exportar.

### 1.5 Configurações

| Opção | Proposta | Observação |
|---|---|---|
| Guardar recordes de chefe | liga/desliga, padrão em P2 | Desligado: não lê nem grava `recordes.json`, faixa do card some |
| Apagar um recorde | ✕ na linha da tela de recordes, 2 cliques ("Apagar?") | O overlay não recebe teclado; confirmação por clique |
| Apagar todos | botão na tela de recordes, 2 cliques | Remove o arquivo |
| Quais chefes | sem opção por chefe | Apagar um chefe resolve o caso de recorde indesejado; lista por chefe seria configuração sem uso claro |

A seção "Recordes" entra na tela de configurações nova da F4; se a F3 sair antes, entra na tela atual
(`janela/configuracoes.rs`) e a F4 move.

### 1.6 Perguntas ao dono

- **P1. Captura de referência.** Uma captura de chefe de dungeon do pull ao kill, com o Axon (ou o
  `capturar.ps1`) aberto antes de entrar na dungeon, e, se possível, uma de world boss desde o spawn
  com HP cheio. Sem elas, as regras de validade só são testadas com eventos sintéticos.
- **P2. Padrão da opção.** Recomendo ligado, com o arquivo nascendo só no primeiro recorde e listado
  no README (Privacidade). Alternativa mais conservadora: desligado, com um aviso único na tela ☰.
- **P3. Chave com a classe.** Recomendo (chefe, classe): um alt de outra classe não mistura recorde
  com o principal. Dois personagens da mesma classe dividem o recorde, porque o arquivo não guarda
  o seu nome. Aceita?
- **P4. Tempo de kill em world boss.** No Axios bateram 784 jogadores: o tempo é da multidão.
  Recomendo guardar só o DPS quando 30 ou mais jogadores bateram no chefe (o mesmo limite que já
  marca chefe pela multidão, `ATACANTES_DE_CHEFE`). Aceita?
- **P5. Tamanho do grupo.** O grupo não é detectável (o `0x9702` nunca apareceu; README,
  Pendências). Recomendo um melhor tempo só, com "N jogadores" ao lado, sem separar por tamanho.
- **P6. Campos opcionais.** `kills` (contador de kills válidos por chefe) entra? Composição por classe
  (só contagem, ex.: 1 Cleric, 1 Templar) fica fora, como proposto?
- **P7. Arquivo corrompido.** Recomendo mover para `recordes.json.corrompido` e seguir com recordes
  vazios. Alternativa: deixar o arquivo onde está e desligar os recordes até o usuário clicar
  "Apagar e recomeçar" (nenhum arquivo extra, mas a função para).
- **P8. Desligar apaga?** Recomendo desligar sem apagar; "Apagar todos" é separado.
- **P9. Limites numéricos.** Luta completa = HP no primeiro golpe ≥ 99% do máximo; DPS só conta com
  pelo menos 20 s ativos no chefe. Os dois números são proposta minha, sem dado que os fixe.
- **P10 (fora da F3, mas do mesmo tema).** O `jogadores.json` de hoje tem 5.489 perfis, um por nome
  de jogador visto, em 193.641 bytes (**conferido**: contagem das chaves de `Perfis` no arquivo local,
  sem ler os nomes). Para quem é sensível a "nada no PC", ele pesa mais que os recordes. Vale abrir
  uma frente para limitar ou tornar opcional?

---

## 2. Técnico

### 2.1 Dados (11 dumps, servidor → cliente)

Método: `chefes.py` lê `0x3641` (spawn: código, tipo, HP), `0x3804`/`0x3805` (dano), `0x8D00` (HP),
`0x8D04` (morte), `0x8D21` (combate e prazo), `0xE005` e `0x3633`/`0x3645`, com os layouts de
`combate.rs`. Chefe = regra do Axon (`InfoNpc::chefe` sobre o `npcs-pt.json` local, ou 30+ atacantes)
ou mob com prazo no `0x8D21` ou com `0xE005`. Ressalva: o `despejar.rs` que gerou os dumps usa um
montador TCP só para todos os fluxos do servidor; perda ou troca de fluxo pode faltar pacote.

**Mobs em geral (conferido, `resumo-mobs.txt`):** 113 mobs levaram golpe de jogador nas 11
capturas; 79 (70%) tinham o código do spawn. Dos 34 sem código, 19 já estavam na tela nos 10
primeiros segundos da captura (o caso esperado). Os outros 15 surgiram depois (9 na
`captura-2026-10-02-combate`, aparecendo pela primeira vez entre 19 e 43 s em `0x8D52` ou `0x3835`)
e mesmo assim nenhum `0x3641` deles veio no dump. Causa não fechada (**hipótese**: perda na
montagem do dump, ou outro pacote de entrada na visão). Consequência: mesmo com o Axon aberto antes,
parte dos kills pode ficar sem chave.

**Chefes (pergunta 1): 3 nas capturas, 2 com código e 1 sem (conferido).**

| Chefe | Código | Spawn | HP no 1º golpe visto | Atacantes | Morte | Você |
|---|---|---|---|---|---|---|
| Arconte da Alma Perdida Axios, #21799 (`captura-2026-10-03-boss`) | 2400425 | 14,6 s | 138.076.470 de 160.000.000 (86,3%) | 784 | 120,7 s | 318 golpes, 173.138 de dano (0,1%), DPS ativo 1.941/s; reconhecido só aos 155,2 s |
| Profanador Newbold, #21524 (mesma captura) | 2400424 | 175,1 s | 155.543.296 de 160.000.000 (97,2%) | 89 | não veio (captura acaba em 182,8 s) | sem golpe |
| #35518 (`captura-2026-10-06-tempo-boss`) | nenhum | fora da captura | 9.857.329, sem máximo | 16 | não veio (captura acaba em 122 s com 7.987.931) | reconhecível só aos 116,7 s |

Detalhes conferidos:

- **Axios:** o spawn chegou com o chefe já a 86,3%: a luta começou antes de ele entrar na visão. Do
  primeiro golpe visto à morte: 106,0 s. O último golpe de jogador veio em 121,1 s, e 50 golpes
  chegaram até 1 s depois da morte. O único "entrou em combate" (`0x8D21` = 1) veio 0,04 s depois da
  morte (a reentrada já descrita no PROTOCOLO §5b); por isso o `0x8D21` não serve de início do pull
  aqui. "Você": o `0x3633` só veio aos 155,2 s, com o mesmo id que bateu no chefe; antes disso nenhum
  `0x8D04` nem spawn de invocação trouxe o seu nome guardado (`reconhecimento.py`). Pelo código do
  medidor (não rodado nesta captura), a luta do Axios fecha no primeiro golpe mais de 1 s depois da
  morte, ainda sem "você", e não geraria recorde pessoal. O mecanismo é **conferido**: o retrato da
  luta é tirado no `encerrar_luta`, com o `meu_id` daquele instante (`montar_tabela`).
- **Newbold:** chegou batido (97,2%) e a captura acaba antes da morte.
- **#35518:** spawn fora da captura, então sem código e sem HP máximo. `0x8D21`: saiu de combate em
  8,78 s (o HP subiu de 9.856.161 para 9.860.000 em 8,83 s, volta ao cheio, **hipótese**: máximo =
  9.860.000), entrou em 10,58 s com prazo de 300,3 s, saiu em 20,88 s sem morrer e entrou de novo em
  68,08 s com prazo novo de 300,3 s. Ou seja, um pull desfeito antes do pull que valeu. Tem `0xE005`.
  Hoje o Axon nem o trata como chefe: sem código, o questlog não responde, e 16 atacantes ficam
  abaixo dos 30 de `ATACANTES_DE_CHEFE` (**conferido** em `Medidor::chefes`). Você aparece só aos
  116,7 s, num spawn de invocação (`0x3641` tipo `0x5F`) com o seu nome guardado.

**O que fazer com chefe sem código (pergunta 1):** sem recorde, com o motivo no mouse da faixa e da
linha em ☰ ("o chefe já estava na tela quando o Axon abriu"). Não existe chave alternativa: o id de
entidade muda a cada conexão (PROTOCOLO §7b: o mesmo personagem foi #11174, #7301 e #11179), e o HP
máximo também vem do spawn (§7c), além de ser igual nos dois world bosses (160.000.000). A barra do
`0xE005` traz outro número (1200 no #35518), sem ligação conhecida com o chefe. **Conferido.**

**Como o tempo é contado hoje (pergunta 2):** na guerra com chefe, `Placar.duracao = ultimo - min(1º
golpe de jogador em cada chefe)` (`obter_placar`, `medidor.rs` linhas 957 a 960). `ultimo` é o último
evento aceito na luta, de qualquer tipo, e a luta aceita evento até 1 s depois da morte do último
chefe (`TOLERANCIA_FIM`). No Axios isso daria pelo menos 106,4 s (último golpe de jogador; outro
evento aceito até 121,7 s esticaria mais) contra 106,0 s da morte. `LutaPassada.inicio`
e `.fim` são da luta inteira, e nem `Placar` nem `LutaPassada` guardam a hora da morte do chefe.
**Conferido no código.**

Cache local, para dimensionar (conferido): `npcs-pt.json` com 438 NPCs, 47 deles chefes pela regra
do Axon.

### 2.2 Regras

**Tempo de kill:** hora do `0x8D04` do chefe menos o primeiro golpe de jogador nele dentro da luta.
Começar pelo `0x8D21` de entrada fica de fora: no mob que puxa, ele chega até 22 s antes do dano
(PROTOCOLO §5b), e no world boss a entrada estava fora da visão. Começar pelo golpe é o mesmo
relógio que a guerra com chefe já mostra, só que terminando na morte.

**Kill que vale (pergunta 2).** Avaliado no `encerrar_luta`, sobre o retrato da luta naquele instante:

| # | Condição | Vale para | Caso nos dados |
|---|---|---|---|
| 1 | Exatamente 1 chefe na luta, com código do spawn | DPS e tempo | #35518 cai aqui |
| 2 | O chefe morreu na luta (`0x8D04`), depois do 1º golpe | DPS e tempo | Newbold e #35518 caem aqui |
| 3 | Você reconhecido no fim da luta, com dano no chefe > 0 | DPS e tempo | Axios cai aqui (reconhecido 34 s depois) |
| 4 | Seu tempo ativo no chefe ≥ 20 s (P9) | DPS | o aDPS tem piso de 1 s: um golpe final de 1 s viraria DPS enorme |
| 5 | HP do chefe no 1º golpe ≥ 99% do máximo do spawn (P9) | tempo | Axios (86,3%) e Newbold (97,2%) caem aqui |
| 6 | O chefe não saiu de combate sem morrer durante a luta (um `0x8D21` = 0 mais de 1 s antes da morte) | tempo | o #35518 saiu em 20,88 s e voltou em 68,08 s |
| 7 | Menos de 30 jogadores bateram no chefe (P4) | tempo | Axios (784) |

Sem mínimo de jogadores: kill solo vale. "Você participou" = condição 3. A condição 6 cobre o reset
dentro da mesma luta (um add em combate pode segurar a luta aberta enquanto o chefe reseta); quando o
chefe é o único mob, o `0x8D21` = 0 já fecha a luta hoje (`registrar_estado_combate`, conferido no
código), e o pull seguinte começa luta e relógio novos.

**Recorde novo:** DPS maior que o guardado; tempo menor que o guardado. Empate não troca. O primeiro
kill válido de um (chefe, classe) vira recorde.

### 2.3 Arquivo `recordes.json` (para o OK do dono)

Arquivos que a F3 pode criar em `%LOCALAPPDATA%\Aion2Meter`:

| Arquivo | Quando existe |
|---|---|
| `recordes.json` | Do primeiro recorde até "Apagar todos" |
| `recordes.json.tmp` | Só durante a gravação (some no `rename`); pode sobrar se o PC desligar no meio, e é ignorado e sobrescrito na próxima |
| `recordes.json.corrompido` | Só se o `recordes.json` estiver ilegível ao abrir (P7) |

Formato: JSON formatado (como o `config.json`), para o dono poder abrir e conferir. Exemplo
ilustrativo (código e números fictícios, coerentes com as regras de 2.2; o melhor DPS e o melhor
tempo podem vir de lutas diferentes):

```json
{
  "versao": 1,
  "recordes": [
    {
      "npc": 2400999,
      "classe": "Ranger",
      "kills": 3,
      "dps": { "valor": 2310.5, "data": 1791401200, "ativo_ms": 148200, "jogadores": 4 },
      "tempo": { "ms": 151870, "data": 1791487600, "jogadores": 4, "dps": 2204.8 }
    }
  ]
}
```

| Campo | Tipo | O que é | Por que precisa |
|---|---|---|---|
| `versao` | inteiro | Versão do esquema (1) | Migração e proteção contra downgrade |
| `recordes` | lista, até 500 | Um item por (chefe, classe) | |
| `npc` | inteiro, 1.000.000 a 9.999.999 | Código do NPC do spawn (o id do questlog) | Chave. Nome, level e retrato vêm do `npcs-pt.json`, não deste arquivo |
| `classe` | texto, uma das 9 classes | A sua classe naquela luta, como o Axon mostra | Chave (P3) |
| `kills` | inteiro | Kills válidos contados | Opcional (P6) |
| `dps.valor` | número | Seu DPS no chefe: dano no chefe dividido pelo seu tempo ativo nele, o mesmo número da aba DPS | Recorde de DPS |
| `dps.data` | inteiro | Hora da morte do chefe, em segundos Unix (relógio do Windows) | Mostrar "em 08/10" |
| `dps.ativo_ms` | inteiro | Seu tempo ativo no chefe (1º ao último golpe seu) | Explicar o número e aplicar a regra 4 na leitura |
| `dps.jogadores` | inteiro | Quantos jogadores bateram no chefe | Contexto ("4 jogadores") |
| `tempo.ms` | inteiro | Morte menos 1º golpe de jogador no chefe | Recorde de tempo |
| `tempo.data` | inteiro | Igual a `dps.data`, da luta do melhor tempo | |
| `tempo.jogadores` | inteiro | Jogadores que bateram no chefe naquela luta | Contexto (P5) |
| `tempo.dps` | número | Seu DPS na luta do melhor tempo | Comparar o ritmo do kill mais rápido |

O que **não** vai para o arquivo: nenhum nome (o seu nem o dos outros), id de entidade, classe dos
outros, servidor, legião, dano por skill, HP, posição, golpe. "Ocultar nomes" não se aplica ao
arquivo porque ele não tem nome nenhum; na tela, recordes mostram só você (pela classe) e contagens.

Tamanho (conferido serializando o exemplo acima em Python): 183 bytes por recorde em JSON compacto;
o arquivo formatado com 500 recordes dá 164.037 bytes (~160 KB). Limite: 500 recordes (passou, sai o de data mais
antiga) e leitura recusada acima de 1 MB.

Regras de leitura, gravação e migração:

1. **Leitura:** ausente = vazio, sem criar nada. Acima de 1 MB, ou JSON ilegível (inclui vazio e
   truncado), = corrompido. BOM no começo é aceito (como `config.rs`).
2. **Corrompido:** renomeia para `recordes.json.corrompido` (substituindo um anterior), começa vazio,
   avisa na tela de recordes. Nunca grava por cima sem antes mover. Hoje `Config::carregar` e
   `carregar_memoria` voltam ao padrão e o próximo salvamento sobrescreve o arquivo ruim
   (**conferido**: `config.rs` linha 79, `janela.rs` linhas 1937 a 1943); copiar esse padrão perderia
   recordes.
3. **Item inválido** (código fora da faixa, classe desconhecida, número negativo ou não finito, `ms` 0
   ou acima de 24 h, `jogadores` 0): o item sai, os outros ficam, e o aviso diz quantos saíram. Chave
   repetida: fica o melhor de cada campo. A data serve só para mostrar: data estranha (antes de
   2026-01-01 ou no futuro, como num PC com o relógio errado na hora da gravação) aparece como "data
   desconhecida" e o recorde fica.
4. **Versão:** ausente ou menor = migra na memória e grava na próxima mudança; maior que a conhecida =
   só leitura (não grava nada, nem apagar), com aviso.
5. **Gravação:** só quando um recorde muda ou o usuário apaga; nunca no `--replay`. Escreve o `.tmp`,
   chama `sync_all` e faz o `rename`. O `catalogo::escrever_trocando` de hoje não chama `sync_all` e
   os chamadores ignoram o erro com `let _` (**conferido**, `catalogo.rs` linhas 786 a 792). Para
   recordes: erro devolvido, recordes ficam na memória marcados como "por gravar", nova tentativa na
   próxima mudança e a cada 30 s, aviso na tela se continuar falhando.
6. **Duas instâncias do Axon:** nada impede hoje (**conferido**: `main.rs` não tem trava de instância).
   Antes de gravar, relê o arquivo e mescla o melhor de cada chave, para uma instância não apagar o
   recorde da outra. Efeito colateral aceito: um "Apagar" numa instância pode voltar pela outra se as
   duas estiverem abertas.
7. **Por que gravar na hora:** o release usa `panic = "abort"` (`Cargo.toml`), então o `on_exit` não
   roda num pânico; recorde que esperasse o fechamento se perderia.

Atomicidade: a documentação do Rust diz que `fs::rename` no Windows usa `MoveFileExW` (com
alternativa por `SetFileInformationByHandle`) e não fala de atomicidade. Que a troca é atômica no
NTFS é **hipótese**; o teste 2.6-F10 cobre o efeito que importa (arquivo velho ou novo, nunca pela
metade, do ponto de vista da leitura).

### 2.4 Comparação (pergunta 4)

| Onde | Quando | O que mostra |
|---|---|---|
| Faixa abaixo do card do alvo (16 px) | Guerra com chefe, chefe com código, você reconhecido, recorde existente para (chefe, sua classe) | `★ 2:41 (4 jog.)  ·  previsão 2:58 (+17 s)  ·  DPS 12,3K de 13,1K (−6%)`. Previsão = duração atual + "derrota em". Verde quando à frente, vermelho quando atrás, dourado no recorde |
| A mesma faixa, depois do kill | Card em "Derrotado" | `Novo recorde 2:35 (antes 2:41)` ou `2:58, recorde 2:41 (+17 s)`; o mesmo para DPS |
| Faixa, luta passada aberta (☰) | Kill avaliado | O resultado contra o melhor que existia antes dela (guardado na memória no fim da luta, e não recalculado contra o recorde de agora) |
| Lista ☰ | Toda luta com chefe | ★ na linha de recorde novo; no mouse, as diferenças ou o motivo de não contar ("Axon viu o chefe com 86% do HP", "chefe sem código", "você não foi reconhecido") |
| Tela de recordes | Botão "Recordes" no topo da tela ☰ (o cabeçalho já tem 6 ícones) | Por recorde: retrato, nome e Nv (do cache), melhor tempo e jogadores, melhor DPS, kills, data; ✕ por linha; "Apagar todos"; o caminho do arquivo e os avisos de 2.3 |

Barra compacta: sem comparação (sem espaço). Resumo copiado: sem mudança.

### 2.5 Onde encaixa no código

`crates/nucleo` (regra e comparação, sem disco):

- `medicao/medidor.rs`:
  - `LutaPassada` ganha `abate: Option<Abate>` com: código, 1º golpe no chefe, hora da morte, HP no
    1º golpe e máximo, se saiu de combate sem morrer, quantos jogadores bateram, quantos chefes.
  - `Medidor` ganha `hp_no_primeiro_golpe` (gravado em `registrar`, na linha do
    `primeiro_golpe_em.entry(...)`, a partir de `hp_de`, que ainda tem o HP de antes do golpe:
    **conferido** nos dumps, em 826 primeiros golpes de jogador nenhum `0x8D00` do mesmo instante veio
    antes do golpe, e em 143 o do mesmo instante veio logo depois, `ordem_hp.py`; se um dia vier antes,
    a folga de 1% absorve um golpe em chefe de HP alto) e
    `saiu_de_combate_em` (no ramo `em_combate == false` de `registrar_estado_combate`). Os dois zeram
    em `encerrar_luta`.
  - `encerrar_luta` monta o `Abate` junto com o `obter_placar()` que já faz, antes de limpar os
    mapas; a hora da morte sai de `mortos`, que sobrevive ao fim da luta.
- `medicao/recordes.rs` (novo): `Recorde`, `Recordes`, `Candidato`, `Resultado` (novo DPS, novo
  tempo, anteriores, motivo de não contar), `candidato(&LutaPassada)`, `registrar`, leitura com
  validação e migração a partir de texto, serialização. Funções puras.

`crates/overlay` (disco e tela):

- Arquivo novo para ler/gravar `recordes.json` recebendo o caminho (testável com pasta temporária),
  com o `sync_all` e o tratamento de corrompido de 2.3.
- `janela.rs`: carregar em `Overlay::novo` (como `carregar_memoria`); em `ler_placar`, a cada leitura
  e não só com a tela ☰ aberta, avaliar as lutas com número maior que a última avaliada, guardar o
  `Resultado` por número de luta (memória) e gravar se mudou; faixa nova em `barra_do_alvo`;
  `Tela::Recordes`.
- `janela/lutas.rs`: `ResumoLuta` com o resultado; botão "Recordes". Tela nova em `janela/recordes.rs`.
- `config.rs`: campo `recordes` (P2), com teste de config antiga sem o campo, como o que já existe.
- README: seções Lutas e Privacidade (o arquivo novo e os campos).

### 2.6 Plano de verificação

Ordem: escrever cada teste antes do código e ver falhar (red check), depois implementar. Comandos
pela ferramenta Bash: `CARGO_TARGET_DIR=target/agente-F3 cargo test --workspace`.

Regras de kill (`crates/nucleo/tests/medidor.rs` e `tests/recordes.rs`, eventos sintéticos):

- K1. Kill inteiro (spawn com HP cheio, golpes, morte): tempo = morte − 1º golpe; um golpe atrasado
  0,5 s depois da morte não muda o tempo (a `duracao` muda, o tempo não).
- K2. Spawn com 86% do HP (o caso do Axios): DPS vale, tempo não, motivo "parcial".
- K3. Chefe sem código (o caso do #35518): sem candidato, motivo "sem código".
- K4. Zerar antes da morte, ou captura acabando sem morte: sem candidato.
- K5. Chefe sai de combate sem morrer e volta, com um add segurando a luta: tempo não vale.
- K6. Você reconhecido só depois do fim da luta (o caso do Axios): sem candidato; reconhecido no meio
  da luta: vale.
- K7. Você sem golpe no chefe; tempo ativo abaixo do mínimo: sem recorde de DPS.
- K8. 30 jogadores no chefe: DPS vale, tempo não.
- K9. Dois chefes na luta: sem candidato.
- K10. Comparação: primeiro kill vira recorde; empate não troca; luta passada aberta compara com o
  melhor de antes dela.

Arquivo (fronteira de confiança, obrigatórios):

- F1. Ausente: vazio e nenhum arquivo criado até o primeiro recorde.
- F2. Ilegível (truncado no meio, bytes aleatórios, vazio, só BOM): movido para `.corrompido` com o
  conteúdo byte a byte igual ao original, recordes vazios, aviso.
- F3. Acima de 1 MB: mesmo tratamento, sem chamar o parser.
- F4. Versão maior: só leitura; registrar e apagar não mudam o arquivo.
- F5. Versão ausente ou menor: migra; ler, gravar e ler de novo dá o mesmo resultado.
- F6. Itens inválidos (cada regra de 2.3, item 3): saem, os válidos ficam; chave repetida fica o
  melhor; data no futuro ou antes de 2026 não derruba o item e aparece como "data desconhecida".
- F7. Ida e volta: gravar e ler devolve os mesmos recordes; o JSON tem só os campos da tabela 2.3.
- F8. Sem nomes: luta com nomes conhecidos gera um arquivo que não contém nenhum deles.
- F9. Falha de gravação (pasta inexistente ou arquivo só leitura): erro devolvido, recordes na memória
  intactos e marcados "por gravar"; a próxima tentativa grava.
- F10. `.tmp` pela metade sobrando de uma queda: a leitura usa o `recordes.json`, e a próxima gravação
  sobrescreve o `.tmp`.
- F11. Limite de 500: o 501º tira o de data mais antiga.
- F12. Mescla: arquivo mudado por outra instância entre a leitura e a gravação não perde o recorde dela.
- Red check extra: trocar a leitura pelo padrão de `config.rs` (`unwrap_or_default` e gravar por cima)
  tem de derrubar F2.

Na janela (debug, sem o jogo): `cargo run -p overlay -- --replay captura-2026-10-03-boss.pcapng --lutas`
deve mostrar a luta do Axios em ☰ com o motivo de não contar e não criar `recordes.json` (o replay não
grava). Uma luta sintética que vale exige a captura de P1 ou um modo de teste que injete um recorde; o
segundo é decisão de implementação.

### 2.7 Riscos

| Risco | Efeito | Mitigação |
|---|---|---|
| Spawn não visto (Axon aberto depois, ou os 15 casos sem causa fechada) | Kill sem recorde | Motivo no mouse; README já pede para abrir antes |
| "Você" reconhecido tarde | Kill sem recorde (caso do Axios) | Motivo no mouse; o login (`0x3633`) resolve com o Axon aberto antes de entrar no mundo |
| Limite de 99% recusar kill legítimo em world boss com multidão (1% de 160 M cai em ~1,2 s com 1,3 M/s) | Tempo não registrado | Só o tempo depende disso, e em world boss o tempo já sai (P4) |
| Chefe com fase de cura ou invulnerável | Nenhum efeito na regra (não se usa subida de HP) | A regra 6 olha só a saída de combate |
| Código do NPC mudar num patch | Recorde dividido em dois (**hipótese**) | Apagar à mão |
| Patch de balanceamento | Recordes velhos inalcançáveis | Apagar por chefe |
| Relógio do PC errado | Só a data mostrada sai errada | Nenhuma |
| Duas instâncias | Recorde de uma sumir | Mescla antes de gravar (2.3, item 6) |
| `rename` não atômico em algum disco | Arquivo pela metade | `sync_all` antes, `.corrompido` na leitura, F10 |

### 2.8 Tamanho e dependências

Tamanho: **M**. Núcleo: o `Abate` no `encerrar_luta`, `recordes.rs` e cerca de 22 testes. Overlay:
arquivo com tratamento de erro, faixa do card, marca e motivo em ☰, tela de recordes, uma opção.

Dependências:

- **F2 (relatório de morte):** as duas mexem no `0x8D04` (`registrar_morte`) e no retrato da luta em
  `encerrar_luta`/`LutaPassada`. Sem dependência de dado (a F3 usa a morte do chefe, a F2 a do
  jogador), mas quem entrar segundo ajusta o `LutaPassada` do primeiro.
- **F4 (configurações com seções):** a seção "Recordes" (ligar e apagar todos) entra na estrutura
  dela; sem a F4, entra na tela de hoje.
- **F1 (barra de groggy, `0xE005`):** relação, sem dependência. O #35518 tinha `0xE005` e prazo e hoje
  não é tratado como chefe; se a F1 usar o `0xE005` para reconhecer chefe, a guerra com chefe passa a
  valer para ele, mas o recorde continua precisando do código.

---

## 3. URLs consultadas

- https://doc.rust-lang.org/std/fs/fn.rename.html (comportamento do `rename` no Windows)
- https://doc.rust-lang.org/std/fs/struct.File.html (`sync_all` e `sync_data`)
