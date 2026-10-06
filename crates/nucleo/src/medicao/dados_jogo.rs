//! Classificação de skills e nomes. Nome e ícone em português vêm do catálogo; o skills.json
//! em inglês (opcional) é só reserva para o que o catálogo não tiver.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::catalogo::{Busca, CatalogoSkills, DropsNpc, InfoRegiao};

// Curas que chegam pelo mesmo opcode do dano e não podem entrar na soma.
const CURAS: [u32; 8] = [18120000, 18170000, 16770000, 16190000, 17120000, 17800000, 17100000, 17410000];

static NOMES: OnceLock<HashMap<String, String>> = OnceLock::new();
static CATALOGO: OnceLock<CatalogoSkills> = OnceLock::new();

/// Liga o catálogo (uma vez por processo; as chamadas seguintes são ignoradas).
pub fn definir_catalogo(catalogo: CatalogoSkills) -> &'static CatalogoSkills {
    CATALOGO.get_or_init(|| catalogo)
}

pub fn catalogo() -> Option<&'static CatalogoSkills> {
    CATALOGO.get()
}

pub fn carregar_nomes(caminho: &Path) {
    let Ok(texto) = std::fs::read_to_string(caminho) else { return };
    if let Ok(nomes) = serde_json::from_str::<HashMap<String, String>>(&texto) {
        let _ = NOMES.set(nomes);
    }
}

/// Procura dados/skills.json a partir da pasta do executável, subindo até a raiz.
pub fn carregar_nomes_padrao() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    for dir in exe.ancestors().skip(1) {
        let caminho = dir.join("dados").join("skills.json");
        if caminho.is_file() {
            carregar_nomes(&caminho);
            return Some(caminho);
        }
    }
    None
}

pub fn nomes_carregados() -> usize {
    NOMES.get().map_or(0, HashMap::len)
}

/// Skill de jogador (8 dígitos, prefixo 10 a 19), pedra Theo (3.0xx.xxx) ou pet (1xx.xxx).
/// Fica de fora skill de NPC (1.000.000 a 9.999.999 fora da faixa Theo).
pub fn eh_skill_de_jogador(skill: u32) -> bool {
    (10_000_000..20_000_000).contains(&skill) || (3_000_000..3_100_000).contains(&skill) || (100_000..200_000).contains(&skill)
}

/// Lista fixa (curas conhecidas do RATmeter) ou marcador de cura no catálogo pt-BR.
pub fn eh_cura(skill: u32) -> bool {
    CURAS.contains(&skill)
        || CURAS.contains(&skill_base(skill))
        || (eh_skill_de_jogador(skill) && catalogo().and_then(|c| c.eh_cura(skill_base(skill))) == Some(true))
}

/// Os 2 primeiros dígitos do código da skill dizem a classe (8 dígitos: 14340000 = Ranger).
/// Nomes de classe ficam em inglês por escolha do usuário (2026-10-02), mesmo com o cliente em português.
pub fn classe(skill: u32) -> &'static str {
    if !(10_000_000..20_000_000).contains(&skill) {
        return "";
    }
    match skill / 1_000_000 {
        10 => "Spirit",
        11 => "Gladiator",
        12 => "Templar",
        13 => "Assassin",
        14 => "Ranger",
        15 => "Sorcerer",
        16 => "Elementalist",
        17 => "Cleric",
        18 => "Chanter",
        19 => "Brawler",
        _ => "",
    }
}

/// Skill de jogador: português do catálogo; senão inglês exato ou da skill base; senão o código.
/// Skill de monstro: o catálogo pt-BR não tem, então "Golpe de monstro (código)" em vez de inglês.
pub fn nome_skill(skill: u32) -> String {
    if !eh_skill_de_jogador(skill) {
        return format!("Golpe de monstro ({skill})");
    }
    if let Some(info) = catalogo().and_then(|c| c.obter(skill_base(skill))) {
        return info.nome;
    }
    let ingles = NOMES.get().and_then(|n| n.get(&skill.to_string()).or_else(|| n.get(&(skill / 10_000 * 10_000).to_string())));
    ingles.cloned().unwrap_or_else(|| format!("Skill {skill}"))
}

/// Caminho local do ícone (PNG), ou None enquanto não baixou.
pub fn icone_skill(skill: u32) -> Option<PathBuf> {
    let c = catalogo()?;
    if !eh_skill_de_jogador(skill) {
        return None;
    }
    c.caminho_icone(c.obter(skill_base(skill))?.icone.as_deref())
}

/// Emblema oficial da classe (`UT_Class_*_Large` na CDN do jogo, o mesmo do questlog e do medidor
/// Abyss), em branco e dourado. None enquanto não baixou, ou com a classe desconhecida.
pub fn emblema_classe(classe: &str) -> Option<PathBuf> {
    let nome = match classe {
        "Gladiator" | "Templar" | "Assassin" | "Ranger" | "Sorcerer" | "Elementalist" | "Cleric" | "Chanter" => classe,
        "Spirit" => "Elementalist",
        "Brawler" => "Fighter",
        _ => return None,
    };
    catalogo()?.caminho_icone(Some(&format!("UT_Class_{nome}_Large")))
}

/// Ícone do item Energia Odyle na CDN do jogo (o mesmo do medidor Abyss). None enquanto não baixou.
pub fn icone_odyle() -> Option<PathBuf> {
    catalogo()?.caminho_icone(Some("Icon_Item_Odenergy_A_001"))
}

/// Nome e chefes de campo da região do 0x9101. None enquanto não chegou do questlog.
pub fn regiao(codigo: u32) -> Option<InfoRegiao> {
    catalogo()?.regiao(codigo)
}

/// Drops do chefe de campo e o que vem nos baús dele, pelo questlog.
pub fn drops(codigo: u32) -> Busca<DropsNpc> {
    catalogo().map_or(Busca::Falhou, |c| c.drops(codigo))
}

/// PNG da CDN pelo nome, só na memória (retratos dos chefes de campo e ícones dos drops).
pub fn imagem(nome: &str) -> Busca<std::sync::Arc<Vec<u8>>> {
    catalogo().map_or(Busca::Falhou, |c| c.imagem(nome))
}

/// Deixa pedir de novo os drops e as imagens que falharam.
pub fn repetir_falhas() {
    if let Some(c) = catalogo() {
        c.repetir_falhas();
    }
}

/// Ícone da CDN do jogo pelo nome (os dos eventos do overlay). None enquanto não baixou.
pub fn icone_do_jogo(nome: &str) -> Option<PathBuf> {
    catalogo()?.caminho_icone(Some(nome))
}

/// Agrupa variantes da mesma skill (14030010, 14030020... viram 14030000).
pub fn skill_base(skill: u32) -> u32 {
    if (10_000_000..20_000_000).contains(&skill) { skill / 10_000 * 10_000 } else { skill }
}
