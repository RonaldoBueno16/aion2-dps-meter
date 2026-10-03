//! Preferências do overlay, em %LOCALAPPDATA%\Aion2Meter\config.json. Campo que faltar no arquivo
//! (versão anterior) fica com o padrão; valor fora da faixa é trazido para dentro dela.

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// DPS, Damage(%), CRIT, AVG e MAX, na ordem da tabela da aba DPS. O nome ficou o da 0.3.0
    /// de antes das outras abas terem tabela, para o config.json continuar valendo.
    pub colunas: [bool; 5],
    /// DTPS, Taken(%), PARRY, CRIT e MAX.
    pub colunas_tank: [bool; 5],
    /// HPS, Heal(%), CRIT, AVG e MAX.
    pub colunas_healer: [bool; 5],
    pub classe: bool,
    pub nivel: bool,
    pub gs: bool,
    /// Só a sua linha, nas três abas.
    pub so_meu_dano: bool,
    /// Segundos sem dano até a próxima pancada começar uma luta nova.
    pub inatividade: u32,
    /// Escala da janela inteira (1 = 470 px de largura).
    pub zoom: f32,
    /// Transparência do fundo, em %. Texto, barras e borda não mudam.
    pub transparencia: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            colunas: [true; 5],
            colunas_tank: [true; 5],
            colunas_healer: [true; 5],
            classe: true,
            nivel: true,
            gs: true,
            so_meu_dano: false,
            inatividade: 15,
            zoom: 1.0,
            transparencia: TRANSPARENCIA_PADRAO,
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
        self
    }

    /// Alfa do fundo do overlay. Os 15% padrão dão 0xD9, o fundo de antes da opção.
    pub fn alfa_do_fundo(&self) -> u8 {
        // Em inteiros, arredondando: 85% de 255 = 216,75 → 217.
        (((100 - self.transparencia.min(100)) * 255 + 50) / 100) as u8
    }

    /// Classe, Nv e GS abaixo do nome, nessa ordem.
    pub fn perfil(&self) -> [bool; 3] {
        [self.classe, self.nivel, self.gs]
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
        let parcial: Config =
            serde_json::from_str(r#"{"so_meu_dano":true,"inatividade":3,"zoom":7.0,"transparencia":250}"#).unwrap();
        let c = parcial.dentro_das_faixas();
        assert!(c.so_meu_dano);
        assert_eq!(c.colunas, [true; 5]);
        assert_eq!((c.colunas_tank, c.colunas_healer), ([true; 5], [true; 5]));
        assert!(c.classe && c.nivel && c.gs);
        assert_eq!(c.inatividade, INATIVIDADE_MIN);
        assert_eq!(c.zoom, ZOOM_MAX);
        assert_eq!(arredondar_zoom(1.234), 1.25);
        assert_eq!(c.transparencia, TRANSPARENCIA_MAX);
        assert_eq!(c.alfa_do_fundo(), 26);
        // Sem o campo (config da 0.3.x): o fundo de antes.
        assert_eq!(Config::default().alfa_do_fundo(), 0xD9);
        assert_eq!(Config { transparencia: 0, ..Config::default() }.alfa_do_fundo(), 0xFF);
    }
}
