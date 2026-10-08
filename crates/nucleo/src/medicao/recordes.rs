//! Recordes de chefe: a regra do kill que vale, a comparação com o melhor e o `recordes.json`
//! (leitura com validação item a item, mescla e texto). Sem disco: quem lê e grava é o overlay.
//! Chave (código do NPC, sua classe); só números e a classe, nenhum nome.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::medidor::{ATACANTES_DE_CHEFE, Abate};
use crate::{TICKS_POR_SEGUNDO, segundos};

pub const VERSAO: u64 = 1;
/// Passou disso, sai o de data mais antiga.
pub const RECORDES_MAX: usize = 500;
/// Arquivo maior que isso nem vai para o parser (500 recordes formatados dão ~160 KB).
pub const TAMANHO_MAX: usize = 1024 * 1024;
/// O DPS só conta com pelo menos isso de tempo ativo no chefe: o aDPS tem piso de 1 s, e um golpe
/// final isolado viraria DPS enorme.
pub const ATIVO_MINIMO_MS: u64 = 20_000;
/// O tempo só conta com o chefe pego com pelo menos isso do HP no primeiro golpe.
pub const HP_INTEIRO: f64 = 0.99;
/// As classes que o Axon mostra (o espírito do Elementalist, prefixo 10, fica de fora).
pub const CLASSES: [&str; 9] =
    ["Gladiator", "Templar", "Assassin", "Ranger", "Sorcerer", "Elementalist", "Cleric", "Chanter", "Brawler"];

/// 2026-01-01 00:00 UTC: data antes disso aparece como desconhecida.
const DATA_MINIMA: i64 = 1_767_225_600;
const TEMPO_MAX_MS: u64 = 24 * 3600 * 1000;
const NPCS: std::ops::RangeInclusive<u32> = 1_000_000..=9_999_999;

/// Por que um kill não vira recorde nenhum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recusa {
    MaisDeUmChefe(usize),
    /// O chefe já estava na tela quando o Axon abriu: sem o spawn, sem o código.
    SemCodigo,
    SemMorte,
    VoceNaoReconhecido,
    SemSeuDano,
    /// A sua classe não saiu das skills.
    SemClasse,
}

impl Recusa {
    pub fn texto(self) -> String {
        match self {
            Recusa::MaisDeUmChefe(n) => format!("{n} chefes na mesma luta"),
            Recusa::SemCodigo => "o chefe já estava na tela quando o Axon abriu (sem o código dele)".into(),
            Recusa::SemMorte => "o chefe não morreu na luta".into(),
            Recusa::VoceNaoReconhecido => "você não foi reconhecido (abra o Axon antes de entrar no mundo)".into(),
            Recusa::SemSeuDano => "você não bateu no chefe".into(),
            Recusa::SemClasse => "a sua classe não apareceu nas skills".into(),
        }
    }
}

/// Por que o DPS não conta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemDps {
    /// Tempo ativo no chefe, em ms.
    AtivoCurto(u64),
}

impl SemDps {
    pub fn texto(self) -> String {
        match self {
            SemDps::AtivoCurto(ms) => format!("só {} s batendo no chefe (o mínimo é 20 s)", ms / 1000),
        }
    }
}

/// Por que o tempo de kill não conta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SemTempo {
    /// Fração do HP no primeiro golpe visto; None sem o HP do spawn.
    Parcial(Option<f64>),
    SaiuDeCombate,
    /// Jogadores no chefe: o tempo é da multidão.
    Multidao(usize),
}

impl SemTempo {
    pub fn texto(self) -> String {
        match self {
            SemTempo::Parcial(Some(fracao)) => {
                format!("o Axon viu o chefe com {}% do HP", (fracao * 100.0).floor() as i64)
            }
            SemTempo::Parcial(None) => "sem o HP do chefe no primeiro golpe".into(),
            SemTempo::SaiuDeCombate => "o chefe saiu de combate e voltou".into(),
            SemTempo::Multidao(n) => format!("{n} jogadores no chefe"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarcaDps {
    /// Seu dano no chefe dividido pelo seu tempo ativo nele (o número da aba DPS).
    pub valor: f64,
    /// Hora da morte do chefe, em segundos Unix.
    pub data: i64,
    pub ativo_ms: u64,
    /// Jogadores que bateram no chefe.
    pub jogadores: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarcaTempo {
    /// Morte menos o primeiro golpe de jogador no chefe.
    pub ms: u64,
    pub data: i64,
    pub jogadores: u32,
    /// Seu DPS na luta do melhor tempo.
    pub dps: f64,
}

/// O melhor de um (chefe, classe). O melhor DPS e o melhor tempo podem vir de lutas diferentes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recorde {
    pub npc: u32,
    pub classe: String,
    /// Kills válidos contados.
    #[serde(default)]
    pub kills: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dps: Option<MarcaDps>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<MarcaTempo>,
}

impl Recorde {
    /// A data mais nova das duas marcas: o limite tira o de data mais antiga.
    fn data(&self) -> i64 {
        let dps = self.dps.as_ref().map(|d| d.data);
        let tempo = self.tempo.as_ref().map(|t| t.data);
        dps.max(tempo).unwrap_or(i64::MIN)
    }

    fn valido(&self) -> bool {
        let dps_ok = self.dps.as_ref().is_none_or(|d| d.valor.is_finite() && d.valor >= 0.0 && d.jogadores > 0);
        let tempo_ok = self.tempo.as_ref().is_none_or(|t| {
            (1..=TEMPO_MAX_MS).contains(&t.ms) && t.dps.is_finite() && t.dps >= 0.0 && t.jogadores > 0
        });
        NPCS.contains(&self.npc)
            && CLASSES.contains(&self.classe.as_str())
            && (self.dps.is_some() || self.tempo.is_some())
            && dps_ok
            && tempo_ok
    }

    /// Fica o melhor de cada campo: DPS maior, tempo menor (empate não troca), kills o maior.
    fn juntar(&mut self, outro: &Recorde) {
        self.kills = self.kills.max(outro.kills);
        if let Some(d) = &outro.dps
            && self.dps.as_ref().is_none_or(|meu| d.valor > meu.valor)
        {
            self.dps = Some(d.clone());
        }
        if let Some(t) = &outro.tempo
            && self.tempo.as_ref().is_none_or(|meu| t.ms < meu.ms)
        {
            self.tempo = Some(t.clone());
        }
    }
}

/// Um kill que passou nas regras comuns, com o DPS e o tempo valendo ou não.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidato {
    pub npc: u32,
    pub classe: &'static str,
    /// Hora da morte, em segundos Unix.
    pub data: i64,
    pub jogadores: u32,
    pub dps: Result<MarcaDps, SemDps>,
    pub tempo: Result<MarcaTempo, SemTempo>,
}

/// O kill contra o melhor que existia antes dele.
#[derive(Clone, Debug, PartialEq)]
pub struct Resultado {
    pub candidato: Candidato,
    pub dps_antes: Option<MarcaDps>,
    pub tempo_antes: Option<MarcaTempo>,
    pub novo_dps: bool,
    pub novo_tempo: bool,
    /// O arquivo mudou (recorde novo ou mais um kill): precisa gravar.
    pub mudou: bool,
}

/// As regras do kill que vale. Comuns: exatamente 1 chefe, com código, morto na luta, você
/// reconhecido com dano nele e com classe. DPS: 20 s ativos. Tempo: HP de 99% no primeiro golpe,
/// sem sair de combate antes da morte e menos de `ATACANTES_DE_CHEFE` jogadores.
pub fn candidato(abate: &Abate) -> Result<Candidato, Recusa> {
    if abate.chefes != 1 {
        return Err(Recusa::MaisDeUmChefe(abate.chefes));
    }
    if abate.codigo == 0 {
        return Err(Recusa::SemCodigo);
    }
    let morte = abate.morte.ok_or(Recusa::SemMorte)?;
    let voce = abate.voce.ok_or(Recusa::VoceNaoReconhecido)?;
    if voce.dano <= 0.0 {
        return Err(Recusa::SemSeuDano);
    }
    let classe = CLASSES.into_iter().find(|&c| c == voce.classe).ok_or(Recusa::SemClasse)?;

    let data = morte.div_euclid(TICKS_POR_SEGUNDO);
    let jogadores = u32::try_from(abate.jogadores).unwrap_or(u32::MAX);
    let ativo_ms = u64::try_from(voce.ativo / (TICKS_POR_SEGUNDO / 1000)).unwrap_or(0);
    let valor = voce.dano / segundos(voce.ativo).max(1.0);
    let dps = if ativo_ms >= ATIVO_MINIMO_MS {
        Ok(MarcaDps { valor, data, ativo_ms, jogadores })
    } else {
        Err(SemDps::AtivoCurto(ativo_ms))
    };
    let fracao = match (abate.hp_inicial, abate.hp_maximo) {
        (Some(hp), Some(maximo)) if maximo > 0 => Some(hp as f64 / maximo as f64),
        _ => None,
    };
    let tempo = if abate.jogadores >= ATACANTES_DE_CHEFE {
        Err(SemTempo::Multidao(abate.jogadores))
    } else if !fracao.is_some_and(|f| f >= HP_INTEIRO) {
        Err(SemTempo::Parcial(fracao))
    } else if abate.saiu_de_combate {
        Err(SemTempo::SaiuDeCombate)
    } else {
        let ms = u64::try_from((morte - abate.primeiro_golpe) / (TICKS_POR_SEGUNDO / 1000)).unwrap_or(0).max(1);
        Ok(MarcaTempo { ms, data, jogadores, dps: valor })
    };
    Ok(Candidato { npc: abate.codigo, classe, data, jogadores, dps, tempo })
}

/// Os recordes guardados, na ordem do arquivo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recordes {
    pub lista: Vec<Recorde>,
}

impl Recordes {
    pub fn melhor(&self, npc: u32, classe: &str) -> Option<&Recorde> {
        self.lista.iter().find(|r| r.npc == npc && r.classe == classe)
    }

    /// O primeiro kill válido de um (chefe, classe) vira recorde; depois, DPS maior ou tempo menor
    /// troca a marca (empate não). Kill sem DPS nem tempo valendo não muda nada.
    pub fn registrar(&mut self, c: &Candidato) -> Resultado {
        let antes = self.melhor(c.npc, c.classe);
        let (dps_antes, tempo_antes) = (antes.and_then(|r| r.dps.clone()), antes.and_then(|r| r.tempo.clone()));
        let novo_dps = c.dps.as_ref().is_ok_and(|d| dps_antes.as_ref().is_none_or(|a| d.valor > a.valor));
        let novo_tempo = c.tempo.as_ref().is_ok_and(|t| tempo_antes.as_ref().is_none_or(|a| t.ms < a.ms));
        let mudou = c.dps.is_ok() || c.tempo.is_ok();
        if mudou {
            let kill = Recorde {
                npc: c.npc,
                classe: c.classe.into(),
                kills: antes.map_or(0, |r| r.kills).saturating_add(1),
                dps: c.dps.clone().ok(),
                tempo: c.tempo.clone().ok(),
            };
            self.juntar(&kill);
        }
        Resultado { candidato: c.clone(), dps_antes, tempo_antes, novo_dps, novo_tempo, mudou }
    }

    /// O arquivo de outra instância (ou o do disco de novo): fica o melhor de cada chave. Mesclar com
    /// o mesmo arquivo não muda nada.
    pub fn mesclar(&mut self, outro: &Recordes) {
        for r in &outro.lista {
            self.juntar(r);
        }
    }

    pub fn apagar(&mut self, npc: u32, classe: &str) -> bool {
        let antes = self.lista.len();
        self.lista.retain(|r| !(r.npc == npc && r.classe == classe));
        self.lista.len() != antes
    }

    fn juntar(&mut self, r: &Recorde) {
        match self.lista.iter_mut().find(|meu| meu.npc == r.npc && meu.classe == r.classe) {
            Some(meu) => meu.juntar(r),
            None => self.lista.push(r.clone()),
        }
        while self.lista.len() > RECORDES_MAX {
            let Some(velho) = self.lista.iter().enumerate().min_by_key(|(_, r)| r.data()).map(|(i, _)| i) else {
                break;
            };
            self.lista.remove(velho);
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Leitura {
    /// `removidos`: itens inválidos que saíram; `so_leitura`: versão mais nova que esta (não grava).
    Lido { recordes: Recordes, removidos: usize, so_leitura: bool },
    /// Grande demais, texto que não é JSON ou sem a lista: o overlay move para `.corrompido`.
    Corrompido,
}

/// Item a item: um item torto sai sozinho, e não derruba os outros. Chave repetida fica com o
/// melhor de cada campo. Versão ausente ou menor migra na memória.
pub fn ler(bytes: &[u8]) -> Leitura {
    if bytes.len() > TAMANHO_MAX {
        return Leitura::Corrompido;
    }
    let Ok(texto) = std::str::from_utf8(bytes) else { return Leitura::Corrompido };
    let Ok(Value::Object(raiz)) = serde_json::from_str::<Value>(texto.trim_start_matches('\u{feff}')) else {
        return Leitura::Corrompido;
    };
    let versao = match raiz.get("versao") {
        None => 0,
        Some(v) => match v.as_u64() {
            Some(versao) => versao,
            None => return Leitura::Corrompido,
        },
    };
    let Some(Value::Array(itens)) = raiz.get("recordes") else { return Leitura::Corrompido };
    let mut recordes = Recordes::default();
    let mut removidos = 0;
    for item in itens {
        match serde_json::from_value::<Recorde>(item.clone()).ok().filter(Recorde::valido) {
            Some(r) => recordes.juntar(&r),
            None => removidos += 1,
        }
    }
    Leitura::Lido { recordes, removidos, so_leitura: versao > VERSAO }
}

/// JSON formatado, como o config.json, para dar para abrir e conferir.
pub fn texto(recordes: &Recordes) -> String {
    #[derive(Serialize)]
    struct Arquivo<'a> {
        versao: u64,
        recordes: &'a [Recorde],
    }
    serde_json::to_string_pretty(&Arquivo { versao: VERSAO, recordes: &recordes.lista }).unwrap_or_default()
}

/// A data serve só para mostrar: antes de 2026 ou no futuro (relógio do PC errado na gravação), "data
/// desconhecida".
pub fn data_valida(data: i64, agora: i64) -> bool {
    (DATA_MINIMA..=agora).contains(&data)
}
