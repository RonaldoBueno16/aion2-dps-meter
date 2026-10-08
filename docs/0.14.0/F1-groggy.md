# F1. Barra de groggy do chefe (refinamento, 0.14.0)

> Anexo do refinamento da 0.14.0 (2026-10-08), escrito por um agente e revisado na integração em
> `docs/0.14.0.md`. Os scripts e respostas citados em `<scratchpad>` ficaram na máquina do
> refinamento e não foram versionados; os pacotes e números que os testes usam estão neste texto.

Escopo: mostrar no card do alvo e na barra compacta a barra de groggy que o servidor manda no
`0xE005`, o momento em que ela quebra e quanto dura o groggy. Toda afirmação sobre pacote vem
marcada como **conferido** (com a evidência) ou **hipótese**. Os scripts estão em
`scratchpad/refino/trabalho-F1/` (`v1_varrer.py` a `v20_buff7.py`; `comum.py` tem o port em Python
do `combate::dano`). "Dump do chefe" = `dumps/captura-2026-10-06-tempo-boss.txt`; os tempos são
segundos desde o primeiro pacote desse dump.

Resumo em cinco linhas:

- O `0xE005` apareceu em 1 dos 11 dumps, só no chefe 35518, sempre com máximo 1200. Os dois
  chefes do world boss de 2026-10-03 (Axios e Newbold) não mandaram nenhum.
- A barra caiu de 1200 a 6 em 45,6 s, quebrou (`00 03 02`), o groggy durou 5 s e ela voltou cheia.
- O groggy chega também como um buff no próprio chefe (código 10.000.002, 5.000 ms), o que permite
  a contagem regressiva sem número fixo no código.
- Dentro do groggy, 200 de 201 golpes no chefe vieram com tipo 3 (crítico, pela leitura do
  RATmeter); fora, 75 de 1.143.
- A queda acompanha a skill, com valor por skill (1 a 50), e muda com buff do autor. Dá para
  atribuir 81% da queda a um jogador, numa luta só: fica fora da 0.14.0.

---

## 1. Negócio

### 1.1 Problema

Em chefe com barra de groggy, o grupo precisa saber quanto falta para quebrar a barra e quando
o groggy abre: nos 5 s dele, 200 de 201 golpes no chefe vieram com tipo de dano 3, que o RATmeter
lê como crítico (**hipótese** até conferir na tela; seção 2.1, pergunta 4). Hoje o
Axon lê o `0xE005` e não usa (PROTOCOLO.md:62). O card do alvo mostra HP, "derrota em" e
"mate em" (janela.rs:603 a 660); a barra compacta mostra o mesmo em uma linha (janela.rs:1372 a
1389). Nada indica o groggy.

Ponto que condiciona tudo: a regra do projeto é não mostrar o que o jogo esconde
(memória `axon-chefes-info-do-jogo`). O guia oficial cita a "그로기 게이지" numa skill do Templar,
o que sugere que o jogo mostra a barra, mas a página renderiza por JavaScript e não consegui
conferir o texto (seção 3). **Hipótese** até o dono mandar um print do quadro do chefe com a
barra.

### 1.2 Histórias de usuário

- **H1.** Como jogador numa luta de chefe com barra de groggy, quero ver no card do alvo quanto
  da barra ainda falta, para segurar as skills fortes até o groggy.
- **H2.** Como jogador, quero ver quando o chefe entrou em groggy e quantos segundos faltam, para
  gastar o dano nessa janela.
- **H3.** Como jogador usando a barra compacta, quero o aviso de groggy na mesma linha do alvo.
- **H4.** Como jogador, quero desligar a barra de groggy se ela atrapalhar a leitura do card.

### 1.3 Critérios de aceite (verificáveis)

Os replays usam `captura-2026-10-06-tempo-boss.pcapng` (raiz do repositório, fora do git).
Nessa captura você não é reconhecido (não há `0x3633`), então o alvo é o mob que mais apanhou,
o 35518 (medidor.rs:1022 a 1028).

- **CA1.** `cargo run -p overlay -- --replay captura-2026-10-06-tempo-boss.pcapng --ate 87`: o
  card mostra a barra de groggy em 619 de 1200 (o último `0xE005` antes de 87 s chegou a 86,127 s,
  e o seguinte só a 87,325 s, o que dá folga de mais de 0,8 s para a diferença entre o início do
  pcapng e o do dump). Com o mouse no card, o texto traz "619 de 1200".
- **CA2.** Teste do medidor com os pacotes reais (seção 2.6): depois do `00 03 02`, o `Alvo` traz
  o groggy ativo; com o buff 10.000.002 chegando 0,05 s depois, o fim do groggy fica na hora do
  buff + 5.000 ms.
- **CA3.** No mesmo teste, o `0xE005` de 1200/1200 que chega depois do groggy (119,676 s) volta o
  estado para a barra cheia, sem groggy, e o contador de quebras da luta fica em 1.
- **CA4.** `0xE005` com qualquer forma fora das duas vistas (outro par de bytes no lugar de
  `03 01`, outro tamanho, atual acima do máximo, máximo 0) não muda nada: teste com o pacote real
  alterado (red check).
- **CA5.** Saída de combate (`0x8D21` com 0), morte do chefe e conexão nova apagam o estado de
  groggy daquele id.
- **CA6.** Replay de `captura-2026-10-03-boss.pcapng` (Axios, sem `0xE005`): o card fica igual ao
  da 0.13.1, sem barra nova.
- **CA7.** Com a opção desligada, nem a barra nem o texto de groggy aparecem no card e na
  compacta (teste de `config.rs` para o padrão e para o `config.json` antigo sem o campo).

### 1.4 O que entra

- Parser do `0xE005` nas duas formas vistas, com opcode e nome em `opcodes.rs`.
- Estado por id no `Medidor`: atual, máximo, groggy ativo, fim do groggy (pela duração do buff),
  quebras na luta.
- Card do alvo: barra fina de groggy junto da barra de HP; texto "GROGGY 3 s" na linha de
  detalhe só durante o groggy; atual e máximo crus no texto do mouse.
- Barra compacta: "GROGGY 3 s" antes do nome do alvo, só durante o groggy.
- Uma opção de liga/desliga (padrão ligado).
- PROTOCOLO.md (seção nova) e README.md (funcionalidade e pendência).
- Bloco de diagnóstico no replay (quedas, quebras e duração por entidade), ao lado do "Razão
  queda/dano" (replay/src/main.rs:540), para conferir depois de patch.

### 1.5 O que fica de fora

- **Quem quebrou a barra** (ranking de groggy por jogador): a atribuição fecha 81% da queda numa
  luta só, depende de um parser novo do `0x3804` de seletor 0 e o valor por skill muda com buff
  (seção 2.1, pergunta 3). Um número estimado também esbarra na regra de exibir só o que o
  servidor manda (memória `axon-valores-crus`). Fica para depois de capturas de outros chefes.
- **"Quebra em 0:18"** (estimativa de tempo até quebrar, como o "derrota em"): conta derivada,
  permitida pela regra, mas a linha de detalhe já tem "derrota em" e "mate em". Pergunta ao dono.
- Alerta sonoro ou visual de "groggy começou": é da F5 (alertas).
- Quebras por luta no histórico, no resumo copiado e nos recordes: é da F3.
- Posição do chefe, distância, quem luta longe: nunca (regra do projeto).

### 1.6 O que vira configuração (seção "Luta" ou "Card do alvo", a decidir na F4)

| Campo no `config.json` | Padrão | Efeito |
|---|---|---|
| `mostrar_groggy` | `true` | Barra no card, "GROGGY n s" no card e na compacta |
| `groggy_na_compacta` (opcional) | `"durante"` | `"durante"`: só no groggy; `"sempre"`: também a % fora dele; `"nunca"` |

O `Config` usa `#[serde(default)]` (config.rs:21), então um `config.json` antigo continua abrindo
com os padrões.

### 1.7 Perguntas para o dono

1. **O jogo mostra a barra de groggy no quadro do chefe?** Mande um print com a barra pela metade
   e outro durante o groggy. Sem isso, a F1 não deve ser publicada (regra do que o jogo esconde).
2. Número cru ou %? Proposta: a barra desenhada no card, "619 de 1200 (51,6%)" só no mouse. O
   jogo mostra algum número na barra dele?
3. A barra aparece desde a entrada em combate (o servidor manda 1200/1200 nessa hora) ou só
   depois da primeira queda?
4. Na compacta: só durante o groggy (proposta), ou sempre com a %?
5. Contagem regressiva do groggy pela duração do buff (5 s, visto uma vez): pode mostrar?
6. "Quem quebrou a barra" entra numa versão futura, mesmo sendo estimativa?
7. Qual chefe e qual dungeon eram o 35518 de 2026-10-06 (23:32)? O spawn ficou fora da captura.
8. Captura nova (seção 2.7): pode gravar uma luta inteira desse chefe ou de outro chefe de
   dungeon, com o Axon aberto antes de entrar?

---

## 2. Técnico

### 2.1 Dados (respostas às perguntas da frente)

#### Pergunta 1. O `0xE005` em outros chefes ou mobs

- **Conferido** (`v1_varrer.py`, `v6_ops.py`): nos 11 dumps (446.699 pacotes, nenhum `0xFFFF`
  sobrando, ou seja, tudo descomprimido), o `0xE005` aparece só no dump do chefe: 224 pacotes,
  todos da entidade 35518, de 8,83 a 121,97 s. Nenhum outro opcode `0xE0xx` aparece em nenhum dump.
- **Conferido**: máximo 1200 em 223 de 223 pacotes com valor.
- **Conferido** (`trabalho-F3/chefes-saida.txt`): os dumps têm outros dois chefes, ambos em
  `captura-2026-10-03-boss`, e nenhum mandou `0xE005`:
  - Arconte da Alma Perdida Axios (#21799, NPC 2400425): 69.674 golpes de 784 jogadores, 106 s do
    primeiro golpe à morte;
  - Profanador Newbold (#21524, NPC 2400424): 1.731 golpes de 89 jogadores, morte fora da captura.
- **Hipótese**: a barra existe só em parte dos chefes (o 35518 tinha prazo de 300 s e luta de 5
  jogadores com `0x9702` de grupo e sem `0x9101` de chefes de campo, cara de chefe de dungeon). O
  "máximo sempre 1200" vale para n = 1 chefe.

#### Pergunta 2. Depois do `00 03 02`, e outros formatos

- **Conferido** (`v2_serie.py`): só duas formas de corpo em 224 pacotes.

  | Forma | Bytes depois da entidade | Pacotes |
  |---|---|---|
  | Valor | `03 01` + u32 máximo + u32 atual + `02` (11 bytes) | 223 |
  | Quebrou | `00 03 02` (3 bytes) | 1 |

- **Conferido**: linha do tempo do ciclo (dump do chefe).

  | Tempo (s) | Pacote | O que mostra |
  |---|---|---|
  | 8,781 / 8,829 | `0x8D21` saiu (`00 00`) / `0xE005` 1200/1200 | barra cheia logo depois de sair de combate |
  | 10,531 / 10,583 | `0xE005` 1200/1200 / `0x8D21` entrou, com prazo | barra cheia logo antes de entrar |
  | 20,879 / 20,929 | `0x8D21` saiu / `0xE005` 1200/1200 | idem |
  | 23,027 | `0xE005` 1200/1200 | sem `0x8D21` junto |
  | 68,077 | `0x8D21` entrou, com prazo | início da luta |
  | 68,776 a 114,424 | 218 `0xE005` com queda | 1200 → 6, quedas de 1 a 50 |
  | 114,524 | `0xE005` `00 03 02` | barra quebrou |
  | 114,576 | `0x382A` no chefe: tipo 0x11, código 10.000.002, duração 5.000 ms, autor = o próprio chefe | groggy |
  | 119,626 | `0x382C` da mesma instância (811) | fim do buff, 5,050 s depois |
  | 119,676 | `0xE005` 1200/1200 | barra cheia de novo, 5,152 s depois do `00 03 02` |
  | 119,976 a 121,975 | 10 `0xE005` com queda | 1200 → 1118 até o fim da captura |

- **Conferido**: a barra não chegou a 0 em valor: o último valor foi 6 (114,424 s) e o pacote
  seguinte já foi o `00 03 02`. O PROTOCOLO.md:62 diz "1200 → 16"; o certo é 1200 → 6.
- **Hipótese**: a quebra vem no lugar do valor que zeraria a barra.
- **Hipótese** sobre a estrutura geral: `[varint entidade][u8 máscara][u8 estado]`, com u32
  máximo e u32 atual presentes conforme a máscara, e `02` no fim. Duas formas não bastam para
  fechar isso, então o parser aceita só as duas, byte a byte.
- O chefe não morreu na captura: não se sabe o que vem na morte, nem se há segunda quebra com
  outro máximo.

#### Pergunta 3. A queda acompanha o dano ou só algumas skills

- **Conferido** (`v7_atrib.py`): cada uma das 218 quedas teve pelo menos um golpe direto no chefe
  nos 300 ms anteriores. A janela de 300 ms não serve para atribuir: o chefe levou 24,9 golpes
  diretos por segundo (1.143 golpes de 68,7 a 114,5 s), ou cerca de 7 golpes por janela.
- **Conferido** (`v4_ordem.py`, `v11_instante.py`): a atribuição usa o `0x3804` de seletor 0, que
  o `combate::dano` recusa (combate.rs:30) e que chega no mesmo instante de captura do golpe da
  mesma skill. Toda queda (218 de 218) chegou num instante com pelo menos um `0x3804` de seletor 0
  no chefe; nenhuma chegou em instante sem ele.
- **Conferido** (`v5_sel0.py`): o seletor 0 não é exclusivo do groggy. Ele aparece nos 11 dumps
  (55.695 no world boss, 11.602 deles no Axios), inclusive com jogador como alvo.
- **Conferido** (`v11_instante.py`): nos 148 instantes com um só cast de seletor 0 e queda, o
  valor bateu com a moda da skill em 138. Os 10 restantes são todos do autor 9209, todos durante o
  buff 112500201 nele (seguinte).
- **Conferido** (`v17_buff3.py`, `v20_buff7.py`): o buff 112500201 (Bênção de Zikel, skill
  11250020, Gladiator) ficou no autor 9209 de 71,23 a 85,83 s. Nessa janela as skills dele
  derrubaram mais, sem exceção nos instantes de cast único:

  | Skill | Sem o buff | Com o buff |
  |---|---|---|
  | Golpe Feroz Dilacerante (1101) | 2 (14 de 14) | 3 (5 de 5) |
  | Golpe Feroz Esmagador (1142) | 2 (14 de 14) | 3 (3 de 3) |
  | Esmagada Descendente (1117) | 5 (4 de 4) | 7 (1 de 1) |
  | Skill 1144 (fora do cache) | 5 (4 de 4) | 7 (1 de 1) |

  As contagens desta tabela vêm do `v20_buff7.py`, que vai até o fim da captura (inclui os 2 s
  depois do groggy); as da tabela seguinte, do `v11_instante.py`, que para em 114,45 s.

  **Hipótese**: o buff multiplica o groggy por 1,5, arredondando para baixo.
- **Conferido** (`v11_instante.py`, até 114,45 s): valor modal por skill (nomes do cache
  `skills-pt.json`, que vem do questlog; entre parênteses, quantos instantes com queda tiveram a
  skill como único cast de seletor 0):

  | Valor | Skills |
  |---|---|
  | 50 | Explosão Divina (1) |
  | 35 | Chamas Infernais (6) |
  | 25 | Golpe Feroz da Perdição (2) |
  | 20 | Tempestade Gélida (1) |
  | 15 | Flecha de Ruptura (2), Espada do Massacre (1), Golpe de Investida (1, durante o buff) |
  | 10 | Flecha de Vento Tempestuoso (3), Congelamento (3), Vento do Frio Intenso (3), Flecha Laçadora (2), Salto Esmagador (1), Ataque Sigiloso (1) |
  | 7 | Explosão de Chamas (10), Explosão Congelante (5), Explosão de Insígnia (2), Corte do Clarão (2) |
  | 5 | Saraivada Flamejante (8), Esmagada Descendente (4), 1144 (3), Estocada no Coração (1) |
  | 2 | Golpe Feroz Dilacerante (13), Golpe Feroz Esmagador (14), Coice da Besta (3), Golpe Brutal da Besta (2), Rugido da Besta (2) |
  | 1 | Disparo Rápido (42) |
  | sem queda | Marca de Fogo (36 instantes de cast único sem queda), Chama Intensa (18), Aplicação de Veneno (9), Barreira de Fogo (3), Explosão Tardia (3), Exploração de Fraqueza (3) |

  Fora da tabela, por amostra fraca ou contraditória: Aprisionamento Aéreo teve 6 instantes de
  cast único sem queda até 114,45 s e 1 com queda de 20 aos 121,875 s; Flecha Certeira (16) e
  Flecha de Grifo (20) só aparecem no pareamento por ordem do fluxo (`v9_pareio.py`), que acerta
  a moda da skill em 78% dos pares e por isso não entra como evidência.

- **Conferido**: o valor não acompanha o dano. Groggy por ponto de dano vai de 0,0006 (Explosão
  Congelante: 7 num golpe de 11.572) a 0,0046 (Saraivada Flamejante: 5 com dano mediano de 1.095);
  Disparo Rápido tira 1 com dano mediano de 381, Golpe Feroz da Perdição tira 25 com 16.170.
- **Conferido** (`v10_hex.py`): o valor de groggy não está dentro do `0x3804`, nem no golpe nem
  no seletor 0 (procurado nos golpes de Chamas Infernais, Golpe Feroz da Perdição e Explosão de
  Chamas). **Hipótese**: é dado da skill no servidor.
- **Conferido** (`v12_cobertura.py`): juntando os valores modais, 161 dos 192 instantes com queda
  fecham numa combinação única de casts; isso atribui 969 de 1.194 pontos (81%). Parte de cada
  autor nessa parte atribuída: 11476 com 24,5%, 10518 com 24,1%, 9209 com 23,8%, 10735 com 13,1%,
  7184 com 7,2%; o resto (7,1%) vem de 6 ids que só usaram skills de Sorcerer em poucos golpes
  (**hipótese**: invocações).
- Conclusão: dá para atribuir por skill e por jogador com uma tabela de valores e uma regra de
  instante, em 81% da queda e numa luta só. **Hipótese** até mais capturas: fica fora da 0.14.0.

#### Pergunta 4. Dano no groggy e marcador do estado

- **Conferido** (`v14_marcador.py`): dentro do groggy (114,524 a 119,676 s), 200 de 201 golpes
  diretos no chefe vieram com tipo de dano 3; de 68,7 a 114,5 s, 75 de 1.143 (6,6%). O tipo 3 é
  crítico pela leitura do RATmeter, ainda pendente de conferir na tela (README).
- **Conferido** (`v15_critico.py`), amostra pequena:
  - crítico dentro / crítico fora, mesma skill e autor: mediana 0,97 (4 pares, 7 golpes dentro);
  - crítico dentro / normal fora: mediana 1,57 (27 pares, 78 golpes dentro), de 1,26 a 3,39;
  - crítico fora / normal fora: mediana 1,90 (5 pares).
- **Conferido** (`v13_groggy.py`): dano do grupo no chefe 43.176/s dentro e 33.733/s fora (+28%);
  queda de HP 44.411/s dentro e 33.869/s fora; golpes diretos 39,0/s dentro e 24,9/s fora. A razão
  queda de HP / dano ficou em 1,009 dentro e 1,012 fora.
- **Hipótese**: o groggy faz todo golpe sair crítico, sem multiplicador extra de dano; o dano por
  segundo sobe pelo crítico e porque o grupo concentra golpes na janela.
- **Conferido**: marcadores do groggy:
  - `0x382A` tipo 0x11 com código 10.000.002 e duração 5.000 ms no chefe, autor o próprio chefe
    (linha 16635 do dump), e o `0x382C` da instância 811 aos 119,626 s (linha 17914). É a única
    ocorrência do código 10.000.002 nos 11 dumps (n = 1);
  - o pipeline atual descarta esse buff: `registrar_buff` sai cedo com
    `!skill_de_classe(b.codigo / 10)` (medidor.rs:835).
- **Conferido**: o `0x8D21` não marca o groggy (nenhum `0x8D21` do chefe entre 68,08 s e o fim).
- **Conferido**, sem uso proposto: aos 114,624 s o chefe mandou `0x3802` com as skills 1.000.009
  e 1.001.450 (únicas vezes) e um `0x380C` `[chefe][08 18][u32 1604400][b0 ea 01]`. O `0x380C`
  aparece em outros 6 dumps (21 pacotes), então não serve de marcador. **Hipótese**: o groggy
  interrompeu um cast do chefe (1604400 é skill dele, vista no `0x3802` aos 76,08 s).

#### Pergunta 5. O que mostrar e o que é configurável

Seções 1.4, 1.6 e 2.4.

### 2.2 Layout proposto para o PROTOCOLO.md

Nova seção "5d", e a linha do `0xE005` na tabela (PROTOCOLO.md:62) troca "1200 → 16" por
"1200 → 6" e aponta para ela.

```
## 5d. Barra de groggy `0xE005` (lida num chefe só)

varint  entidade
forma A (11 bytes): 03 01  u32 máximo  u32 atual  02     valor da barra
forma B (3 bytes):  00 03 02                             a barra quebrou: groggy
```

Texto proposto:

- Chefe 35518 de 2026-10-06: 223 pacotes na forma A, todos com máximo 1200, e 1 na forma B. A
  barra caiu de 1200 a 6 em 45,6 s, aos saltos de 1 a 50, cada salto junto com um `0x3804` de
  seletor 0 da skill que bateu; a forma B veio no lugar do valor que zeraria.
- O groggy chega também como buff no chefe: `0x382A` tipo 0x11, código 10.000.002, duração 5.000
  ms, autor o próprio chefe. O `0x382C` veio 5,05 s depois e a barra voltou a 1200/1200 0,05 s
  depois dele.
- 1200/1200 também chega na entrada e na saída de combate (`0x8D21`).
- Dentro do groggy, 200 de 201 golpes no chefe vieram com tipo de dano 3.
- O valor de cada salto depende da skill (Disparo Rápido 1, Chamas Infernais 35) e de buff do autor
  (com a Bênção de Zikel, 2 virou 3 e 5 virou 7). O pacote não traz o autor.
- Nenhum `0xE005` nos dois chefes do world boss de 2026-10-03.
- Hipótese de estrutura: `[u8 máscara][u8 estado]`, com os u32 conforme a máscara; o medidor
  aceita só as duas formas, byte a byte.

### 2.3 Modelo de dados

Núcleo (`crates/nucleo`):

```rust
// protocolo/combate.rs
pub enum BarraGroggy {
    Valor { entidade: u32, maximo: u32, atual: u32 },
    Quebrou { entidade: u32 },
}
/// 0xE005: só as duas formas vistas; o resto dá None.
pub fn barra_groggy(pacote: &[u8]) -> Option<BarraGroggy>;

// medicao/medidor.rs
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Groggy {
    pub atual: u32,
    pub maximo: u32,
    /// Hora do `00 03 02`; None fora do groggy.
    pub quebrou_em: Option<Hora>,
    /// Hora do buff 10.000.002 + duração dele; None sem o buff.
    pub fim: Option<Hora>,
    /// Vezes que a barra quebrou nesta luta (para a F3).
    pub quebras: u32,
}
// em Alvo (medidor.rs:110): pub groggy: Option<Groggy>,
// no Medidor: groggy_de: HashMap<u32, Groggy>,
```

Regras:

- Forma A: grava atual e máximo, apaga `quebrou_em` e `fim` (a barra cheia encerra o groggy).
- Forma B: `quebrou_em = hora`, `quebras += 1`.
- Buff 10.000.002 num id com `groggy_de`: `fim = hora + duração`. A contagem usa a hora de captura
  do pacote mais a duração; não depende do relógio do servidor (diferente do "mate em").
- Saída de combate limpa `quebrou_em` e `fim`; morte e `nova_conexao` (medidor.rs:924 a 943)
  apagam o id. O 1200/1200 que o servidor manda logo depois da saída (8,83 e 20,93 s) recria o
  estado, cheio. Se o dono escolher mostrar a barra só depois da primeira queda (pergunta 3), o
  card a esconde enquanto `atual == maximo` e `quebras == 0`; com a outra escolha, ela aparece
  desde o primeiro `0xE005`.
- Id de jogador conhecido é ignorado, como no `registrar_hp` (medidor.rs:525).
- `quebras` zera no início de cada luta.

### 2.4 Onde encaixa no código

| Arquivo | Mudança |
|---|---|
| `crates/nucleo/src/protocolo/opcodes.rs` | `BARRA_GROGGY: u16 = 0xE005` e o nome em `nome()` (linhas 8 a 55) |
| `crates/nucleo/src/protocolo/combate.rs` | `BarraGroggy` e `barra_groggy()`, perto do `estado_combate` (linha 115) |
| `crates/nucleo/src/medicao/sessao.rs` | braço novo em `ao_pacote` (linha 186); no braço `BUFF_NOVO` (linha 249), `if buff.codigo == 10_000_002` chama `registrar_fim_groggy` antes do `registrar_buff`, que descarta código fora de classe (medidor.rs:835) |
| `crates/nucleo/src/medicao/medidor.rs` | `Groggy`, `groggy_de`, `registrar_barra_groggy`, `registrar_groggy`, `registrar_fim_groggy`; limpeza em `registrar_morte` (linha 662), `registrar_estado_combate` (linha 731) e `nova_conexao` (linha 941); `groggy` preenchido em `alvo()` (linha 1032 a 1046) |
| `crates/overlay/src/janela.rs` | card: barra fina de groggy junto da barra de HP (linhas 603 a 614) e "GROGGY n s" na linha de detalhe (linhas 651 a 660); texto do mouse do card (linha 559) com "Groggy 619 de 1200"; compacta: "GROGGY n s" antes do nome (linhas 1386 a 1390) |
| `crates/overlay/src/config.rs` | `mostrar_groggy` (e `groggy_na_compacta`, se o dono quiser) com padrão em `Default` (linha 54) |
| `crates/replay/src/main.rs` | bloco "Groggy" por entidade: quedas, quebras, duração até a barra cheia |
| `PROTOCOLO.md`, `README.md` | seção 5d; funcionalidade no README perto do "Mate em" (README.md:73) |

Desenho no card (proposta; a altura do card é 48 px, janela.rs:557):

- A barra de HP ocupa de `max.y - 7` a `max.y - 4`. A de groggy entra logo abaixo, 2 px, em
  âmbar, do mesmo `x` inicial ao mesmo `x` final; sem `0xE005` para o alvo, nada muda (CA6).
- No groggy: a barra fica cheia em âmbar e a linha de detalhe ganha "GROGGY 3 s" em negrito
  âmbar, antes do "mate em". Sem o buff, só "GROGGY", sem número.
- Na tela Lutas (luta passada), o card mostra a barra como ficou e nunca o "GROGGY n s".

### 2.5 Riscos

- **Amostra de um chefe e um ciclo.** Máximo, formas, duração de 5 s e o código 10.000.002 vieram
  de n = 1. Forma nova vira None e a funcionalidade some calada; o bloco do replay mostra isso
  depois de patch.
- **Chefe que o Axon não reconhece.** O 35518 não tem spawn na captura e teve 16 atacantes, abaixo
  dos 30 de `ATACANTES_DE_CHEFE` (medidor.rs:183). Com você reconhecido e batendo num add, o card
  troca para o add e a barra de groggy some. Receber `0xE005` (ou o `0x8D21` com prazo) é sinal
  forte de chefe: decisão conjunta com a F3.
- **CRIT% da aba DPS.** Em luta com groggy, o CRIT% sobe: nesta luta, 201 dos 1.409 golpes diretos
  no chefe caíram no groggy, 200 deles com tipo 3. Se o tipo 3 não for crítico, o CRIT% já está
  errado hoje; a captura com print resolve as duas coisas.
- **Espaço na linha de detalhe e na compacta.** Já há "Nv · Chefe · derrota em · mate em". O
  "GROGGY n s" aparece só por 5 s; fora disso, a barra não usa texto.
- **Relógio no replay.** A contagem compara com a hora atual, como o "mate em"
  (`falta_para_o_prazo`, janela.rs:1779): no replay ela sai vencida. O teste fica no medidor, com
  horas explícitas.
- **Regra do que o jogo esconde.** Sem o print do quadro do chefe, a F1 não pode ser publicada.

### 2.6 Plano de verificação

Testes com pacotes reais do dump do chefe (hex completo, opcode incluso):

| Teste | Pacote | Linha do dump | Esperado |
|---|---|---|---|
| valor | `1405E0BE95020301B00400009704000002` | 5763 (68,776 s) | Valor 35518, 1200, 1175 |
| último valor | `1405E0BE95020301B00400000600000002` | 16607 (114,424 s) | Valor 35518, 1200, 6 |
| quebra | `0C05E0BE9502000302` | 16623 (114,524 s) | Quebrou 35518 |
| barra cheia | `1405E0BE95020301B0040000B004000002` | 17921 (119,676 s); o mesmo hex está nas linhas 759, 895, 1741 e 1904 | Valor 35518, 1200, 1200 |
| buff do groggy | `322A38BE95020111AB06829698008813000000000000F3453614A1010000BE95020100E8031BC70658BBC600DA5847` | 16635 (114,576 s) | `combate::buff`: alvo 35518, instância 811, código 10.000.002, duração 5.000 |
| fim do buff | `0E2C38BE95020100AB0601` | 17914 (119,626 s) | `buffs_removidos`: (35518, [811]) |

Red check, nesta ordem:

1. Escrever os testes de `parsers.rs` e do medidor antes do código e ver falharem.
2. Variantes que devem dar None: `03 02` no lugar de `03 01`; corpo cortado em 10 bytes; atual
   1201 com máximo 1200; máximo 0; `00 03 03`.
3. Teste do medidor (`tests/medidor.rs`, no estilo de
   `prazo_para_matar_vale_ate_o_chefe_sair_de_combate_ou_morrer`, linha 383): 1200 → 6, quebra em
   T, buff em T + 0,05 s, barra cheia em T + 5,15 s; conferir `quebrou_em`, `fim = T + 0,05 s + 5 s`,
   volta ao normal e `quebras == 1`; morte, saída de combate e `nova_conexao` limpam.
4. Teste da janela (como `prazo_fica_vermelho_quando_a_derrota_estimada_passa_dele`,
   janela.rs:2349): texto "GROGGY 3 s" com `fim` 3 s à frente, "GROGGY" sem `fim`, nada sem groggy.
5. Replay manual: CA1 (`--ate 87`, 619 de 1200) e CA6 (Axios, sem barra).

O que precisa de captura nova com print do jogo (pedido ao dono):

1. Abrir o Axon e o `capturar.ps1` **antes** de entrar na dungeon do 35518 (ou de outro chefe com
   barra), para vir o spawn com o código do NPC.
2. Print do quadro do chefe com a barra de groggy pela metade e outro durante o groggy. Anotar a
   hora de cada print.
3. Luta inteira até a morte, de preferência com duas quebras: confere se o máximo muda, se a
   duração continua 5 s e o que vem na morte.
4. Print de um número de dano durante o groggy, para conferir o tipo 3 como crítico.
5. Anotar o nome do chefe, a dungeon e as classes do grupo.

### 2.7 Tamanho

**M.** Parser, opcode e testes: P. Estado no medidor e testes: P. Card, compacta, configuração e
texto do mouse: P a M (layout apertado). Replay, PROTOCOLO.md e README: P.

### 2.8 Dependências com outras frentes

- **F3 (recordes e comparação)**: reconhecer como chefe o mob com `0xE005` ou com prazo no
  `0x8D21`; quebras por luta e tempo até quebrar (68,8 a 114,5 s = 45,7 s nesta captura) como
  número da luta.
- **F4 (configurações com seções)**: em qual seção fica `mostrar_groggy`.
- **F5 (alertas)**: alerta de "groggy começou" usaria o `Quebrou` desta frente.
- **0.13.1 ("mate em")**: divide a linha de detalhe do card e da compacta.

---

## 3. URLs consultadas

| URL | Método | Resultado |
|---|---|---|
| https://aion2.plaync.com/ko-kr/guidebook/view?title=%EC%88%98%ED%98%B8%EC%84%B1+%EC%8A%A4%ED%82%AC | GET (1 vez) | Só o título "가이드북 : AION2-NC"; a página renderiza por JavaScript. A citação de "그로기 게이지" na skill 심판 방패 **não foi conferida** por mim |

Nenhum outro site foi consultado. Os nomes de skill vêm do cache local do Axon
(`%LOCALAPPDATA%\Aion2Meter\skills-pt.json`, montado do questlog), lido sem rede.
