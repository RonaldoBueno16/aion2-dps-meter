//! Nome em português e ícone de cada skill, buscados sob demanda e guardados em disco.
//! Nomes: questlog.gg, base comunitária montada a partir do cliente Global (idioma "pt"),
//! API não documentada: pode mudar sem aviso. Ícones: CDN oficial da NCSoft.
//! Uma requisição por vez, com intervalo, para não sobrecarregar ninguém.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
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
    ja_pedido: Mutex<HashSet<String>>,
    /// Vira true quando o cache foi lido ou a listagem inicial terminou (com ou sem rede).
    pronto: (Mutex<bool>, Condvar),
}

pub struct CatalogoSkills {
    estado: Arc<Estado>,
    fila: Sender<String>,
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

        let estado = Arc::new(Estado {
            pasta,
            arquivo_nomes,
            infos: Mutex::new(infos),
            ja_pedido: Mutex::new(HashSet::new()),
            pronto: (Mutex::new(false), Condvar::new()),
        });
        let (fila, recebidos) = mpsc::channel();
        let trabalhador = estado.clone();
        let _ = std::thread::Builder::new().name("catalogo".into()).spawn(move || trabalhar(&trabalhador, &recebidos));
        Self { estado, fila }
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
            let _ = self.fila.send(item);
        }
    }
}

impl Estado {
    fn infos(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Info>> {
        self.infos.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn marcar_pronto(&self) {
        let (trava, sinal) = &self.pronto;
        *trava.lock().unwrap_or_else(|e| e.into_inner()) = true;
        sinal.notify_all();
    }
}

fn trabalhar(estado: &Estado, fila: &Receiver<String>) {
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

    for item in fila {
        // Falha de rede ou formato: a skill fica com o nome de reserva até a próxima execução.
        if let Some(codigo) = item.strip_prefix("skill:") {
            if let Ok(id) = codigo.parse::<u32>()
                && baixar_skill(estado, &http, id).is_ok()
            {
                let _ = salvar(estado);
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
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { char::from(b).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn salvar(estado: &Estado) -> std::io::Result<()> {
    let json = serde_json::to_string(&*estado.infos()).map_err(std::io::Error::other)?;
    escrever_trocando(&estado.arquivo_nomes, json.as_bytes())
}

/// Grava num .tmp e troca, para um arquivo pela metade nunca substituir o bom.
pub fn escrever_trocando(destino: &Path, conteudo: &[u8]) -> std::io::Result<()> {
    let mut tmp = destino.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, conteudo)?;
    std::fs::rename(&tmp, destino)
}
