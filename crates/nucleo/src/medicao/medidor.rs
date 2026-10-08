//! Soma por jogador e por skill. Cada evento traz autor, alvo e skill, então a atribuição não
//! depende da queda de HP do alvo (que mistura o dano do grupo todo). Classificação:
//! jogador → mob = dano causado; mob → jogador = dano recebido; jogador → jogador = cura, se a
//! skill for de cura (o resto, PvP e buffs, fica de fora).
//! Uma luta termina depois de `inatividade` sem eventos ou quando todos os mobs dela saem de
//! combate (0x8D21 ou morte); o próximo golpe zera tudo e a luta que acabou vai para o histórico.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::catalogo::InfoNpc;
use super::dados_jogo;
use crate::protocolo::combate::{Buff, ChefesDeCampo, EventoDano, TICKET_ODYLE, Ticket};
use crate::{Hora, TICKS_POR_SEGUNDO, segundos};

#[derive(Clone, Debug)]
pub struct LinhaSkill {
    pub skill: u32,
    pub nome: String,
    pub icone: Option<PathBuf>,
    pub total: f64,
    /// Total da skill dividido pela duração da luta (a mesma do DPS do jogador).
    pub por_segundo: f64,
    /// Parte da skill no total do jogador.
    pub porcentagem: f64,
    pub golpes: i32,
    pub criticos: i32,
    /// Golpes desta skill que o jogador aparou (só no dano recebido).
    pub aparos: i32,
    /// Golpes pelas costas do alvo (byte de direção do 0x3804).
    pub costas: i32,
    pub maximo: f64,
}

/// Tempo de um buff ativo no jogador durante a luta. Buffs da mesma skill (efeitos diferentes, ou
/// vindos de jogadores diferentes) contam juntos.
#[derive(Clone, Debug)]
pub struct LinhaBuff {
    /// Skill de onde o buff vem (código do buff / 10, agrupado como as skills do placar).
    pub skill: u32,
    pub nome: String,
    pub icone: Option<PathBuf>,
    /// Parte da luta com o buff ativo, de 0 a 1.
    pub fracao: f64,
}

/// `nivel_lembrado`/`poder_lembrado`: valor da memória por nome (visto numa conexão anterior), não desta.
#[derive(Clone, Debug, Default)]
pub struct LinhaJogador {
    pub id: u32,
    pub nome: String,
    pub classe: &'static str,
    pub nivel: i32,
    pub nivel_lembrado: bool,
    pub poder: i32,
    pub poder_lembrado: bool,
    pub voce: bool,
    pub total: f64,
    pub por_segundo: f64,
    pub porcentagem: f64,
    pub golpes: i32,
    pub criticos: i32,
    pub aparos: i32,
    pub costas: i32,
    pub mortes: i32,
    pub segurando_aggro: i32,
    /// Maior golpe (ou cura) do jogador na luta.
    pub maximo: f64,
    pub skills: Vec<LinhaSkill>,
    /// Buffs que o jogador recebeu na luta, do mais ativo para o menos.
    pub buffs: Vec<LinhaBuff>,
}

#[derive(Clone, Debug, Default)]
pub struct Tabela {
    pub total: f64,
    pub jogadores: Vec<LinhaJogador>,
}

impl Tabela {
    /// Só a sua linha, com total e % recalculados sobre quem ficou (opção "Só o meu dano" do
    /// overlay). Vazia enquanto você não foi reconhecido.
    pub fn so_voce(&self) -> Tabela {
        let mut jogadores: Vec<LinhaJogador> = self.jogadores.iter().filter(|j| j.voce).cloned().collect();
        let total = jogadores.iter().fold(0.0, |soma, j| soma + j.total);
        for j in &mut jogadores {
            j.porcentagem = if total > 0.0 { j.total / total } else { 0.0 };
        }
        Tabela { total, jogadores }
    }
}

/// Último level e power vistos de um nome; 0 = desconhecido. Campos com o nome do jogadores.json.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PerfilJogador {
    #[serde(rename = "Nivel", default)]
    pub nivel: i32,
    #[serde(rename = "Poder", default)]
    pub poder: i32,
}

/// O mob em destaque na luta: na guerra com chefe, o chefe; sem chefe, o último mob em que você
/// bateu; sem isso (você ainda não reconhecido), o que mais apanhou dos jogadores.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Alvo {
    pub entidade: u32,
    /// Código do NPC lido no spawn; 0 se o spawn não foi visto (mob que já estava na tela).
    pub codigo: u32,
    /// Do questlog; vazio enquanto não chegou ou se ele não tem o NPC.
    pub nome: String,
    pub nivel: i32,
    pub chefe: bool,
    pub retrato: Option<PathBuf>,
    /// Último HP do 0x8D00, como o pacote manda (outra escala que a do dano: PROTOCOLO.md §6); antes
    /// do primeiro 0x8D00, o HP do spawn.
    pub hp: Option<u64>,
    /// Do spawn (0x3641); None se ele não foi visto.
    pub hp_maximo: Option<u64>,
    /// Segundos até o HP zerar, na velocidade de queda dos últimos `JANELA_HP` (HP sobre HP, sem
    /// misturar com o dano: a relação entre os dois muda de mob para mob).
    pub derrota_em: Option<f64>,
    pub morto: bool,
    /// Hora limite para matá-lo (0x8D21), na hora do Windows; None sem prazo ou com ele morto.
    pub prazo: Option<Hora>,
    /// Dano dos jogadores nele nesta luta.
    pub dano: f64,
    /// O seu dano nele nesta luta.
    pub meu_dano: f64,
}

/// Uma luta vista de três lados: dano causado (DPS), dano recebido (Tank) e cura (Healer).
#[derive(Clone, Debug, Default)]
pub struct Placar {
    /// Em ticks de 100 ns.
    /// Na guerra com chefe, conta do primeiro golpe no chefe em diante.
    pub duracao: i64,
    pub alvo: Option<Alvo>,
    /// Guerra com chefe: a aba DPS só tem o dano no chefe (golpes em outros mobs ficam de fora).
    pub so_chefe: bool,
    pub dano: Tabela,
    pub dano_recebido: Tabela,
    pub cura: Tabela,
    /// Já se sabe qual id é você (login visto nesta conexão ou nome guardado casado num abate).
    pub voce_reconhecido: bool,
}

/// Luta que já acabou, para o histórico do overlay. Fica só na memória.
#[derive(Clone, Debug)]
pub struct LutaPassada {
    /// Sobe a cada luta guardada: identifica a luta mesmo depois que a lista anda.
    pub numero: u64,
    pub inicio: Hora,
    pub fim: Hora,
    pub placar: Arc<Placar>,
}

/// Quantas lutas o histórico guarda (as mais novas), como o medidor do TK.
pub const LUTAS_GUARDADAS: usize = 20;

/// Mob que atacou alguém nesse intervalo conta como "segurando aggro" nesse jogador.
const JANELA_AGGRO: i64 = 8 * TICKS_POR_SEGUNDO;

/// Golpe que chega até 1 s depois de o último mob sair de combate ainda é da luta que acabou: no
/// world boss de 2026-10-03, golpes atrasados chegaram até 0,39 s depois da morte.
const TOLERANCIA_FIM: i64 = TICKS_POR_SEGUNDO;

/// Mob da luta sem golpe nem entrada em combate há mais que isso não impede o fim dela.
const MOB_RECENTE: i64 = 5 * TICKS_POR_SEGUNDO;

/// Quanto de leituras de HP o "derrota em" olha, e o mínimo para estimar (antes disso, um golpe
/// grande isolado faria a estimativa pular).
const JANELA_HP: i64 = 30 * TICKS_POR_SEGUNDO;
const HP_MINIMO_PARA_ESTIMAR: i64 = 5 * TICKS_POR_SEGUNDO;

/// Mob que apanhou de tantos jogadores diferentes na luta é chefe, mesmo sem o nome (boss que já
/// estava na tela quando o Axon abriu não teve o pacote de criação). No evento de 2026-10-03, os dois
/// world bosses apanharam de 784 e 89 ids; o mob comum mais batido, de 13; nas outras capturas, até 2.
const ATACANTES_DE_CHEFE: usize = 30;

/// Prazo para matar a 1 h ou mais é descartado (o visto foi de 300 s).
const PRAZO_MAXIMO: i64 = 3600 * TICKS_POR_SEGUNDO;

/// "Entrou em combate" até esse tempo depois da morte é ignorado.
const T1_DEPOIS_DA_MORTE: i64 = 5 * TICKS_POR_SEGUNDO;
const MORTOS_MAX: usize = 4096;

/// Com mais buffs abertos que isso, os vencidos (saíram da visão sem 0x382C) são descartados.
const BUFFS_ABERTOS_MAX: usize = 4096;

/// Do primeiro ao último evento de quem está sendo medido. O "por segundo" de cada jogador (e das
/// skills dele) divide só por esse tempo (aDPS), como no Abyss: quem chega no meio da luta não tem o
/// número puxado para baixo pelo tempo em que não estava lá.
#[derive(Clone, Copy, Default)]
struct Ativo {
    de: Option<Hora>,
    ate: Hora,
}

impl Ativo {
    fn marcar(&mut self, hora: Hora) {
        self.de = Some(self.de.map_or(hora, |de| de.min(hora)));
        self.ate = self.ate.max(hora);
    }

    fn juntar(&mut self, outro: Ativo) {
        if let Some(de) = outro.de {
            self.marcar(de);
            self.ate = self.ate.max(outro.ate);
        }
    }

    /// No mínimo 1 s: um golpe só não vira DPS infinito.
    fn segundos(&self) -> f64 {
        self.de.map_or(1.0, |de| segundos(self.ate - de).max(1.0))
    }
}

#[derive(Clone, Copy, Default)]
struct SomaSkill {
    total: f64,
    golpes: i32,
    criticos: i32,
    aparos: i32,
    costas: i32,
    maximo: f64,
    ativo: Ativo,
}

#[derive(Clone, Default)]
struct Acumulado {
    total: f64,
    golpes: i32,
    criticos: i32,
    aparos: i32,
    costas: i32,
    mortes: i32,
    skills: IndexMap<u32, SomaSkill>,
    ativo: Ativo,
}

/// Buff ainda não removido, por (alvo, instância). `ate` = última renovação + duração.
struct BuffAberto {
    codigo: u32,
    inicio: Hora,
    ate: Hora,
}

struct BuffFechado {
    alvo: u32,
    codigo: u32,
    inicio: Hora,
    fim: Hora,
}

/// Os mapas com IndexMap mantêm a ordem de inserção: os empates do placar saem na ordem em que
/// os jogadores e skills apareceram.
pub struct Medidor {
    /// Em ticks; i64::MAX = a luta nunca termina por inatividade.
    pub inatividade: i64,
    /// A luta termina quando todos os mobs dela saem de combate. Desligado no replay, que soma a
    /// captura inteira numa luta só.
    pub fim_pelo_combate: bool,
    /// Energia Odyle (básica, carregada) do último ticket dela (0x610B no login, 0x610C). Não zera
    /// na troca de conexão: é do personagem, e o login manda de novo.
    pub odyle: Option<(u64, Option<u64>)>,
    /// Última lista de chefes de campo da região (0x9101) e quando chegou. Não zera na troca de
    /// conexão nem no Zerar: é do mundo, e o servidor repete a cada poucos segundos.
    pub chefes_de_campo: Option<(ChefesDeCampo, Hora)>,

    dano: IndexMap<u32, Acumulado>,
    recebido: IndexMap<u32, Acumulado>,
    cura_candidata: IndexMap<u32, Acumulado>,
    ultimo_alvo_do_mob: HashMap<u32, (u32, Hora)>,
    inicio: Hora,
    ultimo: Hora,
    em_luta: bool,

    // Fim pelo 0x8D21: os mobs da luta (alvos do dano, autores do dano recebido) com a hora do último
    // golpe ou entrada em combate, e quem saiu de combate (mob sem estado visto conta como em
    // combate). Com todos fora, a luta acaba na hora guardada em `encerrada_em`, e a próxima
    // pancada começa outra.
    mobs_da_luta: HashMap<u32, Hora>,
    fora_de_combate: HashSet<u32>,
    mortos: HashMap<u32, Hora>,
    encerrada_em: Option<Hora>,
    /// Quando o último chefe da luta morreu: a luta acaba ali, mesmo com gente batendo nos mobs em
    /// volta (o 0x8D21 sozinho não fecha, porque mob novo entra na luta).
    chefe_morto_em: Option<Hora>,

    historico: VecDeque<LutaPassada>,
    lutas_guardadas: u64,

    buffs_abertos: HashMap<(u32, u32), BuffAberto>,
    /// Só os que fecharam durante a luta atual.
    buffs_fechados: Vec<BuffFechado>,

    /// Dano de cada jogador em cada mob, na luta atual: a guerra com chefe conta só o do chefe, e
    /// a barra do alvo mostra o dano no mob dela.
    dano_em: IndexMap<u32, IndexMap<u32, Acumulado>>,
    /// Primeiro golpe de jogador em cada mob da luta: a guerra com chefe conta o tempo dali.
    primeiro_golpe_em: HashMap<u32, Hora>,
    /// Último mob em que você bateu nesta luta. O alvo selecionado no jogo não chega em nenhum
    /// pacote conhecido do servidor; o golpe é o que se sabe com certeza.
    meu_alvo: Option<u32>,
    /// Nome, level e chefe de um NPC pelo código. Padrão: o catálogo (questlog); os testes trocam.
    pub consultar_npc: fn(u32) -> Option<InfoNpc>,
    // Por id, até a conexão trocar: código do NPC (spawn) e último HP (0x8D00) de cada mob.
    npc_de: HashMap<u32, u32>,
    hp_de: HashMap<u32, u64>,
    hp_maximo_de: HashMap<u32, u64>,
    /// Leituras de HP dos últimos `JANELA_HP`, por mob, para o "derrota em".
    hp_recente: HashMap<u32, VecDeque<(Hora, u64)>>,
    /// Hora limite para matar cada mob (0x8D21 com prazo), até ele sair de combate (a morte também tira).
    prazo_de: HashMap<u32, Hora>,
    /// Skill de mob → quantos golpes cada NPC (código) deu com ela: o golpe recebido ganha o nome
    /// do mob que mais a usou (nenhuma base pública tem nome de skill de mob). Nas capturas, 1 de 29
    /// skills de mob veio de dois mobs. Vale entre conexões: o código não muda.
    npc_da_skill: HashMap<u32, HashMap<u32, u32>>,

    // Estado que sobrevive entre lutas.
    nomes: IndexMap<u32, String>,
    niveis: HashMap<u32, i32>,
    poderes: HashMap<u32, i32>,
    prefixos_de: HashMap<u32, IndexMap<u32, i32>>,
    meu_id: Option<u32>,

    // Ids que são jogadores ou invocações deles: separa "jogador → mob" de "jogador → jogador".
    jogadores_conhecidos: HashSet<u32>,

    // Invocações, pets e armadilhas agem com id próprio; o efeito vai para a linha do dono.
    dono_de: HashMap<u32, u32>,
    nome_dono_de: HashMap<u32, String>,
    // Dono lido do marcador do spawn, à espera de o dono ser jogador conhecido (`confirmar_dono`).
    dono_marcado: HashMap<u32, u32>,

    // Memória por nome, que sobrevive à troca de conexão e (pelo overlay) entre execuções: level e
    // power só chegam no login (você) ou quando o jogador entra na visão (outros). Com o overlay
    // aberto no meio da sessão, o nome que aparece num abate ou numa invocação puxa o último valor.
    memoria: IndexMap<String, PerfilJogador>,
    meu_nome: Option<String>,
}

impl Default for Medidor {
    fn default() -> Self {
        Self {
            inatividade: 15 * TICKS_POR_SEGUNDO,
            fim_pelo_combate: true,
            odyle: None,
            chefes_de_campo: None,
            dano: IndexMap::new(),
            recebido: IndexMap::new(),
            cura_candidata: IndexMap::new(),
            ultimo_alvo_do_mob: HashMap::new(),
            inicio: 0,
            ultimo: 0,
            em_luta: false,
            mobs_da_luta: HashMap::new(),
            fora_de_combate: HashSet::new(),
            mortos: HashMap::new(),
            encerrada_em: None,
            chefe_morto_em: None,
            historico: VecDeque::new(),
            lutas_guardadas: 0,
            buffs_abertos: HashMap::new(),
            buffs_fechados: Vec::new(),
            dano_em: IndexMap::new(),
            primeiro_golpe_em: HashMap::new(),
            meu_alvo: None,
            consultar_npc: npc_do_catalogo,
            npc_de: HashMap::new(),
            hp_de: HashMap::new(),
            hp_maximo_de: HashMap::new(),
            hp_recente: HashMap::new(),
            prazo_de: HashMap::new(),
            npc_da_skill: HashMap::new(),
            nomes: IndexMap::new(),
            niveis: HashMap::new(),
            poderes: HashMap::new(),
            prefixos_de: HashMap::new(),
            meu_id: None,
            jogadores_conhecidos: HashSet::new(),
            dono_de: HashMap::new(),
            nome_dono_de: HashMap::new(),
            dono_marcado: HashMap::new(),
            memoria: IndexMap::new(),
            meu_nome: None,
        }
    }
}

impl Medidor {
    /// Nível 0 = desconhecido (não apaga um nível já visto).
    pub fn definir_jogador(&mut self, id: u32, nome: &str, nivel: i32, voce: bool) {
        self.dono_marcado.remove(&id); // id reaproveitado por jogador
        if !nome.is_empty() {
            self.nomear(id, nome);
        }
        if nivel > 0 {
            self.niveis.insert(id, nivel);
        }
        if voce {
            self.meu_id = Some(id);
            if !nome.is_empty() {
                self.meu_nome = Some(nome.to_string());
            }
        }
        self.jogadores_conhecidos.insert(id);
        self.lembrar(id);
    }

    /// Power 0 = desconhecido (não apaga um valor já visto).
    pub fn definir_poder(&mut self, id: u32, poder: i32) {
        if poder <= 0 {
            return;
        }
        self.poderes.insert(id, poder);
        self.lembrar(id);
    }

    /// Carrega a memória salva (o overlay guarda em arquivo).
    pub fn carregar_memoria(&mut self, seu_nome: Option<String>, perfis: impl IntoIterator<Item = (String, PerfilJogador)>) {
        if self.meu_nome.is_none() {
            self.meu_nome = seu_nome;
        }
        for (nome, perfil) in perfis {
            self.memoria.entry(nome).or_insert(perfil);
        }
    }

    pub fn exportar_memoria(&self) -> (Option<String>, IndexMap<String, PerfilJogador>) {
        (self.meu_nome.clone(), self.memoria.clone())
    }

    /// "Apagar" nas configurações: some a memória guardada e o seu nome. O que esta conexão já mandou
    /// continua valendo; quem aparecer de novo volta a ser guardado.
    pub fn esquecer_memoria(&mut self) {
        self.memoria.clear();
        self.meu_nome = None;
    }

    fn nomear(&mut self, id: u32, nome: &str) {
        self.nomes.insert(id, nome.to_string());
        // Sem 0x3633 nesta conexão, o seu nome guardado identifica você; um 0x3633 nunca é trocado.
        if self.meu_id.is_none() && self.meu_nome.as_deref() == Some(nome) && !self.eh_invocacao(id) {
            self.meu_id = Some(id);
        }
    }

    fn lembrar(&mut self, id: u32) {
        let Some(nome) = self.nomes.get(&id) else { return };
        let antes = self.memoria.get(nome).copied().unwrap_or_default();
        let perfil = PerfilJogador {
            nivel: self.niveis.get(&id).copied().unwrap_or(antes.nivel),
            poder: self.poderes.get(&id).copied().unwrap_or(antes.poder),
        };
        self.memoria.insert(nome.clone(), perfil);
    }

    pub fn definir_invocacao(&mut self, invocacao: u32, dono: u32, nome_dono: &str) {
        self.jogadores_conhecidos.insert(invocacao);
        if dono != 0 {
            self.jogadores_conhecidos.insert(dono);
            self.dono_de.insert(invocacao, dono);
            if !nome_dono.is_empty() && !self.nomes.contains_key(&dono) {
                self.nomear(dono, nome_dono);
            }
            self.passar_para_o_dono(invocacao);
        } else if !nome_dono.is_empty() {
            self.nome_dono_de.insert(invocacao, nome_dono.to_string());
        }
    }

    /// Na chegada ao world boss de 2026-10-03, o spawn veio depois dos primeiros golpes em 88 de 92
    /// invocações (mediana 0,6 s): o que a invocação somou como linha própria vai para a do dono. Golpe
    /// levado por ela sai do Tank, como os que chegam depois do vínculo.
    fn passar_para_o_dono(&mut self, invocacao: u32) {
        let dono = self.resolver_autor(invocacao);
        if dono == invocacao {
            return;
        }
        for tabela in [&mut self.dano, &mut self.cura_candidata].into_iter().chain(self.dano_em.values_mut()) {
            if let Some(a) = tabela.shift_remove(&invocacao) {
                juntar(tabela.entry(dono).or_default(), a);
            }
        }
        self.recebido.shift_remove(&invocacao);
        // Abate da invocação ainda sem vínculo traz o nome do dono, e com o seu nome guardado ela virava "você".
        if let Some(nome) = self.nomes.shift_remove(&invocacao) {
            self.nomes.entry(dono).or_insert(nome);
        }
        if self.meu_id == Some(invocacao) {
            self.meu_id = Some(dono);
        }
        if let Some(prefixos) = self.prefixos_de.remove(&invocacao) {
            let destino = self.prefixos_de.entry(dono).or_default();
            for (prefixo, vezes) in prefixos {
                *destino.entry(prefixo).or_insert(0) += vezes;
            }
        }
    }

    /// Spawn de NPC (0x3641): o código diz nome, level e retrato pelo catálogo. Id reaproveitado é
    /// outro mob: o HP e a morte do anterior não valem para ele.
    pub fn registrar_npc(&mut self, entidade: u32, codigo: u32) {
        self.npc_de.insert(entidade, codigo);
        self.hp_de.remove(&entidade);
        self.hp_maximo_de.remove(&entidade);
        self.hp_recente.remove(&entidade);
        self.prazo_de.remove(&entidade);
        self.mortos.remove(&entidade);
    }

    /// Ticket de conteúdo do login (0x610B) ou da mudança (0x610C); por enquanto só a Odyle interessa.
    pub fn registrar_ticket(&mut self, ticket: Ticket) {
        if let (TICKET_ODYLE, Some(valor)) = (ticket.id, ticket.valor) {
            self.odyle = Some((valor, ticket.extra));
        }
    }

    /// HP do spawn: o máximo, e o atual até chegar o primeiro 0x8D00.
    pub fn registrar_hp_do_spawn(&mut self, entidade: u32, atual: u64, maximo: u64) {
        self.hp_maximo_de.insert(entidade, maximo);
        self.hp_de.insert(entidade, atual);
    }

    /// 0x8D00. O de jogador conhecido fica de fora: o HP dele não entra no alvo.
    pub fn registrar_hp(&mut self, entidade: u32, hp: u64, hora: Hora) {
        if self.jogadores_conhecidos.contains(&entidade) {
            return;
        }
        self.hp_de.insert(entidade, hp);
        let leituras = self.hp_recente.entry(entidade).or_default();
        leituras.push_back((hora, hp));
        while leituras.front().is_some_and(|&(antes, _)| hora - antes > JANELA_HP) {
            leituras.pop_front();
        }
    }

    /// Na velocidade de queda do HP nos últimos `JANELA_HP`; sem queda (ou com HP subindo), None.
    fn derrota_em(&self, entidade: u32) -> Option<f64> {
        let leituras = self.hp_recente.get(&entidade)?;
        let (&(de, hp_antes), &(ate, hp)) = (leituras.front()?, leituras.back()?);
        if ate - de < HP_MINIMO_PARA_ESTIMAR || hp >= hp_antes {
            return None;
        }
        let por_segundo = (hp_antes - hp) as f64 / segundos(ate - de);
        Some(hp as f64 / por_segundo)
    }

    /// Ids são reaproveitados depois de troca de zona: spawn que não é invocação apaga o vínculo.
    pub fn esquecer_invocacao(&mut self, entidade: u32) {
        self.dono_de.remove(&entidade);
        self.nome_dono_de.remove(&entidade);
        self.dono_marcado.remove(&entidade);
        self.jogadores_conhecidos.remove(&entidade);
    }

    /// Dono do marcador do spawn (espírito do Elementalist, armadilha). Só vira vínculo quando o dono
    /// for jogador conhecido: em outros tipos de spawn o marcador traz lixo, e um mob tratado como
    /// invocação tiraria do DPS os golpes que leva.
    pub fn marcar_dono(&mut self, invocacao: u32, dono: u32) {
        if !self.dono_de.contains_key(&invocacao) {
            self.dono_marcado.insert(invocacao, dono);
            self.confirmar_dono(invocacao);
        }
    }

    fn confirmar_dono(&mut self, entidade: u32) {
        let Some(&dono) = self.dono_marcado.get(&entidade) else { return };
        if self.jogadores_conhecidos.contains(&dono) && !self.eh_invocacao(dono) {
            self.dono_marcado.remove(&entidade);
            self.definir_invocacao(entidade, dono, "");
        }
    }

    fn resolver_autor(&self, mut autor: u32) -> u32 {
        // Invocação de invocação existe (cadeia); o limite evita laço se os ids se repetirem.
        for _ in 0..16 {
            if let Some(&dono) = self.dono_de.get(&autor) {
                autor = dono;
                continue;
            }
            if let Some(nome) = self.nome_dono_de.get(&autor) {
                let achado = self.nomes.iter().find(|(_, n)| *n == nome).map_or(0, |(id, _)| *id);
                if achado != 0 && achado != autor {
                    autor = achado;
                    continue;
                }
            }
            break;
        }
        autor
    }

    fn eh_invocacao(&self, id: u32) -> bool {
        self.dono_de.contains_key(&id) || self.nome_dono_de.contains_key(&id)
    }

    pub fn registrar(&mut self, e: EventoDano, hora: Hora) {
        if e.dano == 0 {
            return;
        }

        self.confirmar_dono(e.autor_id);
        self.confirmar_dono(e.alvo_id);

        // A skill só diz quem é o autor no golpe direto. No DoT ela pode ser de um efeito de jogador
        // com autor mob: o Círculo de Proteção do Chanter (18730002) chega como DoT do boss no jogador,
        // e com o boss tratado como jogador 90% do dano nele saiu do DPS (world boss de 2026-10-03).
        let skill_diz_o_autor = !e.periodico;
        if skill_diz_o_autor && skill_de_classe(e.skill) {
            self.jogadores_conhecidos.insert(e.autor_id);
        }

        let autor_jogador =
            self.jogadores_conhecidos.contains(&e.autor_id) || (skill_diz_o_autor && dados_jogo::eh_skill_de_jogador(e.skill));
        let alvo_jogador = self.jogadores_conhecidos.contains(&e.alvo_id);

        if autor_jogador && (alvo_jogador || dados_jogo::eh_cura(e.skill)) {
            // Jogador → jogador (inclui si mesmo): só vira cura se a skill for de cura,
            // decidido em obter_placar (a classificação pode chegar depois do evento).
            if !self.iniciar_ou_continuar_luta(hora, false) {
                return;
            }
            let autor = self.resolver_autor(e.autor_id);
            self.contar_prefixo(autor, e.skill);
            somar(&mut self.cura_candidata, autor, e.skill, e.dano as f64, &e, hora);
        } else if autor_jogador {
            self.iniciar_ou_continuar_luta(hora, true);
            self.juntar_mob(e.alvo_id, hora);
            let autor = self.resolver_autor(e.autor_id);
            self.contar_prefixo(autor, e.skill);
            somar(self.dano_em.entry(e.alvo_id).or_default(), autor, e.skill, e.dano as f64, &e, hora);
            self.primeiro_golpe_em.entry(e.alvo_id).or_insert(hora);
            if self.meu_id == Some(autor) {
                self.meu_alvo = Some(e.alvo_id);
            }
            // O campo é o número que sobe na tela ao bater (golpe de 2.574 no jogo = 2.574 no
            // campo). A barra de HP do mob cai um múltiplo disso que muda por mob (4,5 a 18,82,
            // PROTOCOLO.md §6), mas o medidor mostra o número do jogo.
            somar(&mut self.dano, autor, e.skill, e.dano as f64, &e, hora);
        } else if alvo_jogador && !self.eh_invocacao(e.alvo_id) {
            self.iniciar_ou_continuar_luta(hora, true);
            self.juntar_mob(e.autor_id, hora);
            if let Some(&codigo) = self.npc_de.get(&e.autor_id) {
                *self.npc_da_skill.entry(dados_jogo::skill_base(e.skill)).or_default().entry(codigo).or_default() += 1;
            }
            let a = somar(&mut self.recebido, e.alvo_id, e.skill, e.dano as f64, &e, hora);
            if e.aparo {
                a.aparos += 1;
                if let Some(s) = a.skills.get_mut(&dados_jogo::skill_base(e.skill)) {
                    s.aparos += 1;
                }
            }
            self.ultimo_alvo_do_mob.insert(e.autor_id, (e.alvo_id, hora));
        }
        // Mob → mob e golpe em invocação ficam de fora.
    }

    /// 0x8D04: quem morreu e quem matou. Se a skill que matou é de classe e o abate traz servidor, o
    /// matador é jogador e o nome dele vale (mob que mata também pode trazer nome; invocação traz o do
    /// dono). O servidor barra mob que mate com efeito de jogador, como no DoT: nas capturas, jogador
    /// matou com servidor 2401 em 73 de 73 abates, e o world boss com 0.
    pub fn registrar_morte(&mut self, entidade: u32, matador: u32, skill: u32, servidor: u16, nome_matador: &str, hora: Hora) {
        let matador_jogador =
            (skill_de_classe(skill) && (1000..=9999).contains(&servidor)) || self.jogadores_conhecidos.contains(&matador);
        if matador != 0 && matador_jogador && !self.eh_invocacao(matador) && !nome_matador.is_empty() {
            self.nomear(matador, nome_matador);
            self.jogadores_conhecidos.insert(matador);
        }
        // Mob morto não ataca mais ninguém; jogador morto não segura mais o aggro de ninguém.
        self.ultimo_alvo_do_mob.remove(&entidade);
        self.ultimo_alvo_do_mob.retain(|_, (alvo, _)| *alvo != entidade);

        if !self.jogadores_conhecidos.contains(&entidade) {
            // Mob morto saiu de combate, mesmo que o 0x8D21 não chegue.
            if self.mortos.len() > MORTOS_MAX {
                self.mortos.retain(|_, morte| hora - *morte < T1_DEPOIS_DA_MORTE);
            }
            self.mortos.insert(entidade, hora);
            self.registrar_estado_combate(entidade, false, hora);
            if self.fim_pelo_combate && self.em_luta && self.chefe_morto_em.is_none() {
                let chefes = self.chefes();
                if chefes.contains(&entidade) && chefes.iter().all(|c| self.mortos.contains_key(c)) {
                    self.chefe_morto_em = Some(hora);
                }
            }
            return;
        }
        if self.eh_invocacao(entidade) || !self.iniciar_ou_continuar_luta(hora, false) {
            return;
        }
        self.recebido.entry(entidade).or_default().mortes += 1;
    }

    /// 0x8D21 e morte de mob. Um mob da luta saindo de combate encerra a luta quando nenhum mob dela
    /// continua em combate (no mob, o 0 chegou no instante da morte em 73 de 84 casos). O estado de
    /// jogador não entra: o seu caía 1,5 a 3 s depois de cada golpe e quebrou em 4 pedaços a luta
    /// contínua do world boss de 2026-10-03.
    /// Prazo para matar o mob, que o 0x8D21 traz ao ele entrar em combate (visto num chefe só: 300 s
    /// depois da entrada). Comparado depois com a hora do Windows, como o renascer dos chefes de
    /// campo: o relógio do PC precisa estar certo. Mob morto, prazo vencido ou a mais de
    /// `PRAZO_MAXIMO` ficam de fora.
    pub fn registrar_prazo(&mut self, entidade: u32, prazo_ms: u64, hora: Hora) {
        if self.mortos.contains_key(&entidade) {
            return;
        }
        let Some(prazo) = i64::try_from(prazo_ms).ok().and_then(|ms| ms.checked_mul(TICKS_POR_SEGUNDO / 1000)) else {
            return;
        };
        if prazo > hora && prazo - hora < PRAZO_MAXIMO {
            self.prazo_de.insert(entidade, prazo);
        }
    }

    pub fn registrar_estado_combate(&mut self, entidade: u32, em_combate: bool, hora: Hora) {
        if self.jogadores_conhecidos.contains(&entidade) {
            return;
        }
        if em_combate {
            // O world boss de 2026-10-03 voltou a "em combate" 0,04 s depois de morrer.
            if self.mortos.get(&entidade).is_some_and(|&morte| hora - morte < T1_DEPOIS_DA_MORTE) {
                return;
            }
            self.fora_de_combate.remove(&entidade);
            // Mob da luta que voltou ao combate (largou e foi puxado de novo): a luta continua.
            if let Some(visto) = self.mobs_da_luta.get_mut(&entidade) {
                *visto = hora;
                self.encerrada_em = None;
            }
            return;
        }
        self.fora_de_combate.insert(entidade);
        self.prazo_de.remove(&entidade);
        if !self.fim_pelo_combate
            || !self.em_luta
            || self.encerrada_em.is_some()
            || !self.mobs_da_luta.contains_key(&entidade)
        {
            return;
        }
        // Mob sem golpe nem entrada em combate há MOB_RECENTE não segura a luta: no world boss, 19
        // mobs entraram em combate e saíram da visão sem mandar o 0.
        let todos_fora = self
            .mobs_da_luta
            .iter()
            .filter(|&(_, &visto)| hora - visto <= MOB_RECENTE)
            .all(|(mob, _)| self.fora_de_combate.contains(mob));
        if todos_fora {
            self.encerrada_em = Some(hora);
        }
    }

    /// Mob ainda em combate que entra na luta (até 1 s depois de o último sair) desfaz o fim: é a
    /// próxima leva puxada em seguida. Golpe atrasado no mob que acabou de morrer não desfaz.
    fn juntar_mob(&mut self, mob: u32, hora: Hora) {
        self.mobs_da_luta.insert(mob, hora);
        if !self.fora_de_combate.contains(&mob) {
            self.encerrada_em = None;
        }
    }

    /// `abre`: o evento pode começar uma luta (golpe entre jogador e mob). Cura e morte de jogador
    /// depois que os mobs saíram de combate não abrem luta: o grupo se cura depois que o último mob
    /// morre, e isso trocaria o placar da luta que acabou por uma luta só de cura. Devolve se o
    /// evento entra na luta.
    fn iniciar_ou_continuar_luta(&mut self, hora: Hora, abre: bool) -> bool {
        if self.em_luta {
            let saiu_de_combate =
                self.encerrada_em.into_iter().chain(self.chefe_morto_em).any(|fim| hora - fim > TOLERANCIA_FIM);
            if saiu_de_combate && !abre {
                return false;
            }
            if saiu_de_combate || hora - self.ultimo > self.inatividade {
                self.encerrar_luta();
            }
        }
        if !self.em_luta {
            self.inicio = hora;
            self.em_luta = true;
            // O que fechou ou venceu antes desta luta não conta nela.
            self.buffs_fechados.clear();
            self.buffs_abertos.retain(|_, b| b.ate >= hora);
        }
        self.ultimo = hora;
        true
    }

    /// "Zerar" do overlay e conexão nova: a luta atual acaba (e vai para o histórico).
    pub fn reiniciar(&mut self) {
        self.encerrar_luta();
    }

    /// Todo fim de luta passa por aqui (inatividade, saída de combate, "Zerar", conexão nova): a luta
    /// com algum número vai para o histórico e o placar zera.
    fn encerrar_luta(&mut self) {
        if self.em_luta {
            let placar = self.obter_placar();
            let vazia = placar.dano.jogadores.is_empty()
                && placar.dano_recebido.jogadores.is_empty()
                && placar.cura.jogadores.is_empty();
            if !vazia {
                self.lutas_guardadas += 1;
                let luta = LutaPassada {
                    numero: self.lutas_guardadas,
                    inicio: self.inicio,
                    fim: self.ultimo,
                    placar: Arc::new(placar),
                };
                self.historico.push_front(luta);
                self.historico.truncate(LUTAS_GUARDADAS);
            }
        }
        self.dano.clear();
        self.recebido.clear();
        self.cura_candidata.clear();
        self.ultimo_alvo_do_mob.clear();
        self.mobs_da_luta.clear();
        self.hp_recente.clear();
        self.encerrada_em = None;
        self.chefe_morto_em = None;
        self.buffs_fechados.clear();
        self.dano_em.clear();
        self.primeiro_golpe_em.clear();
        self.meu_alvo = None;
        self.em_luta = false;
    }

    /// As lutas que já acabaram, da mais nova para a mais velha (até LUTAS_GUARDADAS).
    pub fn lutas_passadas(&self) -> &VecDeque<LutaPassada> {
        &self.historico
    }

    /// 0x382A e 0x382B. Fica de fora o buff permanente (0xFFFFFFFF), o instantâneo (duração 0) e o
    /// código que não vem de skill de classe (200, 231, 10002...: efeitos do sistema, hipótese).
    pub fn registrar_buff(&mut self, b: Buff, hora: Hora) {
        if b.duracao_ms == u32::MAX || b.duracao_ms == 0 || !skill_de_classe(b.codigo / 10) {
            return;
        }
        let ate = hora + i64::from(b.duracao_ms) * TICKS_POR_SEGUNDO / 1000;
        let chave = (b.alvo, b.instancia);
        match self.buffs_abertos.get_mut(&chave) {
            Some(aberto) if aberto.codigo == b.codigo => aberto.ate = ate,
            // Instância que trocou de código (216 de 40.937 renovações): é outro buff.
            _ => {
                self.fechar_buff(chave, hora);
                self.buffs_abertos.insert(chave, BuffAberto { codigo: b.codigo, inicio: hora, ate });
            }
        }
        if self.buffs_abertos.len() > BUFFS_ABERTOS_MAX {
            self.buffs_abertos.retain(|_, b| b.ate >= hora);
        }
    }

    /// 0x382C: o buff saiu antes do fim (consumido, cancelado ou trocado) ou junto com ele.
    pub fn remover_buffs(&mut self, alvo: u32, instancias: &[u32], hora: Hora) {
        for &instancia in instancias {
            self.fechar_buff((alvo, instancia), hora);
        }
    }

    fn fechar_buff(&mut self, chave: (u32, u32), hora: Hora) {
        let Some(b) = self.buffs_abertos.remove(&chave) else {
            return;
        };
        // Fora de luta o intervalo não serve para nada: a próxima luta começa depois dele.
        if self.em_luta {
            self.buffs_fechados.push(BuffFechado {
                alvo: chave.0,
                codigo: b.codigo,
                inicio: b.inicio,
                fim: hora.min(b.ate),
            });
        }
    }

    /// Tempo de cada buff ativo por jogador (alvo), dentro da luta. Intervalos da mesma skill se
    /// juntam: dois Chanters dando o mesmo buff não passam de 100%.
    fn buffs_por_jogador(&self) -> HashMap<u32, Vec<LinhaBuff>> {
        let (inicio, fim) = (self.inicio, self.ultimo);
        if !self.em_luta || fim <= inicio {
            return HashMap::new();
        }
        let abertos = self.buffs_abertos.iter().map(|(&(alvo, _), b)| (alvo, b.codigo, b.inicio, b.ate));
        let fechados = self.buffs_fechados.iter().map(|b| (b.alvo, b.codigo, b.inicio, b.fim));
        let mut intervalos: HashMap<(u32, u32), Vec<(Hora, Hora)>> = HashMap::new();
        for (alvo, codigo, de, ate) in abertos.chain(fechados) {
            let (de, ate) = (de.max(inicio), ate.min(fim));
            if de < ate {
                intervalos.entry((alvo, dados_jogo::skill_base(codigo / 10))).or_default().push((de, ate));
            }
        }

        let mut por_jogador: HashMap<u32, Vec<LinhaBuff>> = HashMap::new();
        for ((alvo, skill), mut lista) in intervalos {
            lista.sort_unstable();
            let (mut ativo, mut aberto): (i64, Option<(Hora, Hora)>) = (0, None);
            for (de, ate) in lista {
                aberto = match aberto {
                    Some((a, b)) if de <= b => Some((a, b.max(ate))),
                    Some((a, b)) => {
                        ativo += b - a;
                        Some((de, ate))
                    }
                    None => Some((de, ate)),
                };
            }
            ativo += aberto.map_or(0, |(a, b)| b - a);
            let fracao = ativo as f64 / (fim - inicio) as f64;
            let buff =
                LinhaBuff { skill, nome: dados_jogo::nome_skill(skill), icone: dados_jogo::icone_skill(skill), fracao };
            por_jogador.entry(alvo).or_default().push(buff);
        }
        for buffs in por_jogador.values_mut() {
            buffs.sort_by(|a, b| decrescente(a.fracao, b.fracao));
        }
        por_jogador
    }

    fn contar_prefixo(&mut self, jogador: u32, skill: u32) {
        *self.prefixos_de.entry(jogador).or_default().entry(skill / 1_000_000).or_insert(0) += 1;
    }

    /// Conexão nova com o servidor (login, troca de servidor ou canal): os ids de entidade mudam
    /// (o mesmo personagem foi #11174, #7301 e #11179), então nada indexado por id continua valendo.
    pub fn nova_conexao(&mut self) {
        self.reiniciar();
        self.nomes.clear();
        self.niveis.clear();
        self.poderes.clear();
        self.prefixos_de.clear();
        self.jogadores_conhecidos.clear();
        self.dono_de.clear();
        self.nome_dono_de.clear();
        self.dono_marcado.clear();
        self.fora_de_combate.clear();
        self.mortos.clear();
        self.buffs_abertos.clear();
        self.npc_de.clear();
        self.hp_de.clear();
        self.hp_maximo_de.clear();
        self.hp_recente.clear();
        self.prazo_de.clear();
        self.meu_id = None; // a memória, o seu nome e o histórico continuam: valem para a conexão nova
    }

    pub fn obter_placar(&self) -> Placar {
        let mut duracao = if self.em_luta { self.ultimo - self.inicio } else { 0 };

        // Guerra com chefe: o DPS é só o dano no chefe (num evento, os golpes nos mobs em volta
        // inflariam o placar), e o relógio da luta conta do primeiro golpe nele.
        let chefes = self.chefes();
        let mut so_no_chefe: IndexMap<u32, Acumulado> = IndexMap::new();
        for chefe in &chefes {
            for (&quem, a) in &self.dano_em[chefe] {
                juntar(so_no_chefe.entry(quem).or_default(), a.clone());
            }
        }
        let inicio_chefe = chefes.iter().filter_map(|c| self.primeiro_golpe_em.get(c)).min();
        if let Some(&inicio) = inicio_chefe.filter(|_| self.em_luta) {
            duracao = self.ultimo - inicio;
        }

        // Cura: só as skills classificadas como cura; o resto (PvP, buff com valor) some.
        let mut cura: IndexMap<u32, Acumulado> = IndexMap::new();
        for (&quem, a) in &self.cura_candidata {
            let mut c = Acumulado::default();
            for (&skill, &s) in a.skills.iter().filter(|(skill, _)| dados_jogo::eh_cura(**skill)) {
                c.skills.insert(skill, s);
                c.ativo.juntar(s.ativo);
                c.total += s.total;
                c.golpes += s.golpes;
                c.criticos += s.criticos;
            }
            if c.golpes > 0 {
                cura.insert(quem, c);
            }
        }

        let mut segurando: HashMap<u32, i32> = HashMap::new();
        for &(alvo, hora) in self.ultimo_alvo_do_mob.values() {
            if self.ultimo - hora <= JANELA_AGGRO {
                *segurando.entry(alvo).or_insert(0) += 1;
            }
        }

        let buffs = self.buffs_por_jogador();
        Placar {
            duracao,
            alvo: self.alvo(&chefes),
            so_chefe: !chefes.is_empty(),
            dano: if chefes.is_empty() {
                self.montar_tabela(&self.dano, &segurando, &buffs)
            } else {
                self.montar_tabela(&so_no_chefe, &segurando, &buffs)
            },
            dano_recebido: self.montar_tabela(&self.recebido, &segurando, &buffs),
            cura: self.montar_tabela(&cura, &segurando, &buffs),
            voce_reconhecido: self.meu_id.is_some(),
        }
    }

    /// Mobs desta luta que são chefe: nomeado ou herói no questlog, ou batido por uma multidão.
    fn chefes(&self) -> Vec<u32> {
        self.dano_em
            .iter()
            .filter(|&(&mob, jogadores)| {
                jogadores.len() >= ATACANTES_DE_CHEFE || self.npc(mob).is_some_and(|n| n.chefe())
            })
            .map(|(&mob, _)| mob)
            .collect()
    }

    fn npc(&self, mob: u32) -> Option<InfoNpc> {
        (self.consultar_npc)(*self.npc_de.get(&mob)?)
    }

    fn dano_no_mob(&self, mob: u32) -> f64 {
        self.dano_em.get(&mob).map_or(0.0, |jogadores| jogadores.values().fold(0.0, |soma, a| soma + a.total))
    }

    /// Na guerra com chefe, o chefe (o que mais apanhou, se houver mais de um), mesmo que você bata em
    /// outro mob; sem chefe, o último mob em que você bateu; sem isso, o mob que mais apanhou.
    fn alvo(&self, chefes: &[u32]) -> Option<Alvo> {
        let mais_batido = |mobs: Vec<u32>| {
            mobs.into_iter().rev().max_by(|&a, &b| self.dano_no_mob(a).total_cmp(&self.dano_no_mob(b)))
        };
        let entidade = mais_batido(chefes.to_vec())
            .or(self.meu_alvo.filter(|mob| self.dano_em.contains_key(mob)))
            .or_else(|| mais_batido(self.dano_em.keys().copied().collect()))?;
        let npc = self.npc(entidade);
        let retrato = npc.as_ref().and_then(|n| dados_jogo::catalogo()?.caminho_icone(n.retrato.as_deref()));
        let meu_dano = self.meu_id.and_then(|eu| self.dano_em.get(&entidade)?.get(&eu)).map_or(0.0, |a| a.total);
        Some(Alvo {
            entidade,
            codigo: self.npc_de.get(&entidade).copied().unwrap_or(0),
            nome: npc.as_ref().map(|n| n.nome.clone()).unwrap_or_default(),
            nivel: npc.as_ref().map_or(0, |n| n.nivel),
            chefe: chefes.contains(&entidade),
            retrato,
            hp: self.hp_de.get(&entidade).copied(),
            hp_maximo: self.hp_maximo_de.get(&entidade).copied(),
            derrota_em: self.derrota_em(entidade).filter(|_| !self.mortos.contains_key(&entidade)),
            morto: self.mortos.contains_key(&entidade),
            prazo: self.prazo_de.get(&entidade).copied(),
            dano: self.dano_no_mob(entidade),
            meu_dano,
        })
    }

    fn montar_tabela(
        &self,
        tabela: &IndexMap<u32, Acumulado>,
        segurando: &HashMap<u32, i32>,
        buffs: &HashMap<u32, Vec<LinhaBuff>>,
    ) -> Tabela {
        // fold e não sum: o sum de f64 vazio dá -0,0, que sairia "-0" no placar.
        let total = tabela.values().fold(0.0, |soma, a| soma + a.total);
        let mut ordem: Vec<(&u32, &Acumulado)> = tabela.iter().collect();
        // Estável: empate fica na ordem em que o jogador apareceu.
        ordem.sort_by(|a, b| decrescente(a.1.total, b.1.total).then(b.1.mortes.cmp(&a.1.mortes)));

        let jogadores = ordem
            .into_iter()
            .map(|(&id, a)| {
                let segundos = a.ativo.segundos();
                let mut skills: Vec<(&u32, &SomaSkill)> = a.skills.iter().collect();
                skills.sort_by(|x, y| decrescente(x.1.total, y.1.total));
                let skills = skills
                    .into_iter()
                    .map(|(&skill, s)| LinhaSkill {
                        skill,
                        nome: self.nome_skill(skill),
                        icone: self.icone_skill(skill),
                        total: s.total,
                        por_segundo: s.total / segundos,
                        porcentagem: if a.total > 0.0 { s.total / a.total } else { 0.0 },
                        golpes: s.golpes,
                        criticos: s.criticos,
                        aparos: s.aparos,
                        costas: s.costas,
                        maximo: s.maximo,
                    })
                    .collect();

                let nome = self.nomes.get(&id);
                let lembrado = nome.and_then(|n| self.memoria.get(n)).copied().unwrap_or_default();
                let nivel_lembrado = !self.niveis.contains_key(&id) && lembrado.nivel > 0;
                let poder_lembrado = !self.poderes.contains_key(&id) && lembrado.poder > 0;
                LinhaJogador {
                    id,
                    nome: nome.cloned().unwrap_or_else(|| format!("#{id}")),
                    classe: self.classe(id),
                    nivel: if nivel_lembrado { lembrado.nivel } else { self.niveis.get(&id).copied().unwrap_or(0) },
                    nivel_lembrado,
                    poder: if poder_lembrado { lembrado.poder } else { self.poderes.get(&id).copied().unwrap_or(0) },
                    poder_lembrado,
                    voce: Some(id) == self.meu_id,
                    total: a.total,
                    por_segundo: a.total / segundos,
                    porcentagem: if total > 0.0 { a.total / total } else { 0.0 },
                    golpes: a.golpes,
                    criticos: a.criticos,
                    aparos: a.aparos,
                    costas: a.costas,
                    mortes: a.mortes,
                    segurando_aggro: segurando.get(&id).copied().unwrap_or(0),
                    maximo: a.skills.values().fold(0.0, |maior, s| maior.max(s.maximo)),
                    skills,
                    buffs: buffs.get(&id).cloned().unwrap_or_default(),
                }
            })
            .collect();
        Tabela { total, jogadores }
    }

    /// Skill de mob: "Golpe <código> · <mob>" quando se sabe quem a usou ("e outros" se mais de um
    /// mob a usou). O código vem antes: é ele que separa os golpes do mesmo mob, e o nome comprido é
    /// o que o corte com "…" encurta.
    fn nome_skill(&self, skill: u32) -> String {
        let Some(npc) = self.npc_que_usou(skill) else {
            return dados_jogo::nome_skill(skill);
        };
        let outros = if self.npc_da_skill.get(&skill).is_some_and(|mobs| mobs.len() > 1) { " e outros" } else { "" };
        format!("Golpe {skill} · {}{outros}", npc.nome)
    }

    /// Skill de mob: o retrato do mob, se o questlog tiver.
    fn icone_skill(&self, skill: u32) -> Option<PathBuf> {
        if dados_jogo::eh_skill_de_jogador(skill) {
            return dados_jogo::icone_skill(skill);
        }
        let retrato = self.npc_que_usou(skill)?.retrato;
        dados_jogo::catalogo()?.caminho_icone(retrato.as_deref())
    }

    fn npc_que_usou(&self, skill: u32) -> Option<InfoNpc> {
        if dados_jogo::eh_skill_de_jogador(skill) {
            return None;
        }
        // Empate fica com o menor código: o nome não troca de uma leitura para outra.
        let (&codigo, _) = self
            .npc_da_skill
            .get(&skill)?
            .iter()
            .max_by_key(|&(&codigo, &golpes)| (golpes, std::cmp::Reverse(codigo)))?;
        (self.consultar_npc)(codigo)
    }

    fn classe(&self, jogador: u32) -> &'static str {
        let Some(p) = self.prefixos_de.get(&jogador) else { return "" };
        // Prefixo 10 é o espírito do Elementalist: só vale como classe se não houver outra.
        // Empate fica com o prefixo visto primeiro.
        let chave = |prefixo: u32, vezes: i32| ((11..20).contains(&prefixo), vezes);
        let mut melhor: Option<(u32, i32)> = None;
        for (&prefixo, &vezes) in p {
            if melhor.is_none_or(|(mp, mv)| chave(prefixo, vezes) > chave(mp, mv)) {
                melhor = Some((prefixo, vezes));
            }
        }
        dados_jogo::classe(melhor.map_or(0, |m| m.0) * 1_000_000)
    }
}

fn npc_do_catalogo(codigo: u32) -> Option<InfoNpc> {
    dados_jogo::catalogo()?.npc(codigo)
}

fn decrescente(a: f64, b: f64) -> Ordering {
    b.partial_cmp(&a).unwrap_or(Ordering::Equal)
}

fn juntar(destino: &mut Acumulado, a: Acumulado) {
    destino.total += a.total;
    destino.golpes += a.golpes;
    destino.criticos += a.criticos;
    destino.aparos += a.aparos;
    destino.costas += a.costas;
    destino.mortes += a.mortes;
    destino.ativo.juntar(a.ativo);
    for (skill, s) in a.skills {
        let d = destino.skills.entry(skill).or_default();
        d.ativo.juntar(s.ativo);
        d.total += s.total;
        d.golpes += s.golpes;
        d.criticos += s.criticos;
        d.aparos += s.aparos;
        d.costas += s.costas;
        d.maximo = d.maximo.max(s.maximo);
    }
}

fn somar<'a>(
    tabela: &'a mut IndexMap<u32, Acumulado>,
    quem: u32,
    skill: u32,
    valor: f64,
    e: &EventoDano,
    hora: Hora,
) -> &'a mut Acumulado {
    let a = tabela.entry(quem).or_default();
    a.ativo.marcar(hora);
    a.total += valor;
    a.golpes += 1;
    if e.critico {
        a.criticos += 1;
    }
    if e.costas {
        a.costas += 1;
    }

    let s = a.skills.entry(dados_jogo::skill_base(skill)).or_default();
    s.ativo.marcar(hora);
    s.total += valor;
    s.golpes += 1;
    if e.critico {
        s.criticos += 1;
    }
    if e.costas {
        s.costas += 1;
    }
    s.maximo = s.maximo.max(valor);
    a
}

/// Faixa de id das skills de classe de jogador.
fn skill_de_classe(skill: u32) -> bool {
    (11_000_000..20_000_000).contains(&skill)
}
