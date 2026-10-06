//! Nome em português e ícone de cada skill, nome, level e retrato de cada NPC (o alvo da luta), os
//! chefes de campo de cada região e os drops de um chefe, buscados sob demanda e guardados em disco.
//! Nomes: questlog.gg, base comunitária montada a partir do cliente Global (idioma "pt"),
//! API não documentada: pode mudar sem aviso. Ícones: CDN oficial da NCSoft.
//! Uma requisição por vez, com intervalo, para não sobrecarregar ninguém.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `cura`: None enquanto o detalhe da skill não foi consultado (a listagem não traz).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Info {
    #[serde(rename = "Nome")]
    pub nome: String,
    #[serde(rename = "Icone", default)]
    pub icone: Option<String>,
    #[serde(rename = "Cura", default)]
    pub cura: Option<bool>,
}

/// NPC do questlog pelo código do spawn (0x3641). `retrato`: só alguns têm (o world boss tem, mob
/// comum quase nunca).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InfoNpc {
    #[serde(rename = "Nome")]
    pub nome: String,
    #[serde(rename = "Nivel", default)]
    pub nivel: i32,
    /// isNamed do questlog: chefe com nome próprio.
    #[serde(rename = "Nomeado", default)]
    pub nomeado: bool,
    /// npcSubType do questlog: "normalmonster", "heromonster", "normalsummon"...
    #[serde(rename = "Tipo", default)]
    pub tipo: String,
    #[serde(rename = "Retrato", default)]
    pub retrato: Option<String>,
}

impl InfoNpc {
    /// Chefe: nomeado, herói ou lendário no questlog. Na amostra de 2026-10-05, world boss e chefe de
    /// dungeon (Kromede, Bakarma) vieram heromonster nomeados, legendmonster também nomeado;
    /// elitemonster e normalmonster, não. Invocação (normalsummon) vem nomeada às vezes e fica de fora.
    pub fn chefe(&self) -> bool {
        !self.tipo.contains("summon")
            && (self.nomeado || ["hero", "legend", "boss"].iter().any(|t| self.tipo.contains(t)))
    }
}

/// Chefe de campo de uma região do questlog.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChefeRegiao {
    #[serde(rename = "Codigo")]
    pub codigo: u32,
    #[serde(rename = "Nome")]
    pub nome: String,
    #[serde(rename = "Nivel", default)]
    pub nivel: i32,
    #[serde(rename = "Retrato", default)]
    pub retrato: Option<String>,
}

/// Região do questlog (getRegion): o nome e os NPCs nomeados dela, em ordem de código. Em Altgard
/// (1110) são os 24 chefes de campo, na ordem dos ids do 0x9101 (111001 a 111024): conferido no
/// Gartua Imortal (21º, pelo timer da tela) e no Profanador Newbold e no Arconte Axios (12º e 13º,
/// pela posição do 0x3641 deles).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InfoRegiao {
    #[serde(rename = "Nome")]
    pub nome: String,
    #[serde(rename = "Chefes")]
    pub chefes: Vec<ChefeRegiao>,
}

/// Item que um NPC deixa cair, ou que vem num baú de saque, como o questlog dá.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemDrop {
    #[serde(rename = "Codigo")]
    pub codigo: u32,
    #[serde(rename = "Nome")]
    pub nome: String,
    #[serde(rename = "Icone", default)]
    pub icone: Option<String>,
    /// "grade" do questlog: 11 comum, 21 raro, 31 épico, 41 lendário, 51 mítico, 61 único, 71 especial.
    #[serde(rename = "Raridade", default)]
    pub raridade: u8,
    /// mainCategory do questlog (armor, weapon, accessory, misc, usable, pantheon...).
    #[serde(rename = "Categoria", default)]
    pub categoria: String,
    /// subCategory do questlog (gloves, sword, rewardbox...).
    #[serde(rename = "Tipo", default)]
    pub tipo: String,
    /// De 0 a 1; None quando o questlog não dá.
    #[serde(rename = "Chance", default)]
    pub chance: Option<f64>,
    /// Mínimo e máximo por vez; None quando o questlog não dá (os itens de baú).
    #[serde(rename = "Quantidade", default)]
    pub quantidade: Option<(u32, u32)>,
}

/// Drops de um NPC pelo questlog: os itens (npcDropsItems do getNpc) e o que vem em cada baú de saque
/// entre eles (itemContainsItems do getItem), pelo código do baú.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DropsNpc {
    #[serde(rename = "Itens")]
    pub itens: Vec<ItemDrop>,
    #[serde(rename = "Baus", default)]
    pub baus: BTreeMap<u32, Vec<ItemDrop>>,
}

/// Resposta do questlog que pode demorar ou não vir.
#[derive(Clone, Debug, PartialEq)]
pub enum Busca<T> {
    Pronto(T),
    Buscando,
    /// Sem resposta (rede ou formato): só pede de novo depois de `repetir_drops`.
    Falhou,
}

const API: &str = "https://questlog.gg/aion-2/api/trpc/database.";
const CDN: &str = "https://assets.playnccdn.com/static-aion2-gamedata/resources/";
const IDIOMA: &str = "pt";
const CLASSES: [&str; 9] =
    ["gladiator", "templar", "assassin", "ranger", "sorcerer", "elementalist", "cleric", "chanter", "brawler"];
const INTERVALO_ENTRE_REQUISICOES: Duration = Duration::from_millis(400);

struct Estado {
    pasta: PathBuf,
    arquivo_nomes: PathBuf,
    infos: Mutex<HashMap<u32, Info>>,
    arquivo_npcs: PathBuf,
    npcs: Mutex<HashMap<u32, InfoNpc>>,
    arquivo_regioes: PathBuf,
    regioes: Mutex<HashMap<u32, InfoRegiao>>,
    arquivo_drops: PathBuf,
    drops: Mutex<HashMap<u32, DropsNpc>>,
    /// Pedidos de drops que falharam nesta execução.
    falhas: Mutex<HashSet<String>>,
    ja_pedido: Mutex<HashSet<String>>,
    /// Vira true quando o cache foi lido ou a listagem inicial terminou (com ou sem rede).
    pronto: (Mutex<bool>, Condvar),
}

pub struct CatalogoSkills {
    estado: Arc<Estado>,
    fila: Sender<String>,
    /// Nome de NPC e retrato passam na frente: num world boss, centenas de skills e ícones entram na
    /// fila antes, a 400 ms cada, e o nome do boss esperaria minutos.
    urgente: Sender<String>,
}

/// %LOCALAPPDATA%\Aion2Meter, onde ficam o cache das skills, os ícones e a memória dos jogadores.
pub fn pasta_dados() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map_or_else(std::env::temp_dir, PathBuf::from).join("Aion2Meter")
}

impl CatalogoSkills {
    pub fn novo(pasta: Option<PathBuf>) -> Self {
        let pasta = pasta.unwrap_or_else(pasta_dados);
        let _ = std::fs::create_dir_all(pasta.join("icones"));
        let arquivo_nomes = pasta.join(format!("skills-{IDIOMA}.json"));

        // Cache corrompido: refaz do zero.
        let infos: HashMap<u32, Info> = std::fs::read_to_string(&arquivo_nomes)
            .ok()
            .and_then(|texto| serde_json::from_str(&texto).ok())
            .unwrap_or_default();

        let arquivo_npcs = pasta.join(format!("npcs-{IDIOMA}.json"));
        let npcs: HashMap<u32, InfoNpc> = std::fs::read_to_string(&arquivo_npcs)
            .ok()
            .and_then(|texto| serde_json::from_str(&texto).ok())
            .unwrap_or_default();

        let arquivo_regioes = pasta.join(format!("regioes-{IDIOMA}.json"));
        let regioes: HashMap<u32, InfoRegiao> = std::fs::read_to_string(&arquivo_regioes)
            .ok()
            .and_then(|texto| serde_json::from_str(&texto).ok())
            .unwrap_or_default();

        let arquivo_drops = pasta.join(format!("drops-{IDIOMA}.json"));
        let drops: HashMap<u32, DropsNpc> = std::fs::read_to_string(&arquivo_drops)
            .ok()
            .and_then(|texto| serde_json::from_str(&texto).ok())
            .unwrap_or_default();

        let estado = Arc::new(Estado {
            pasta,
            arquivo_nomes,
            infos: Mutex::new(infos),
            arquivo_npcs,
            npcs: Mutex::new(npcs),
            arquivo_regioes,
            regioes: Mutex::new(regioes),
            arquivo_drops,
            drops: Mutex::new(drops),
            falhas: Mutex::new(HashSet::new()),
            ja_pedido: Mutex::new(HashSet::new()),
            pronto: (Mutex::new(false), Condvar::new()),
        });
        let (fila, recebidos) = mpsc::channel();
        let (urgente, urgentes) = mpsc::channel();
        let trabalhador = estado.clone();
        let _ = std::thread::Builder::new()
            .name("catalogo".into())
            .spawn(move || trabalhar(&trabalhador, &urgentes, &recebidos));
        Self { estado, fila, urgente }
    }

    /// Espera o cache ou a listagem inicial, no máximo `limite`.
    pub fn esperar_pronto(&self, limite: Duration) {
        let (trava, sinal) = &self.estado.pronto;
        let guarda = trava.lock().unwrap_or_else(|e| e.into_inner());
        let _ = sinal.wait_timeout_while(guarda, limite, |pronto| !*pronto);
    }

    pub fn quantidade(&self) -> usize {
        self.estado.infos().len()
    }

    /// Nome e ícone da skill base; None enquanto não chegou (pede a busca uma vez).
    pub fn obter(&self, skill_base: u32) -> Option<Info> {
        if let Some(info) = self.estado.infos().get(&skill_base) {
            return Some(info.clone());
        }
        self.pedir(format!("skill:{skill_base}"));
        None
    }

    /// Se a skill recupera PV: a descrição de cura usa o marcador SkillUIHPHeal (a de dano usa
    /// SkillUIMinDmgSum). None enquanto não consultou; pede o detalhe uma vez.
    pub fn eh_cura(&self, skill_base: u32) -> Option<bool> {
        if let Some(cura) = self.estado.infos().get(&skill_base).and_then(|i| i.cura) {
            return Some(cura);
        }
        self.pedir(format!("skill:{skill_base}"));
        None
    }

    /// Nome, level e retrato do NPC; None enquanto não chegou ou se o questlog não tem (pede uma vez).
    pub fn npc(&self, codigo: u32) -> Option<InfoNpc> {
        if let Some(info) = self.estado.npcs().get(&codigo) {
            return Some(info.clone());
        }
        self.pedir(format!("npc:{codigo}"));
        None
    }

    /// Nome e chefes de campo da região; None enquanto não chegou ou se o questlog não tem (pede uma vez).
    pub fn regiao(&self, codigo: u32) -> Option<InfoRegiao> {
        if let Some(info) = self.estado.regioes().get(&codigo) {
            return Some(info.clone());
        }
        self.pedir(format!("regiao:{codigo}"));
        None
    }

    /// Drops do NPC e o que vem nos baús dele (pede uma vez; com falha, só depois de `repetir_drops`).
    pub fn drops(&self, codigo: u32) -> Busca<DropsNpc> {
        if let Some(drops) = self.estado.drops().get(&codigo) {
            return Busca::Pronto(drops.clone());
        }
        let item = format!("drops:{codigo}");
        if self.estado.falhas().contains(&item) {
            return Busca::Falhou;
        }
        self.pedir(item);
        Busca::Buscando
    }

    /// Depois de uma falha, deixa o próximo `drops` pedir de novo.
    pub fn repetir_drops(&self, codigo: u32) {
        let item = format!("drops:{codigo}");
        if self.estado.falhas().remove(&item) {
            self.estado.ja_pedido.lock().unwrap_or_else(|e| e.into_inner()).remove(&item);
        }
    }

    /// Caminho local do PNG do ícone; None enquanto não baixou (pede o download uma vez).
    pub fn caminho_icone(&self, icone: Option<&str>) -> Option<PathBuf> {
        let icone = icone.filter(|i| !i.is_empty())?;
        let caminho = self.estado.pasta.join("icones").join(format!("{icone}.png"));
        if caminho.is_file() {
            return Some(caminho);
        }
        self.pedir(format!("icone:{icone}"));
        None
    }

    fn pedir(&self, item: String) {
        let novo = self.estado.ja_pedido.lock().unwrap_or_else(|e| e.into_inner()).insert(item.clone());
        if novo {
            // Retrato de mob e emblema de classe (UT_), ícone de item (Icon_ e icon_; os das skills são
            // ICON_), os chefes da região e os drops de um chefe não esperam as skills.
            let urgente = ["npc:", "regiao:", "drops:", "icone:UT_", "icone:Icon_", "icone:icon_"]
                .iter()
                .any(|p| item.starts_with(p));
            let _ = if urgente { self.urgente.send(item) } else { self.fila.send(item) };
        }
    }
}

impl Estado {
    fn infos(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Info>> {
        self.infos.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn npcs(&self) -> std::sync::MutexGuard<'_, HashMap<u32, InfoNpc>> {
        self.npcs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn regioes(&self) -> std::sync::MutexGuard<'_, HashMap<u32, InfoRegiao>> {
        self.regioes.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn drops(&self) -> std::sync::MutexGuard<'_, HashMap<u32, DropsNpc>> {
        self.drops.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn falhas(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.falhas.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn marcar_pronto(&self) {
        let (trava, sinal) = &self.pronto;
        *trava.lock().unwrap_or_else(|e| e.into_inner()) = true;
        sinal.notify_all();
    }
}

fn trabalhar(estado: &Estado, urgentes: &Receiver<String>, fila: &Receiver<String>) {
    let http = cliente_http();

    // Primeira execução: uma listagem por classe cobre quase todas as skills ativas.
    if estado.infos().is_empty() {
        for classe in CLASSES {
            let _ = baixar_listagem(estado, &http, classe);
            std::thread::sleep(INTERVALO_ENTRE_REQUISICOES);
        }
        let _ = salvar(estado);
    }
    estado.marcar_pronto();

    loop {
        let item = match urgentes.try_recv() {
            Ok(item) => item,
            Err(_) => match fila.recv_timeout(Duration::from_millis(100)) {
                Ok(item) => item,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            },
        };
        // Falha de rede ou formato: a skill fica com o nome de reserva até a próxima execução.
        if let Some(codigo) = item.strip_prefix("skill:") {
            if let Ok(id) = codigo.parse::<u32>()
                && baixar_skill(estado, &http, id).is_ok()
            {
                let _ = salvar(estado);
            }
        } else if let Some(codigo) = item.strip_prefix("npc:") {
            // NPC que o questlog não tem fica sem nome até a próxima execução.
            if let Ok(id) = codigo.parse::<u32>()
                && baixar_npc(estado, &http, id).is_ok()
            {
                let _ = salvar_npcs(estado);
            }
        } else if let Some(codigo) = item.strip_prefix("regiao:") {
            if let Ok(id) = codigo.parse::<u32>()
                && baixar_regiao(estado, &http, id).is_ok()
            {
                let _ = salvar_regioes(estado);
            }
        } else if let Some(codigo) = item.strip_prefix("drops:") {
            // Falha fica marcada: o painel avisa e só pede de novo quando o usuário reabre.
            match codigo.parse::<u32>() {
                Ok(id) if baixar_drops(estado, &http, id).is_ok() => {
                    let _ = salvar_drops(estado);
                }
                _ => {
                    estado.falhas().insert(item.clone());
                }
            }
        } else if let Some(icone) = item.strip_prefix("icone:") {
            let _ = baixar_icone(estado, &http, icone);
        }
        std::thread::sleep(INTERVALO_ENTRE_REQUISICOES);
    }
}

/// Também usado pela atualização do overlay (a API do GitHub recusa pedido sem user-agent).
pub fn cliente_http() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("Aion2Meter/", env!("CARGO_PKG_VERSION"), " (medidor de DPS pessoal)"))
        // TLS do Windows (SChannel) com os certificados do sistema: sem isso o ureq procura o rustls.
        .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

type Falha = Box<dyn std::error::Error>;

fn baixar_listagem(estado: &Estado, http: &ureq::Agent, classe: &str) -> Result<(), Falha> {
    let dados = trpc(http, "getSkills", &format!(r#"{{"language":"{IDIOMA}","page":1,"mainCategory":"{classe}"}}"#))?;
    if let Some(lista) = dados.get("pageData").and_then(Value::as_array) {
        for s in lista {
            guardar(estado, s);
        }
    }
    Ok(())
}

fn baixar_skill(estado: &Estado, http: &ureq::Agent, id: u32) -> Result<(), Falha> {
    let dados = trpc(http, "getSkill", &format!(r#"{{"id":"{id}","language":"{IDIOMA}"}}"#))?;
    guardar(estado, &dados);
    Ok(())
}

fn baixar_npc(estado: &Estado, http: &ureq::Agent, id: u32) -> Result<(), Falha> {
    let dados = trpc(http, "getNpc", &format!(r#"{{"id":"{id}","language":"{IDIOMA}"}}"#))?;
    let (codigo, info) = ler_npc(&dados).ok_or("NPC sem nome")?;
    estado.npcs().insert(codigo, info);
    Ok(())
}

/// Resposta do getNpc do questlog. O retrato vem como o ícone de skill:
/// "/assets/.../UT_256_MOB_DstrArchonE_01.UT_256_MOB_DstrArchonE_01" → "UT_256_MOB_DstrArchonE_01".
pub fn ler_npc(n: &Value) -> Option<(u32, InfoNpc)> {
    let codigo = n.get("id").and_then(Value::as_str)?.parse::<u32>().ok()?;
    let nome = n.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty())?;
    let info = InfoNpc {
        nome: nome.to_string(),
        nivel: n.get("level").and_then(Value::as_i64).map_or(0, |l| l.clamp(0, 999) as i32),
        nomeado: n.get("isNamed").and_then(Value::as_bool).unwrap_or(false),
        tipo: n.get("npcSubType").and_then(Value::as_str).unwrap_or_default().to_string(),
        retrato: n.get("icon").and_then(Value::as_str).and_then(|i| i.rsplit('.').next()).map(str::to_string),
    };
    Some((codigo, info))
}

fn baixar_regiao(estado: &Estado, http: &ureq::Agent, id: u32) -> Result<(), Falha> {
    let dados = trpc(http, "getRegion", &format!(r#"{{"id":"{id}","language":"{IDIOMA}"}}"#))?;
    let (codigo, info) = ler_regiao(&dados).ok_or("região sem nome")?;
    estado.regioes().insert(codigo, info);
    Ok(())
}

/// Resposta do getRegion do questlog: o nome e os NPCs de regionHasNpcs em ordem de código (a ordem
/// em que o 0x9101 numera os chefes). Retrato como no `ler_npc`.
pub fn ler_regiao(r: &Value) -> Option<(u32, InfoRegiao)> {
    let codigo = r.get("id").and_then(Value::as_str)?.parse::<u32>().ok()?;
    let nome = r.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty())?;
    let mut chefes: Vec<ChefeRegiao> = r
        .get("regionHasNpcs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|n| {
            Some(ChefeRegiao {
                codigo: n.get("id").and_then(Value::as_str)?.parse().ok()?,
                nome: n.get("name").and_then(Value::as_str)?.to_string(),
                nivel: n.get("level").and_then(Value::as_i64).map_or(0, |l| l.clamp(0, 999) as i32),
                retrato: n.get("icon").and_then(Value::as_str).and_then(|i| i.rsplit('.').next()).map(str::to_string),
            })
        })
        .collect();
    chefes.sort_by_key(|c| c.codigo);
    Some((codigo, InfoRegiao { nome: nome.to_string(), chefes }))
}

fn baixar_drops(estado: &Estado, http: &ureq::Agent, id: u32) -> Result<(), Falha> {
    let npc = trpc(http, "getNpc", &format!(r#"{{"id":"{id}","language":"{IDIOMA}"}}"#))?;
    let itens = ler_itens(npc.get("npcDropsItems"));
    let mut baus = BTreeMap::new();
    for bau in itens.iter().filter(|i| i.tipo == "rewardbox") {
        std::thread::sleep(INTERVALO_ENTRE_REQUISICOES);
        let dados = trpc(http, "getItem", &format!(r#"{{"id":"{}","language":"{IDIOMA}"}}"#, bau.codigo))?;
        baus.insert(bau.codigo, ler_itens(dados.get("itemContainsItems")));
    }
    estado.drops().insert(id, DropsNpc { itens, baus });
    Ok(())
}

/// Lista de itens do questlog (npcDropsItems do getNpc ou itemContainsItems do getItem). O baú repete
/// cada lasca sem a chance e com ela: fica uma por código, a com chance. Ícone como no `ler_npc`.
pub fn ler_itens(lista: Option<&Value>) -> Vec<ItemDrop> {
    let mut itens: Vec<ItemDrop> = Vec::new();
    for item in lista.and_then(Value::as_array).into_iter().flatten().filter_map(ler_item) {
        match itens.iter_mut().find(|i| i.codigo == item.codigo) {
            Some(repetido) if repetido.chance.is_none() => *repetido = item,
            Some(_) => {}
            None => itens.push(item),
        }
    }
    itens
}

fn ler_item(n: &Value) -> Option<ItemDrop> {
    // O questlog manda número ou texto conforme a lista ("grade": 41 ou "41").
    let numero = |campo: &str| n.get(campo).and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()));
    let texto = |campo: &str| n.get(campo).and_then(Value::as_str).unwrap_or_default().to_string();
    let limitar = |v: u64| v.min(u64::from(u32::MAX)) as u32;
    let quantidade = match (numero("countMin"), numero("countMax")) {
        (Some(minimo), Some(maximo)) => Some((limitar(minimo), limitar(maximo))),
        _ => None,
    };
    Some(ItemDrop {
        codigo: n.get("id").and_then(Value::as_str)?.parse().ok()?,
        nome: n.get("name").and_then(Value::as_str).filter(|nome| !nome.trim().is_empty())?.to_string(),
        icone: n.get("icon").and_then(Value::as_str).and_then(|i| i.rsplit('.').next()).map(str::to_string),
        raridade: numero("grade").map_or(0, |g| g.min(255) as u8),
        categoria: texto("mainCategory"),
        tipo: texto("subCategory"),
        chance: n.get("chance").and_then(Value::as_f64).filter(|c| (0.0..=1.0).contains(c)),
        quantidade,
    })
}

fn guardar(estado: &Estado, s: &Value) {
    let Some(codigo) = s.get("id").and_then(Value::as_str).and_then(|t| t.parse::<u32>().ok()) else { return };
    let Some(nome) = s.get("name").and_then(Value::as_str).filter(|n| !n.trim().is_empty()) else { return };
    // "/assets/.../ICON_RA_SKILL_034.ICON_RA_SKILL_034" → "ICON_RA_SKILL_034"
    let icone = s.get("icon").and_then(Value::as_str).and_then(|i| i.rsplit('.').next()).map(str::to_string);
    let mut infos = estado.infos();
    let cura = match s.get("descriptionData") {
        Some(desc) => Some(desc.to_string().contains("SkillUIHPHeal")),
        None => infos.get(&codigo).and_then(|i| i.cura),
    };
    infos.insert(codigo, Info { nome: nome.to_string(), icone, cura });
}

fn baixar_icone(estado: &Estado, http: &ureq::Agent, icone: &str) -> Result<(), Falha> {
    if icone.chars().any(|c| c < ' ' || "\"<>|:*?\\/".contains(c)) {
        return Ok(());
    }
    let png = http.get(format!("{CDN}{icone}.png")).call()?.body_mut().read_to_vec()?;
    let destino = estado.pasta.join("icones").join(format!("{icone}.png"));
    escrever_trocando(&destino, &png)?;
    Ok(())
}

fn trpc(http: &ureq::Agent, procedimento: &str, entrada: &str) -> Result<Value, Falha> {
    let url = format!("{API}{procedimento}?input={}", escapar(entrada));
    let texto = http.get(url).call()?.body_mut().read_to_string()?;
    let mut doc: Value = serde_json::from_str(&texto)?;
    let dados = doc.pointer_mut("/result/data").ok_or("resposta sem result.data")?.take();
    Ok(dados)
}

/// Como o Uri.EscapeDataString: só letras, dígitos e `-._~` passam sem escape.
fn escapar(texto: &str) -> String {
    texto
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn salvar(estado: &Estado) -> std::io::Result<()> {
    let json = serde_json::to_string(&*estado.infos()).map_err(std::io::Error::other)?;
    escrever_trocando(&estado.arquivo_nomes, json.as_bytes())
}

fn salvar_npcs(estado: &Estado) -> std::io::Result<()> {
    let json = serde_json::to_string(&*estado.npcs()).map_err(std::io::Error::other)?;
    escrever_trocando(&estado.arquivo_npcs, json.as_bytes())
}

fn salvar_regioes(estado: &Estado) -> std::io::Result<()> {
    let json = serde_json::to_string(&*estado.regioes()).map_err(std::io::Error::other)?;
    escrever_trocando(&estado.arquivo_regioes, json.as_bytes())
}

fn salvar_drops(estado: &Estado) -> std::io::Result<()> {
    let json = serde_json::to_string(&*estado.drops()).map_err(std::io::Error::other)?;
    escrever_trocando(&estado.arquivo_drops, json.as_bytes())
}

/// Grava num .tmp e troca, para um arquivo pela metade nunca substituir o bom.
pub fn escrever_trocando(destino: &Path, conteudo: &[u8]) -> std::io::Result<()> {
    let mut tmp = destino.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, conteudo)?;
    std::fs::rename(&tmp, destino)
}
