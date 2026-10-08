## F2. Relatório de morte

> Anexo do refinamento da 0.14.0 (2026-10-08), escrito por um agente e revisado na integração em
> `docs/0.14.0.md`. Os scripts e respostas citados em `<scratchpad>` ficaram na máquina do
> refinamento e não foram versionados; os pacotes e números que os testes usam estão neste texto.

Pedido do dono (2026-10-08): "quando morre também, isso tudo!". Ao você morrer, mostrar o que te
acertou antes da morte (skill, quem bateu, dano, golpe final) e o que mais ajudar a entender a morte.

Convenções deste texto: **conferido** traz a evidência (dump, contagem, arquivo:linha ou script em
`scratchpad\refino\trabalho-F2\`); **hipótese** é leitura ainda sem prova. Nenhum nome de jogador
aparece aqui: jogadores e mobs vão por id de entidade, NPCs pelo código do questlog.

### Negócio

**Problema.** Hoje a morte vira só um "☠1" na sua linha da aba Tank (`janela.rs:866-868`) e uma
contagem no mouse (`janela.rs:769-783`). O medidor soma o dano recebido por skill e por luta, sem a
hora de cada golpe (ver Q1), então não dá para responder "o que me matou e como cheguei lá".

**Exemplo com pacotes reais.** A única morte de jogador das 11 capturas (world boss de 2026-10-03,
jogador #16201, tratado aqui como se fosse você) daria este relatório (**conferido**,
`linha_morte.py`):

```
┌ ☠ Você morreu  14:32:05 ───────────────────── ver relatório › ┐
│ [retrato] <nome do NPC 2400425>   Golpe 1235330    4.926       │
│ HP antes do golpe 4.850 (passou 76) · 2 golpes do monstro em 4,5 s │
└────────────────────────────────────────────────────────────────┘

┌ ‹ Medidor ───────────── Morte das 14:32:05 ───────────────────┐
│ Golpe final                                                    │
│ [retrato] <nome do NPC 2400425>  Golpe 1235330  4.926          │
│ HP antes 4.850 · passou 76                                     │
│                                                                │
│ Últimos 10 s                                     HP depois     │
│ -4,5 s  <mob>   Golpe 1235300            -2.386      2.612     │
│ -4,2 s  você    <skill de cura>          +1.306      3.918     │
│ -3,5 s  você    efeito 2011101 (2×)       sem valor  4.374     │
│ -1,5 s  você    efeito 2011101 (2×)       sem valor  4.830     │
│ -0,0 s  <mob>   Golpe 1235330            -4.926          0     │
│                                                                │
│ Recebido 7.312 · maior golpe 4.926 · 1 monstro · curas 1.306    │
└────────────────────────────────────────────────────────────────┘
```

A hora (14:32:05) é ilustrativa; os números, os golpes e os ids são os do pacote. A conta fecha no
HP que o servidor mandou: 4.998 − 2.386 = 2.612; 2.612 + 1.306 = 3.918; 4.850 −
4.926 = 0 (o golpe passou 76). O "efeito 2011101" vem no pacote de dano periódico com autor = o
próprio morto; o HP subiu 456 a cada ~2 s nesse trecho, mas o valor do pacote não bate com a subida
(335 + 126 contra +456; 2 + 3 contra +456), por isso ele aparece sem valor (**hipótese**: cura
contínua ou poção).

**Histórias.**
- Como jogador que acabou de morrer, quero ver em um relance quem me deu o golpe final, com que
  golpe, quanto tirou e com quanto HP eu estava.
- Como jogador, quero a sequência dos últimos segundos (golpes, curas, HP depois de cada um) para
  saber se morri de um golpe só, de vários monstros ou por falta de cura.
- Como jogador, quero reabrir a morte depois, pela lista de lutas, sem ter ficado olhando o overlay
  na hora.

**Critérios de aceite.**
1. Com os 19 pacotes reais da morte do #16201 (lista em "Plano de verificação"), tratando o #16201
   como você, o relatório traz: golpe final skill 1235330 do autor #21799, NPC 2400425, dano 4.926,
   HP antes 4.850, excesso 76; nos últimos 10 s, 2 golpes de monstro somando 7.312; 1 cura direta de
   1.306 (skill 18170000); os 4 efeitos 2011101 fora do dano recebido. Teste automatizado.
2. Morte que chega depois de a luta ter fechado ainda gera relatório (hoje `registrar_morte` descarta
   a morte de jogador nesse caso, `medidor.rs:688`). Teste.
3. Golpe no morto que chega depois do 0x8D04 não entra no relatório. Teste.
4. Invocação, armadilha ou pet que morre não gera relatório. Teste.
5. A luta que acabou com a sua morte guarda o relatório: `lutas_passadas()[0]` tem a morte, e a tela
   Lutas abre o mesmo relatório. Teste e conferência visual com `--replay`.
6. Com "Ocultar nomes", o nome de um matador jogador (PvP) vira o nome da classe. Teste.
7. O card aparece no primeiro ciclo de leitura do placar depois da morte (500 ms no padrão) e some
   depois do tempo configurado ou no ✕. Conferência visual com `--replay`.
8. Nada novo em disco: os relatórios ficam na memória e somem ao fechar o Axon, como as lutas.

**Entra.**
- Relatório da **sua** morte: golpe final (quem, golpe, dano quando o matador é mob, HP antes,
  excesso), linha do
  tempo dos últimos N s (golpes, curas, efeitos de jogador sem valor, HP depois de cada um) e um
  resumo (recebido, maior golpe, quantos monstros bateram, curas recebidas).
- Card "☠ Você morreu" no medidor por alguns segundos, com clique para o relatório.
- Tela própria do relatório (`Tela::Morte`), aberta pelo card, pela sua linha expandida na aba Tank e
  pela luta na tela Lutas.
- Leitura nova do HP de jogador no 0x8D00 (ver Q2b), que hoje é descartada.
- Na tela Lutas, "☠" na linha da luta em que você morreu.

**Fica de fora (desta versão).**
- Relatório da morte de outros jogadores e do grupo (ver Q5; o grupo depende do 0x9702, ainda não
  capturado).
- Nome da skill de monstro: não existe fonte pública (ver Q3). Fica "Golpe 1235330" com o nome e o
  retrato do mob.
- Dano de golpe de outro jogador (PvP). Golpe direto de jogador em jogador sem skill de cura nem
  sempre é dano: no world boss, 241 golpes diretos de invocações de jogador (skill 1699xxxx) em
  jogadores, e nos 4 com leitura de HP em até 0,3 s o HP subiu (**conferido**, `golpe_1699.py`). Numa
  morte em PvP, o relatório traz o matador e a skill do 0x8D04, sem o dano do golpe final, e os golpes
  de jogador aparecem como efeito sem valor, com o HP depois.
- HP máximo e % de HP do jogador: o pacote de jogador não traz o máximo conhecido (a chave 7 apareceu
  em 70 de 16.316 pacotes, sem significado conferido; `hp_formato.py`).
- Buffs e debuffs ativos na hora da morte (pergunta 3 abaixo).
- Ressurreição e tempo morto: o pacote não é conhecido.
- Quem bateu longe, posição e distância: o jogo não mostra, e o Axon não vai mostrar.
- Gravar relatório em disco.
- Reconhecer você pelo 0x8D00 (ver Q4): mexe no app inteiro, vai como item separado.

**Configurações** (na página "Luta" da F4, `docs/0.14.0.md:160`):

| Campo no `config.json` | O que faz | Faixa | Padrão |
|---|---|---|---|
| `relatorio_morte` | Liga o card e o relatório | ligado ou desligado | ligado |
| `card_morte_s` | Quanto tempo o card fica na tela | 5 a 60 s | 15 s |
| `janela_morte_s` | Quantos segundos antes da morte o relatório mostra | 5 a 30 s | 10 s (pergunta 4) |

Os três entram com `#[serde(default)]` e passam por `dentro_das_faixas` (`config.rs:94-100`), como
os campos de hoje.

**Perguntas para o dono.**
1. Só a sua morte nesta versão (recomendado) ou também a de quem está perto? No world boss de
   2026-10-03, 115 jogadores apanharam de monstro em 183 s; o relatório de cada um vira uma lista
   longa e sem grupo não dá para filtrar quem é do seu time.
2. O card fica abaixo do card do alvo, acima das abas (recomendado), ou toma o lugar do card do alvo
   por alguns segundos?
3. Os buffs de classe ativos em você na hora da morte entram no relatório? O dado existe (0x382A/B/C,
   com nome pela skill de origem); debuff de monstro não tem nome.
4. A janela do relatório vira configuração (5 a 30 s) ou fica fixa em 10 s?
5. "Reconhecer você pelo pacote de HP" (Q4) entra na 0.14.0 como item separado? Ele resolve o
   "(você)" logo no começo da sessão em todo o app, e precisa de uma captura em grupo antes.
6. As mortes ficam só junto das 20 lutas guardadas (somem com elas) ou numa lista própria das
   últimas N mortes desta execução?
7. A barra compacta mostra o "☠ 4.926 <mob>" pelo mesmo tempo do card?
8. Morte em PvP sai só com o matador e o golpe, sem o dano, até uma captura de PvP conferir o dano de
   jogador em jogador. Pode ser assim?

### Técnico

#### Dados (respostas às perguntas da frente)

**Q1. O medidor guarda cada golpe com a hora?** Não. **Conferido** em `medidor.rs`: `Acumulado`
(`234-244`) e `SomaSkill` (`223-232`) guardam total, golpes, críticos, aparos, costas, máximo e um
`Ativo` com a hora do primeiro e do último evento (`198-221`); `somar` (`1191-1222`) só soma. O
único estado com hora por golpe é `ultimo_alvo_do_mob` (`278`), um golpe por mob, para o aggro.
Precisa de buffer:

- O que guardar por evento recebido: hora, autor, skill, valor, tipo (golpe, periódico, cura,
  efeito de jogador), crítico, aparo, costas. Cerca de 32 bytes.
- Janela: até 30 s (o máximo da configuração), com teto de 128 eventos por jogador.
- Carga medida (**conferido**, `carga.py`): em você, no máximo 15 golpes de monstro em 10 s e 23 em
  30 s (teleporte de 2026-10-02). No world boss, 1.486 golpes de monstro em jogador em 182,8 s,
  pico de 89 por segundo, 115 jogadores atingidos, no máximo 17 golpes num mesmo jogador em 10 s.
- Memória: guardando para todo jogador conhecido, o pior caso do world boss é 89/s × 30 s ≈ 2.700
  eventos ≈ 85 KB, mais as curas. O teto de 128 eventos limita cada jogador a 4 KB.

**Q2. Morte do usuário e de outros jogadores nos dumps; campos do 0x8D04.**

Contagem dos 372 pacotes 0x8D04 das 11 capturas (**conferido**, `mortes_desconhecidos.py`,
`sem_spawn.py`):

| Morto | Quantos |
|---|---|
| Você (id do 0x3633 em 6 dumps; id do 0x8D00 de bit 0 nos outros 5, ver Q4) | **0** |
| Outro jogador | **1** (#16201, world boss de 2026-10-03, aos 110,0 s do dump) |
| Invocação, armadilha ou pet (spawn tipo 0x5F) | 89 |
| Entidade com spawn 0x3641 de outro tipo (mob) | 250 |
| Sem spawn na captura | 32, todos mortos por skill de classe e batidos por skill de classe (mobs) |

Nenhuma captura tem a sua morte: o relatório de você será conferido com a morte do #16201 e com uma
captura nova (ver "Plano de verificação").

Campos do 0x8D04 (**conferido**, `corpo_morte.py`, `combate.rs:345-365`, PROTOCOLO.md §5):

```
varint  morto
u32     skill que matou
varint  matador                    (0 em 279 de 372: armadilha ou invocação que expirou)
u16     servidor                   (1000..9999 nos 92 abates por jogador; 0 no abate por mob)
u8 + UTF-8  nome do matador        (veio nos 92 abates por jogador; vazio no abate por mob)
u8 + UTF-8  legião
u16     desconhecido               (2 nos 64 com legião, 0 nos 29 sem)
varint  código do NPC do matador   (0 nos 92 abates por jogador; 2400425 no abate por mob)
6 bytes desconhecido               (00 00 00 00 01 00 ou ... 01 02)
```

- O varint depois do u16 é o código do NPC do matador: no único abate por mob (a morte do #16201),
  veio 2400425, igual ao código do spawn do matador #21799. **Conferido uma vez** (n = 1). Ele dá o
  nome e o retrato do matador mesmo quando o spawn do mob não foi visto. O parser atual para no nome
  (`combate.rs:354-360`).
- O último byte veio `02` em 2 pacotes: a morte do #16201 e a de uma entidade com spawn tipo 0x1C
  (código 2920360) sem matador (`caso0102.py`). Significado não fechou; não usar.
- O 0x8D04 não traz o dano do golpe final: ele sai do buffer.

Golpe final casado com o buffer, nos 93 abates com matador (92 deles são mortes de mob, então para
morte de jogador é **hipótese** com n = 1; `golpe_final.py`):

- o último golpe no morto antes do 0x8D04 é do matador em 79 de 93, com a mesma skill em 75;
- um golpe do matador com a skill do 0x8D04 está nos 2 s anteriores em 88 de 93;
- o 0x8D04 chega no máximo 0,057 s depois do último golpe (mediana 0,05 s);
- em 37 de 93, ainda chegou golpe no morto até 1 s depois do 0x8D04.

Regra proposta: entre os eventos com valor de dano (Golpe e Periodico, ver "Onde encaixa"), o golpe
final é o último com autor = matador (depois de `resolver_autor`, `medidor.rs:574-591`) e o mesmo
`skill_base`; sem ele, o último golpe do buffer; sem nenhum (inclusive na morte em PvP), só o que o
0x8D04 traz (matador e skill, sem dano). O relatório congela no 0x8D04, e o
que chega depois fica de fora.

**Q2b. HP de jogador no 0x8D00 (correção do protocolo).** PROTOCOLO.md §5 (`144-154`) diz que o
HP de jogador vem num formato diferente do de mob, e o medidor descarta o 0x8D00 de jogador
conhecido (`medidor.rs:524-528`). O formato é um só (**conferido**, `hp_formato.py`: fechou no
último byte em 16.316 de 16.316 pacotes das 11 capturas):

```
varint  entidade
u8      bits
bit 0:  u8 n, n × [u8 chave][u32 valor]     (chaves 1, 3, 4, 6, 8; sem significado conferido)
bit 1:  u8 n, n × [u8 chave][u64 valor]     (chave 0 = HP; chave 7 em 70 pacotes, desconhecida)
```

- O "mob" de hoje (`02 01 00` + u64, `combate.rs:89-101`) é este formato com bits 2, n 1, chave 0.
  Por isso `hp_restante` acerta o mob e erra o pacote com bit 0.
- Bits 1: 3.394 pacotes; bits 2: 12.642; bits 3: 280.
- Outros jogadores recebem bits 2 com o HP: 7.264 pacotes de ids vistos no 0x3645, 6.844 deles de
  115 jogadores no world boss (`hp_quem.py`, `bit0.py`).
- Chave 0 é o HP também em jogador: a queda entre dois 0x8D00 em volta de um golpe de monstro foi
  igual ao dano em 89 de 93 golpes em você e em 565 de 899 em outros jogadores; nos 338 restantes, o
  HP caiu menos (cura ou escudo no meio, **hipótese**) em 337 e mais em 1 (`hp_jog2.py`). Isso fecha a pendência do README
  "Tank: escala 1:1 conferida num golpe só" (`README.md:409-410`).
- Na morte do #16201, o 0x8D00 com HP 0 chegou junto com o golpe final, 0,04 s antes do 0x8D04.

**Q3. Nome da skill e do mob que bateu.**
- Skill de mob: sem fonte de nome. `dados_jogo::nome_skill` devolve "Golpe de monstro (código)"
  para skill fora da faixa de jogador (`dados_jogo.rs:85-94`), e o README registra que o questlog só
  tem skill de jogador e a tabela do RATmeter cobre 9% dos golpes de mob (`README.md:291-295`). O
  `Medidor::nome_skill` já monta "Golpe 1235330 · <mob>" pelo `npc_da_skill` (`medidor.rs:1118-1146`);
  o relatório usa o mesmo nome.
- Mob que bateu: o autor do 0x3804 é o id da entidade; o código do NPC vem do spawn 0x3641
  (`npc_de`, `medidor.rs:313`) e o nome, o level e o retrato vêm do questlog. Cobertura nos dumps
  (**conferido**, `cobertura_mob.py`): o autor tinha spawn com código em 1.534 de 1.623 golpes de
  monstro em jogador, mas o world boss sozinho tem 1.473 de 1.486. Sem ele, 61 de 137; no dump de
  2026-10-06 (Gartua), 0 de 32, porque os mobs nasceram antes de a captura começar. Sem spawn, o
  relatório mostra "Monstro #id" nos golpes do meio e, no golpe final, o nome pelo código do 0x8D04.
- Matador jogador (PvP): o nome vem no 0x8D04 (92 de 92 abates por jogador trouxeram nome e
  servidor). Nenhuma captura tem morte de jogador por jogador. O dano do golpe final em PvP fica de
  fora desta versão (ver "Fica de fora").

**Q4. Usuário não reconhecido (sem 0x3633).** Hoje `meu_id` vem do 0x3633 (`sessao.rs:197-202`)
ou do seu nome guardado aparecendo num abate ou invocação (`medidor.rs:439-445`); sem isso,
`voce_reconhecido` fica falso. Sem `meu_id` não há "sua" morte.

Proposta para a F2:
- O buffer guarda por jogador conhecido, não só por `meu_id`. Se você for reconhecido depois (pelo
  nome num abate), a próxima morte já sai com relatório, e a de agora também, se os golpes ainda
  estiverem na janela.
- Sem você reconhecido, nenhum card. Na aba Tank, a dica da sua linha não muda; a tela de
  configurações, no item do relatório, mostra "O relatório precisa saber quem é você: abra o Axon
  antes de entrar no mundo".

Achado para um item separado (fora da F2), **conferido** em `bit0.py`, `reconhecer.py` e
`hp_tempo.py`:
- Só um id por conexão recebe 0x8D00 com bit 0, e ele é o id do 0x3633 nos 6 dumps com login. Nos 5
  sem login, é o #11174 e o #7301 que o PROTOCOLO §7b já liga ao personagem do dono, e os ids
  #8446 e #10735 (sem outra prova). Nenhum dos 7.264 pacotes HP de ids do 0x3645 tem bit 0.
- O bit 0 chegou entre 0,1 e 12,2 s depois do começo do dump em 9 dos 11 (98,3 s e 61,3 s nos
  outros dois), e antes do primeiro golpe de monstro em você nos 5 dumps em que você apanhou.
- Observação: no world boss e no Gartua, o 0x3633 chegou aos 155 s com o mesmo id que recebia bit 0
  desde 7,9 s e 7,2 s, sem buraco de heartbeat maior que 0,1 s; no teleporte de 2026-10-05, aos 56 s
  com o id que recebia bit 0 desde 0,3 s. Isso contradiz "o 0x3633 só chega no login"
  (`README.md:398-402`). Causa desconhecida; o programa que gerou os dumps junta todos os fluxos do
  servidor (`scratchpad\despejar.rs`), então "mesma conexão" é **hipótese**.
- Risco: as capturas são, ao que tudo indica, sem grupo. Membros do grupo podem receber bit 0 (MP,
  por exemplo). Precisa da captura em grupo antes de virar regra. Como muda o "(você)", o "Só o meu
  dano" e o alvo do app inteiro, é item próprio, não parte da F2.

**Q5. Escopo.** Recomendação: só a sua morte nesta versão.
- O pedido fala da sua morte.
- Sem o pacote do grupo (`0x9702`, `README.md:415-416`), "outros" seria todo jogador por perto: 115
  atingidos no world boss.
- A estrutura (buffer por jogador conhecido) já serve para o grupo depois: quando o 0x9702 for lido,
  o relatório dos membros é a mesma função com outro id.
- Os outros continuam com o "☠N" na aba Tank, como hoje.

**Q6. Onde mostrar e por quanto tempo.**
- Card "☠ Você morreu" no medidor, abaixo do card do alvo e acima das abas, por `card_morte_s`
  (padrão 15 s) ou até o ✕. Mostra o golpe final em uma linha e o HP antes; o clique abre o relatório.
- Tela do relatório (`Tela::Morte`), com "‹ Medidor" para voltar, como a tela Lutas.
- Sua linha expandida na aba Tank: "☠ Morte às 14:32:05 ›" para cada morte da luta.
- Tela Lutas: "☠" na linha da luta com morte sua; abrir a luta mostra o card parado e o link.
- Barra compacta: "☠ 4.926 <mob>" pelo mesmo tempo do card (pergunta 7).
- Duração: o relatório vive junto da luta (até sair das 20 guardadas, `medidor.rs:163`); o card, pelo
  tempo configurado.

#### Modelo de dados

```rust
// medidor.rs (núcleo)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoRecebido { Golpe, Periodico, Cura, Efeito }

/// Um evento recebido por um jogador conhecido, guardado até JANELA_MORTE_MAX.
#[derive(Clone, Copy)]
struct Recebido { hora: Hora, autor: u32, skill: u32, valor: u64, tipo: TipoRecebido,
                  critico: bool, aparo: bool, costas: bool }

/// Uma linha do relatório, do mais velho ao golpe final. Nomes já resolvidos na hora da morte.
#[derive(Clone, Debug)]
pub struct LinhaMorte { pub antes_s: f64, pub quem: String, pub retrato: Option<PathBuf>,
                        pub skill: u32, pub nome_skill: String, pub icone: Option<PathBuf>,
                        pub valor: Option<u64>, pub tipo: TipoRecebido, pub critico: bool,
                        pub aparo: bool, pub hp_depois: Option<u64>, pub golpe_final: bool }

#[derive(Clone, Debug)]
pub struct RelatorioMorte {
    pub numero: u64,              // sobe a cada morte: o card sabe se é nova
    pub hora: Hora,
    pub morto: u32,
    pub matador: u32,
    pub matador_jogador: bool,
    pub nome_matador: String,     // mob: questlog; jogador: o do 0x8D04 ("Ocultar nomes" troca)
    pub classe_matador: &'static str,
    pub npc_matador: u32,         // código do 0x8D04 ou do spawn; 0 se jogador ou desconhecido
    pub retrato_matador: Option<PathBuf>,
    pub skill_final: u32,
    pub nome_skill_final: String,
    pub dano_final: Option<u64>,  // None: o golpe não casou no buffer
    pub hp_antes_final: Option<u64>,
    pub linhas: Vec<LinhaMorte>,
    pub recebido: u64, pub maior: u64, pub monstros: usize, pub curado: u64,
}
```

Estado novo no `Medidor` (`medidor.rs:262-346`):
- `recebidos: HashMap<u32, VecDeque<Recebido>>`: por jogador conhecido, até 30 s e 128 eventos.
- `hp_jogador: HashMap<u32, VecDeque<(Hora, u64)>>`: HP do 0x8D00 de jogador, na mesma janela.
- `mortes_da_luta: Vec<Arc<RelatorioMorte>>` e `ultima_morte: Option<Arc<RelatorioMorte>>`.
- `pub janela_morte: i64` (ticks), lida da config como a `inatividade` (`janela.rs:223`, `373`).
- `Placar` ganha `mortes: Vec<Arc<RelatorioMorte>>` (`medidor.rs:137-150`), para a luta passada levar
  junto (`LutaPassada`, `153-160`). Em `Arc` porque `obter_placar` é clonado a cada leitura do overlay.

#### Onde encaixa no código

| Arquivo | Função | Mudança |
|---|---|---|
| `crates/nucleo/src/protocolo/combate.rs` | nova `atributos(pacote)` | Lê o 0x8D00 pelo layout de bits; devolve entidade e HP (chave 0 da lista u64) quando vier |
| `combate.rs:334-365` | `morte` | `Morte` ganha `npc_matador`: depois da legião, pula o u16 e lê o varint |
| `crates/nucleo/src/medicao/sessao.rs:220-224` | `ao_pacote`, ramo `HP_RESTANTE` | Também chama `combate::atributos` e `medidor.registrar_hp_jogador` |
| `crates/nucleo/src/medicao/medidor.rs:597-656` | `registrar` | Logo depois de calcular `autor_jogador` e `alvo_jogador` (`613-615`) e **antes** de `iniciar_ou_continuar_luta` (que devolve falso com a luta fechada), empurra o `Recebido` quando o alvo é jogador conhecido e não é invocação. Tipo pela mesma separação do `registrar` (`617-640`): autor jogador (depois de `resolver_autor`) com skill de cura = Cura, com valor; autor jogador sem cura, inclusive outro jogador e golpe direto = Efeito, sem valor; autor que não é jogador = Golpe ou Periodico, com valor. Assim o efeito 2011101 do #16201 não vira dano, e o golpe de jogador (PvP) fica fora do dano (ver Riscos) |
| `medidor.rs:524-535` | nova `registrar_hp_jogador` | Guarda o HP do jogador conhecido na janela; `registrar_hp` continua só para mob |
| `medidor.rs:662-692` | `registrar_morte` | Monta o relatório quando `entidade == meu_id`, **antes** do `return` da linha 688; casa o golpe final (regra da Q2); guarda em `ultima_morte` e, com luta aberta, em `mortes_da_luta` |
| `medidor.rs:794-825` | `encerrar_luta` | Leva `mortes_da_luta` para o placar guardado e limpa a lista; o buffer não é limpo (a janela de tempo cuida dele) |
| `medidor.rs:924-943` | `nova_conexao` | Limpa `recebidos` e `hp_jogador` (ids mudam); `ultima_morte` fica (já tem os nomes resolvidos) |
| `medidor.rs:945-999` | `obter_placar` | Copia `mortes_da_luta` (os `Arc`) para `Placar::mortes` |
| `crates/overlay/src/janela.rs:80-85` | `Tela` | Nova `Tela::Morte(u64)` |
| `janela.rs:458-489` | `conteudo` | Card da morte depois do `barra_do_alvo` (`477-480`) |
| `janela.rs:302-347` | `ler_placar` | Lê `ultima_morte()` do medidor; `ocultar_nomes` (`317-319`) também troca o nome do matador jogador |
| `janela.rs:723-766` | `linhas` (Tank, sua linha expandida) | Linha "☠ Morte às ..." por relatório |
| novo `crates/overlay/src/janela/morte.rs` | `tela_morte`, `card_morte` | Desenho do relatório e do card |
| `crates/overlay/src/janela/lutas.rs:17-45`, `88-130` | `ResumoLuta`, `linha_luta` | Conta das suas mortes e "☠" na linha |
| `crates/overlay/src/config.rs:20-107` | `Config` | `relatorio_morte`, `card_morte_s`, `janela_morte_s` |
| `PROTOCOLO.md` §5 e §3, `README.md` | | Layout do 0x8D00, campos novos do 0x8D04, pendência do Tank, o relatório na lista de funções |

#### Riscos

- **Memória.** Buffer por jogador conhecido com janela de 30 s e teto de 128 eventos; faxina por
  tempo a cada evento do mesmo jogador e, a cada 30 s, remoção dos jogadores sem evento na janela.
  Pior caso medido no world boss: ~85 KB (Q1). Relatórios: um por morte sua, com no máximo 128
  linhas, guardados só com as 20 lutas.
- **Desempenho no world boss.** O custo novo por golpe é um `push_back` e um `pop_front` amortizado;
  no pico de 89 golpes/s em jogadores é desprezível perto do que `registrar` já faz em `dano_em`. O
  relatório é montado só na morte (1 morte de jogador em 183 s de world boss). `obter_placar` clona
  só os `Arc`.
- **Golpe final errado.** A regra do casamento foi medida em mortes de mob (93 casos) e numa morte
  de jogador só. Mitigação: o relatório mostra matador e skill do 0x8D04 mesmo sem o dano, e o teste
  com pacote real cobre o caso visto.
- **Cura x dano.** Efeito de jogador em você (HoT, poção) chega pelo mesmo opcode do DoT. Sem a regra
  do `registrar`, ele apareceria como dano recebido. O teste 3 cobre.
- **Classificação no world boss, além da morte limpa, não conferida.** Dos 1.486 golpes de não-jogador
  em jogador do world boss, 542 têm código na faixa de skill de jogador (`cobertura_mob.py`,
  `faixa_jogador.py`): ~295 são DoT do boss com código de classe (269 da skill 1873xxxx, o caso do
  Círculo de Proteção do PROTOCOLO §4), que entram como Periodico com valor, mas com nome de skill de
  jogador debaixo do boss; 241 são golpes diretos de invocações de jogador (1699xxxx), que entram como
  efeito sem valor. A morte do #16201 não tem nenhum desses; uma morte num world boss pode mostrar
  linhas com nome estranho até isso ser conferido.
- **Morte com a luta fechada.** `registrar_morte` hoje sai cedo (`medidor.rs:688`); o relatório vem
  antes. A morte fica em `ultima_morte` mesmo sem luta aberta (o card aparece), mas só entra numa luta
  guardada se a luta estava aberta.
- **Id reaproveitado.** Ids mudam a cada conexão (PROTOCOLO §7b): `nova_conexao` limpa o buffer.
- **Relógio.** As horas relativas ("-4,5 s") vêm da hora da captura; a hora absoluta do card usa o
  relógio do PC, como a tela Lutas.
- **Privacidade.** O nome de matador jogador fica só na memória e respeita "Ocultar nomes". Se a F3
  gravar lutas em disco e as lutas levarem os relatórios, esse nome vai junto: entra em "Dados em
  disco" para o OK do dono.

#### Plano de verificação

Testes com pacotes reais (todos em `crates/nucleo/tests`, pela API pública de `combate::*` e do
`Medidor`, como os testes de hoje; `ao_pacote` é privada). Rodar com
`CARGO_TARGET_DIR=target/agente-F2`.

1. **Parser do 0x8D00** (`parsers.rs`), com pacotes do seu personagem (#371, dump do Gartua;
   `fixture_hp.py`), sem nome:
   - bits 2: `13008df3020201001c29000000000000` → entidade 371, HP 10.524;
   - bits 1: `0f008df302010104dc050000` → entidade 371, sem HP;
   - bits 3: `1e008df302030201450c000006d83b050001006612000000000000` → entidade 371, HP 4.710;
   - o pacote de mob do teste `hp_de_mob` (`parsers.rs:34-36`) continua dando HP 105.038.
2. **Parser do 0x8D04**: `1f048dc97e82d91200a7aa01000000000000a9c19201000000000102` → morto 16201,
   skill 1235330, matador 21799, servidor 0, sem nome, `npc_matador` 2400425; o pacote do teste
   `morte_de_mob_traz_skill_matador_e_nome` (`parsers.rs:80-87`) dá `npc_matador` 0.
3. **Relatório com a morte real** (`medidor.rs` de testes), os 19 pacotes de `fixture.py`, de -4,65 s
   a 0 s, na ordem, cada um com a sua hora relativa:

   ```
   -4.650 0x8d00 13008dc97e0201008613000000000000
   -4.451 0x3804 210438c97e0400a7aa0164d912005e021beb5c0701000000904ed2120100
   -4.451 0x8d21 0a218dc97e0001
   -4.451 0x8d00 13008dc97e020100340a000000000000
   -4.201 0x3804 200438c97e0400c97e9040150148024b384d6c01000000a0519a0a0100
   -4.201 0x8d00 13008dc97e0201004e0f000000000000
   -3.501 0x3805 190538c97e0bc97ed3095fb2fc0bcf02cd02ddaf1e00
   -3.501 0x3805 170538c97e0bc97ed30960b2fc0b7e7bddaf1e00
   -3.501 0x8d00 13008dc97e0201001611000000000000
   -1.502 0x3805 180538c97e0bc97ed3095fb2fc0b02cd02ddaf1e00
   -1.502 0x3805 170538c97e0bc97ed30960b2fc0b037bddaf1e00
   -1.502 0x8d00 13008dc97e020100de12000000000000
   -1.450 0x8d21 0a218dc97e0000
   -0.451 0x8d00 13008dc97e020100f212000000000000
   -0.040 0x3804 210438c97e0400a7aa0182d912005f02d3f65c0701000000904ebe260100
   -0.040 0x8d21 0a218dc97e0001
   -0.040 0x8d00 13008dc97e0201000000000000000000
   +0.000 0x8d04 1f048dc97e82d91200a7aa01000000000000a9c19201000000000102
   +0.000 0x8d21 0a218dc97e0000
   ```

   Preparo: `definir_jogador(16201, "<nome falso>", 0, true)`, `registrar_npc(21799, 2400425)` e o
   `npc_de_teste` (`tests/medidor.rs:572`). O 0x3645 do #16201 (tem nome) e o spawn de 1.789 bytes do
   boss ficam fora. Verifica: golpe final 4.926, skill 1235330, matador 21799, NPC 2400425, HP antes
   4.850, excesso 76; linhas: 2 golpes (2.386 e 4.926), 1 cura de 1.306, 4 efeitos sem valor; HP
   depois 2.612, 3.918, 4.374, 4.830 e 0; recebido 7.312.
4. **Casos de borda** (sem pacote real, como os testes de hoje): morte com a luta já fechada gera
   relatório; golpe depois do 0x8D04 fica de fora; invocação morta não gera relatório; "Ocultar nomes"
   troca o nome do matador jogador; morte em PvP sai com matador e skill do 0x8D04, `dano_final` None
   e os golpes do jogador como efeito sem valor; `nova_conexao` limpa o buffer; mais de 128 golpes em
   30 s mantém só os 128 mais novos.
5. **Red check.** Escrever os testes 1 a 4 antes e rodar: precisam falhar (a API não existe). Depois
   de implementar e passar, quebrar de propósito e rodar de novo: (a) empurrar todo evento como Golpe
   → o teste 3 falha no "4 efeitos sem valor" e no "recebido 7.312"; (b) montar o relatório depois do
   `iniciar_ou_continuar_luta` → o teste 4 da luta fechada falha; (c) ler o HP com `hp_restante` →
   o teste 1 de bits 3 falha. Desfazer cada quebra.
6. **Visual com replay**, sem o jogo: `cargo run -p overlay -- --replay captura-2026-10-03-boss.pcapng
   --lutas --voce 16201 --ate <s>`. Três cuidados:
   - o `--voce <id>` é opção nova, só de debug. Ela não pode só chamar `definir_jogador` na abertura:
     quando o replay trava no fluxo, `trocar_para` chama `nova_conexao` (`sessao.rs:142-149`), que zera
     `meu_id` e os jogadores conhecidos (`medidor.rs:924-943`). O id forçado fica num campo de debug do
     `Medidor` que `nova_conexao` reaplica;
   - sem `--lutas`, o replay soma a captura inteira numa luta só, e o critério 5 (relatório pela tela
     Lutas) não dá para conferir;
   - os 110,0 s da morte contam do primeiro pacote do servidor no dump, não do começo do pcapng: o
     valor do `--ate` precisa de folga (ou de uma conferida rápida no replay de console).

   Confere o card, a tela do relatório e a tela Lutas, com print.
7. **Captura nova, com print da tela do jogo** (nada disso está nos dumps):
   - a sua morte para um mob, sozinho: confere o 0x8D04 com você como morto, o 0x8D00 de bits 1 e 3
     até o HP 0, e o que o jogo mostra na tela de morte (nome do matador, nome do golpe) para comparar;
   - a sua morte em PvP, se der: o nome do matador jogador no 0x8D04;
   - uma captura em grupo: o 0x9702 e se membros recebem 0x8D00 com bit 0 (decide o item da Q4 e a
     versão com grupo);
   - morte sem matador (queda, ambiente), se existir: o que vem no 0x8D04.

#### Tamanho

**M.** Parser do 0x8D00 e campo novo do 0x8D04: P. Buffer, relatório e casamento do golpe final no
`Medidor`: P a M. Card, tela do relatório, Tank e Lutas no overlay: M. Testes com os pacotes reais: P.

#### Dependências

- **F4 (Configurações):** a página "Luta" recebe os três campos (`docs/0.14.0.md:160`).
- **F3 (Recordes e comparação):** pode usar as mortes por luta ("luta sem morte", comparação com e
  sem morte). Se a F3 gravar lutas em disco com os relatórios, o nome do matador jogador vai para o
  disco: precisa do OK do dono em "Dados em disco".
- **F1 (Barra de groggy):** disputa o mesmo espaço em volta do card do alvo; combinar a ordem (alvo,
  groggy, card da morte, abas).
- **Item separado "você pelo 0x8D00 de bit 0"** (Q4): melhora a F2 (mais mortes com relatório), mas
  não bloqueia.
- **Captura em grupo (0x9702):** bloqueia a versão com o grupo, não esta.

### URLs consultadas

Nenhuma. Tudo veio do repositório (só leitura) e dos 11 dumps em `scratchpad\dumps`.

### Scripts (em `scratchpad\refino\trabalho-F2\`)

| Script | O que mede |
|---|---|
| `comum.py` | Parsers mínimos (espelho do `combate.rs`) usados pelos outros |
| `visao.py` | Duração e contagem de opcodes por dump |
| `mortes_desconhecidos.py`, `sem_spawn.py` | Classificação dos 372 mortos |
| `corpo_morte.py`, `caso0102.py` | Campos do 0x8D04 depois da legião |
| `golpe_final.py` | Casamento do último golpe com matador e skill do 0x8D04 |
| `linha_morte.py`, `fixture.py` | Linha do tempo e pacotes da morte do #16201 |
| `hp_formato.py`, `hp_jog2.py`, `hp_quem.py`, `bit0.py`, `fixture_hp.py` | Layout do 0x8D00, HP 1:1, quem recebe cada bits |
| `reconhecer.py`, `hp_tempo.py` | Quando cada dump saberia quem é você |
| `carga.py` | Carga para o buffer |
| `cobertura_mob.py` | Golpes de mob com o spawn do autor visto |
| `faixa_jogador.py`, `golpe_1699.py` | Golpes de não-jogador com código de skill de jogador no world boss; HP depois dos golpes de invocação em jogador |
