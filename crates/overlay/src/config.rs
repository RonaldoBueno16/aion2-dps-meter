//! Preferências do overlay, em %LOCALAPPDATA%\Aion2Meter\config.json. Campo que faltar no arquivo
//! (versão anterior) fica com o padrão; valor fora da faixa é trazido para dentro dela.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use nucleo::medicao::catalogo;
use serde::{Deserialize, Serialize};

pub const INATIVIDADE_MIN: u32 = 5;
pub const INATIVIDADE_MAX: u32 = 120;
pub const ZOOM_MIN: f32 = 0.6;
pub const ZOOM_MAX: f32 = 2.0;
pub const TRANSPARENCIA_PADRAO: u32 = 15;
/// Até 90%: com o fundo todo transparente, o overlay some sobre cenas claras do jogo.
pub const TRANSPARENCIA_MAX: u32 = 90;
/// Releitura do placar, em ms. Abaixo de 100 ms o número muda mais rápido do que dá para ler.
pub const ATUALIZACAO_MIN: u32 = 100;
pub const ATUALIZACAO_MAX: u32 = 1000;
/// Antecedência dos alertas, em minutos (0 = só na hora).
pub const ANTES_MAX_MIN: u32 = 60;
pub const CHEFES_MARCADOS_MAX: usize = 100;
/// Quanto a faixa do alerta fica no overlay, em segundos.
pub const BANNER_MIN_S: u32 = 5;
pub const BANNER_MAX_S: u32 = 120;
/// Quanto o card "Você morreu" fica no medidor, em segundos.
pub const CARD_MORTE_MIN_S: u32 = 5;
pub const CARD_MORTE_MAX_S: u32 = 60;
/// Quantos segundos antes da morte o relatório mostra (o medidor guarda até 30).
pub const JANELA_MORTE_MIN_S: u32 = 5;
pub const JANELA_MORTE_MAX_S: u32 = 30;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Total, por segundo e % em cada linha, nas três abas. Na 0.8.0 substituiu as 5 colunas por aba
    /// (colunas, colunas_tank, colunas_healer) e classe/nivel/gs: o config.json antigo continua
    /// abrindo, e esses campos são ignorados.
    pub numeros: [bool; 3],
    /// Só a sua linha, nas três abas.
    pub so_meu_dano: bool,
    /// Segundos sem dano até a próxima pancada começar uma luta nova.
    pub inatividade: u32,
    /// Escala da janela inteira (1 = 470 px de largura).
    pub zoom: f32,
    /// Transparência do fundo, em %. Texto, barras e borda não mudam.
    pub transparencia: u32,
    /// Atalhos globais ("Ctrl+H"; vazio desliga), ativos só com o jogo em primeiro plano. Os
    /// padrões vêm do medidor do TK (Aion2-Dps-Meter). Trocar só pelo arquivo, com o Axon fechado.
    pub atalho_mostrar: String,
    pub atalho_atravessar: String,
    /// Sem padrão: enquanto registrada, a combinação deixa de chegar ao jogo.
    pub atalho_resumo: String,
    pub atalho_compacta: String,
    /// Os outros jogadores aparecem pelo nome da classe; o seu continua (como no Abyss DPS Meter).
    pub ocultar_nomes: bool,
    /// De quanto em quanto o placar é relido, em ms.
    pub atualizacao_ms: u32,
    /// "Copiar resumo": um jogador por linha; desligado, tudo numa linha só.
    pub resumo_em_linhas: bool,
    /// Barra compacta: uma linha com o alvo, o seu DPS, o do grupo e o ping.
    pub compacta: bool,
    /// Eventos embaixo do rodapé: todos, um por linha; desligado, só a linha com os próximos.
    pub eventos_expandidos: bool,
    pub alertas: Alertas,
    /// Recordes de chefe no recordes.json. Desligado, o arquivo não é lido nem gravado.
    pub recordes: bool,
    /// Relatório da sua morte: o card no medidor e a tela com os últimos segundos.
    pub relatorio_morte: bool,
    pub card_morte_s: u32,
    pub janela_morte_s: u32,
}

/// Alertas de evento e de chefe de campo marcado (antes e na hora).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Alertas {
    /// Chave geral: o menu da bandeja e a página Alertas.
    pub ligados: bool,
    /// Id do evento (`eventos::EVENTOS`) → minutos antes. Id que saiu da tabela é ignorado.
    pub eventos: BTreeMap<String, u32>,
    /// Chefes de campo marcados, pelo id do 0x9101 (região × 100 + número: 111021).
    pub chefes: BTreeSet<u32>,
    /// Minutos antes do renascer, um valor para todos os chefes marcados.
    pub chefes_antes_min: u32,
    /// Também no início do evento e no renascer. Com antecedência 0 o "na hora" sai sempre.
    pub na_hora: bool,
    pub som: bool,
    /// "nunca", "sem_banner" (só quando a faixa não aparece) ou "sempre". Texto, e não enum: um valor
    /// desconhecido derrubaria a config inteira na leitura.
    pub balao: String,
    /// Segundos que a faixa fica no overlay.
    pub banner_s: u32,
    pub so_com_jogo_aberto: bool,
}

/// Quando o balão da bandeja sai.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Balao {
    Nunca,
    SemBanner,
    Sempre,
}

impl Balao {
    pub const TODOS: [Balao; 3] = [Balao::Nunca, Balao::SemBanner, Balao::Sempre];

    pub fn texto(self) -> &'static str {
        match self {
            Balao::Nunca => "nunca",
            Balao::SemBanner => "sem_banner",
            Balao::Sempre => "sempre",
        }
    }
}

impl Default for Alertas {
    fn default() -> Self {
        Self {
            ligados: true,
            eventos: BTreeMap::new(),
            chefes: BTreeSet::new(),
            chefes_antes_min: 5,
            na_hora: true,
            som: true,
            balao: Balao::SemBanner.texto().into(),
            banner_s: 20,
            so_com_jogo_aberto: false,
        }
    }
}

impl Alertas {
    pub fn balao(&self) -> Balao {
        Balao::TODOS.into_iter().find(|b| b.texto() == self.balao).unwrap_or(Balao::SemBanner)
    }

    fn dentro_das_faixas(mut self) -> Self {
        for minutos in self.eventos.values_mut() {
            *minutos = (*minutos).min(ANTES_MAX_MIN);
        }
        self.chefes = std::mem::take(&mut self.chefes).into_iter().take(CHEFES_MARCADOS_MAX).collect();
        self.chefes_antes_min = self.chefes_antes_min.min(ANTES_MAX_MIN);
        self.balao = self.balao().texto().into();
        self.banner_s = self.banner_s.clamp(BANNER_MIN_S, BANNER_MAX_S);
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            numeros: [true; 3],
            so_meu_dano: false,
            inatividade: 15,
            zoom: 1.0,
            transparencia: TRANSPARENCIA_PADRAO,
            atalho_mostrar: "Ctrl+H".into(),
            atalho_atravessar: "Ctrl+T".into(),
            atalho_resumo: String::new(),
            atalho_compacta: String::new(),
            ocultar_nomes: false,
            atualizacao_ms: 500,
            resumo_em_linhas: false,
            compacta: false,
            eventos_expandidos: false,
            alertas: Alertas::default(),
            recordes: true,
            relatorio_morte: true,
            card_morte_s: 15,
            janela_morte_s: 10,
        }
    }
}

impl Config {
    pub fn carregar() -> Self {
        let Ok(texto) = std::fs::read_to_string(arquivo()) else { return Self::default() };
        // Arquivo corrompido: volta ao padrão.
        let config: Self = serde_json::from_str(texto.trim_start_matches('\u{feff}')).unwrap_or_default();
        config.dentro_das_faixas()
    }

    pub fn salvar(&self) {
        let caminho = arquivo();
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        // Sem disco agora: a próxima mudança tenta de novo.
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = catalogo::escrever_trocando(&caminho, json.as_bytes());
        }
    }

    pub fn dentro_das_faixas(mut self) -> Self {
        self.inatividade = self.inatividade.clamp(INATIVIDADE_MIN, INATIVIDADE_MAX);
        self.zoom = if self.zoom.is_finite() { arredondar_zoom(self.zoom) } else { 1.0 };
        self.transparencia = self.transparencia.min(TRANSPARENCIA_MAX);
        self.atualizacao_ms = self.atualizacao_ms.clamp(ATUALIZACAO_MIN, ATUALIZACAO_MAX);
        self.alertas = self.alertas.dentro_das_faixas();
        self.card_morte_s = self.card_morte_s.clamp(CARD_MORTE_MIN_S, CARD_MORTE_MAX_S);
        self.janela_morte_s = self.janela_morte_s.clamp(JANELA_MORTE_MIN_S, JANELA_MORTE_MAX_S);
        self
    }

    /// Alfa do fundo do overlay. Os 15% padrão dão 0xD9, o fundo de antes da opção.
    pub fn alfa_do_fundo(&self) -> u8 {
        // Em inteiros, arredondando: 85% de 255 = 216,75 → 217.
        (((100 - self.transparencia.min(100)) * 255 + 50) / 100) as u8
    }
}

/// Passos de 5%: cada escala nova refaz o atlas de fontes do egui.
pub fn arredondar_zoom(zoom: f32) -> f32 {
    ((zoom * 20.0).round() / 20.0).clamp(ZOOM_MIN, ZOOM_MAX)
}

fn arquivo() -> PathBuf {
    catalogo::pasta_dados().join("config.json")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn arquivo_antigo_ou_torto_fica_com_padrao_e_dentro_das_faixas() {
        // Com campos da 0.7 (colunas, classe), que a 0.8.0 ignora.
        let parcial: Config = serde_json::from_str(
            r#"{"colunas":[true,false,true,true,true],"classe":false,"so_meu_dano":true,"inatividade":3,"zoom":7.0,"transparencia":250}"#,
        )
        .unwrap();
        let c = parcial.dentro_das_faixas();
        assert!(c.so_meu_dano);
        assert_eq!(c.numeros, [true; 3]);
        assert_eq!(c.inatividade, INATIVIDADE_MIN);
        assert_eq!(c.zoom, ZOOM_MAX);
        assert_eq!(arredondar_zoom(1.234), 1.25);
        assert_eq!(c.transparencia, TRANSPARENCIA_MAX);
        assert_eq!(c.alfa_do_fundo(), 26);
        // Config de antes dos atalhos e das opções da 0.8.0: os padrões.
        assert_eq!((c.atalho_mostrar.as_str(), c.atalho_atravessar.as_str()), ("Ctrl+H", "Ctrl+T"));
        assert_eq!((c.atalho_resumo.as_str(), c.atalho_compacta.as_str()), ("", ""));
        assert_eq!((c.ocultar_nomes, c.atualizacao_ms, c.resumo_em_linhas, c.compacta), (false, 500, false, false));
        // Config de antes da 0.10.0: os eventos recolhidos.
        assert!(!c.eventos_expandidos);
        let rapida = Config { atualizacao_ms: 10, ..Config::default() }.dentro_das_faixas();
        assert_eq!(rapida.atualizacao_ms, ATUALIZACAO_MIN);
        // Sem o campo (config da 0.3.x): o fundo de antes.
        assert_eq!(Config::default().alfa_do_fundo(), 0xD9);
        assert_eq!(Config { transparencia: 0, ..Config::default() }.alfa_do_fundo(), 0xFF);
        // Config de antes da 0.14.0: os alertas e o relatório de morte com o padrão.
        assert_eq!(c.alertas, Alertas::default());
        assert_eq!((c.relatorio_morte, c.card_morte_s, c.janela_morte_s), (true, 15, 10));
        assert!(c.recordes);
        let torta = Config { card_morte_s: 1, janela_morte_s: 90, ..Config::default() }.dentro_das_faixas();
        assert_eq!((torta.card_morte_s, torta.janela_morte_s), (CARD_MORTE_MIN_S, JANELA_MORTE_MAX_S));
    }

    #[test]
    fn alertas_tortos_voltam_para_a_faixa_sem_perder_o_resto() {
        let lida: Config = serde_json::from_str(
            r#"{"zoom":1.5,"alertas":{"eventos":{"nahma":10,"shugo":500,"sumiu":3},"chefes_antes_min":90,
                "balao":"as_vezes","banner_s":1,"na_hora":false}}"#,
        )
        .unwrap();
        let c = lida.dentro_das_faixas();
        assert_eq!(c.zoom, 1.5);
        let a = &c.alertas;
        assert_eq!(a.eventos.get("nahma"), Some(&10));
        assert_eq!(a.eventos.get("shugo"), Some(&ANTES_MAX_MIN));
        assert_eq!(a.chefes_antes_min, ANTES_MAX_MIN);
        assert_eq!((a.balao(), a.balao.as_str()), (Balao::SemBanner, "sem_banner"));
        assert_eq!(a.banner_s, BANNER_MIN_S);
        assert!(!a.na_hora);
        assert!(a.ligados && a.som && !a.so_com_jogo_aberto);

        let muitos = Alertas { chefes: (0..150).collect(), ..Alertas::default() }.dentro_das_faixas();
        assert_eq!(muitos.chefes.len(), CHEFES_MARCADOS_MAX);
    }
}
