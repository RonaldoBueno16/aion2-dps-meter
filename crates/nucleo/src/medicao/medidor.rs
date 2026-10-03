//! Soma por jogador e por skill. Cada evento traz autor, alvo e skill, então a atribuição não
//! depende da queda de HP do alvo (que mistura o dano do grupo todo). Classificação:
//! jogador → mob = dano causado; mob → jogador = dano recebido; jogador → jogador = cura, se a
//! skill for de cura (o resto, PvP e buffs, fica de fora).
//! Uma luta termina depois de `inatividade` sem eventos; o próximo zera tudo.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::dados_jogo;
use crate::protocolo::combate::EventoDano;
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
    pub maximo: f64,
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
    pub mortes: i32,
    pub segurando_aggro: i32,
    /// Maior golpe (ou cura) do jogador na luta.
    pub maximo: f64,
    pub skills: Vec<LinhaSkill>,
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

/// Uma luta vista de três lados: dano causado (DPS), dano recebido (Tank) e cura (Healer).
#[derive(Clone, Debug, Default)]
pub struct Placar {
    /// Em ticks de 100 ns.
    pub duracao: i64,
    pub dano: Tabela,
    pub dano_recebido: Tabela,
    pub cura: Tabela,
    /// Já se sabe qual id é você (login visto nesta conexão ou nome guardado casado num abate).
    pub voce_reconhecido: bool,
}

/// Mob que atacou alguém nesse intervalo conta como "segurando aggro" nesse jogador.
const JANELA_AGGRO: i64 = 8 * TICKS_POR_SEGUNDO;

#[derive(Clone, Copy, Default)]
struct SomaSkill {
    total: f64,
    golpes: i32,
    criticos: i32,
    aparos: i32,
    maximo: f64,
}

#[derive(Default)]
struct Acumulado {
    total: f64,
    golpes: i32,
    criticos: i32,
    aparos: i32,
    mortes: i32,
    skills: IndexMap<u32, SomaSkill>,
}

/// Os mapas com IndexMap mantêm a ordem de inserção: os empates do placar saem na ordem em que
/// os jogadores e skills apareceram.
pub struct Medidor {
    /// Em ticks; i64::MAX = a luta nunca termina por inatividade.
    pub inatividade: i64,

    dano: IndexMap<u32, Acumulado>,
    recebido: IndexMap<u32, Acumulado>,
    cura_candidata: IndexMap<u32, Acumulado>,
    ultimo_alvo_do_mob: HashMap<u32, (u32, Hora)>,
    inicio: Hora,
    ultimo: Hora,
    em_luta: bool,

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
            dano: IndexMap::new(),
            recebido: IndexMap::new(),
            cura_candidata: IndexMap::new(),
            ultimo_alvo_do_mob: HashMap::new(),
            inicio: 0,
            ultimo: 0,
            em_luta: false,
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
        for tabela in [&mut self.dano, &mut self.cura_candidata] {
            if let Some(a) = tabela.shift_remove(&invocacao) {
                juntar(tabela.entry(dono).or_default(), a);
            }
        }
        self.recebido.shift_remove(&invocacao);
        if let Some(prefixos) = self.prefixos_de.remove(&invocacao) {
            let destino = self.prefixos_de.entry(dono).or_default();
            for (prefixo, vezes) in prefixos {
                *destino.entry(prefixo).or_insert(0) += vezes;
            }
        }
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
            self.iniciar_ou_continuar_luta(hora);
            let autor = self.resolver_autor(e.autor_id);
            self.contar_prefixo(autor, e.skill);
            somar(&mut self.cura_candidata, autor, e.skill, e.dano as f64, &e);
        } else if autor_jogador {
            self.iniciar_ou_continuar_luta(hora);
            let autor = self.resolver_autor(e.autor_id);
            self.contar_prefixo(autor, e.skill);
            // O campo é o número que sobe na tela ao bater (golpe de 2.574 no jogo = 2.574 no
            // campo). A barra de HP do mob cai ~18,82 vezes isso (PROTOCOLO.md §6), mas o medidor
            // mostra o número do jogo.
            somar(&mut self.dano, autor, e.skill, e.dano as f64, &e);
        } else if alvo_jogador && !self.eh_invocacao(e.alvo_id) {
            self.iniciar_ou_continuar_luta(hora);
            let a = somar(&mut self.recebido, e.alvo_id, e.skill, e.dano as f64, &e);
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

        if !self.jogadores_conhecidos.contains(&entidade) || self.eh_invocacao(entidade) {
            return;
        }
        self.iniciar_ou_continuar_luta(hora);
        self.recebido.entry(entidade).or_default().mortes += 1;
    }

    fn iniciar_ou_continuar_luta(&mut self, hora: Hora) {
        if self.em_luta && hora - self.ultimo > self.inatividade {
            self.reiniciar();
        }
        if !self.em_luta {
            self.inicio = hora;
            self.em_luta = true;
        }
        self.ultimo = hora;
    }

    pub fn reiniciar(&mut self) {
        self.dano.clear();
        self.recebido.clear();
        self.cura_candidata.clear();
        self.ultimo_alvo_do_mob.clear();
        self.em_luta = false;
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
        self.meu_id = None; // a memória e o seu nome continuam: valem para a conexão nova
    }

    pub fn obter_placar(&self) -> Placar {
        let duracao = if self.em_luta { self.ultimo - self.inicio } else { 0 };
        let segundos = segundos(duracao).max(1.0);

        // Cura: só as skills classificadas como cura; o resto (PvP, buff com valor) some.
        let mut cura: IndexMap<u32, Acumulado> = IndexMap::new();
        for (&quem, a) in &self.cura_candidata {
            let mut c = Acumulado::default();
            for (&skill, &s) in a.skills.iter().filter(|(skill, _)| dados_jogo::eh_cura(**skill)) {
                c.skills.insert(skill, s);
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

        Placar {
            duracao,
            dano: self.montar_tabela(&self.dano, segundos, &segurando),
            dano_recebido: self.montar_tabela(&self.recebido, segundos, &segurando),
            cura: self.montar_tabela(&cura, segundos, &segurando),
            voce_reconhecido: self.meu_id.is_some(),
        }
    }

    fn montar_tabela(&self, tabela: &IndexMap<u32, Acumulado>, segundos: f64, segurando: &HashMap<u32, i32>) -> Tabela {
        // fold e não sum: o sum de f64 vazio dá -0,0, que sairia "-0" no placar.
        let total = tabela.values().fold(0.0, |soma, a| soma + a.total);
        let mut ordem: Vec<(&u32, &Acumulado)> = tabela.iter().collect();
        // Estável: empate fica na ordem em que o jogador apareceu.
        ordem.sort_by(|a, b| decrescente(a.1.total, b.1.total).then(b.1.mortes.cmp(&a.1.mortes)));

        let jogadores = ordem
            .into_iter()
            .map(|(&id, a)| {
                let mut skills: Vec<(&u32, &SomaSkill)> = a.skills.iter().collect();
                skills.sort_by(|x, y| decrescente(x.1.total, y.1.total));
                let skills = skills
                    .into_iter()
                    .map(|(&skill, s)| LinhaSkill {
                        skill,
                        nome: dados_jogo::nome_skill(skill),
                        icone: dados_jogo::icone_skill(skill),
                        total: s.total,
                        por_segundo: s.total / segundos,
                        porcentagem: if a.total > 0.0 { s.total / a.total } else { 0.0 },
                        golpes: s.golpes,
                        criticos: s.criticos,
                        aparos: s.aparos,
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
                    mortes: a.mortes,
                    segurando_aggro: segurando.get(&id).copied().unwrap_or(0),
                    maximo: a.skills.values().fold(0.0, |maior, s| maior.max(s.maximo)),
                    skills,
                }
            })
            .collect();
        Tabela { total, jogadores }
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

fn decrescente(a: f64, b: f64) -> Ordering {
    b.partial_cmp(&a).unwrap_or(Ordering::Equal)
}

fn juntar(destino: &mut Acumulado, a: Acumulado) {
    destino.total += a.total;
    destino.golpes += a.golpes;
    destino.criticos += a.criticos;
    destino.aparos += a.aparos;
    destino.mortes += a.mortes;
    for (skill, s) in a.skills {
        let d = destino.skills.entry(skill).or_default();
        d.total += s.total;
        d.golpes += s.golpes;
        d.criticos += s.criticos;
        d.aparos += s.aparos;
        d.maximo = d.maximo.max(s.maximo);
    }
}

fn somar<'a>(tabela: &'a mut IndexMap<u32, Acumulado>, quem: u32, skill: u32, valor: f64, e: &EventoDano) -> &'a mut Acumulado {
    let a = tabela.entry(quem).or_default();
    a.total += valor;
    a.golpes += 1;
    if e.critico {
        a.criticos += 1;
    }

    let s = a.skills.entry(dados_jogo::skill_base(skill)).or_default();
    s.total += valor;
    s.golpes += 1;
    if e.critico {
        s.criticos += 1;
    }
    s.maximo = s.maximo.max(valor);
    a
}

/// Faixa de id das skills de classe de jogador.
fn skill_de_classe(skill: u32) -> bool {
    (11_000_000..20_000_000).contains(&skill)
}
