//! Página Lista de desejos: o destaque na tela Bosses, a chance mínima e o filtro. Os itens, a
//! prioridade e o alerta de cada um ficam na tela da lista (★ no cabeçalho).

use eframe::egui::Ui;

use super::super::desejos::chance_da_config;
use super::super::{Overlay, botao};
use super::{dica, frase, interruptor, passo, valor};
use crate::config::Config;

/// Chances mínimas que o −/+ percorre, em % (0 = qualquer).
const CHANCES: [f32; 8] = [0.0, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0];

pub(super) fn resumo_desejos(c: &Config) -> String {
    match c.desejos.len() {
        0 => "nenhum item".into(),
        1 => "1 item".into(),
        n => format!("{n} itens"),
    }
}

/// O vizinho na lista de chances: o próximo maior (+) ou menor (−).
fn vizinho(pct: f32, mais: bool) -> Option<f32> {
    if mais {
        CHANCES.into_iter().find(|&c| c > pct)
    } else {
        CHANCES.into_iter().rev().find(|&c| c < pct)
    }
}

impl Overlay {
    pub(super) fn pagina_desejos(&mut self, ui: &mut Ui) {
        let acento = self.acento();
        interruptor(
            ui,
            "Destacar na tela Bosses",
            "★ no chefe de campo da região que derruba um item da lista, direto ou pelo baú de saque, com a \
             chance mínima abaixo.",
            &mut self.config.desejos_destacar,
            acento,
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            frase(ui, "Chance mínima");
            let c = self.config.desejos_chance_minima;
            if passo(ui, "−", vizinho(c, false).is_some()) {
                self.config.desejos_chance_minima = vizinho(c, false).unwrap_or(c);
            }
            let texto_valor = if c <= 0.0 { "qualquer".to_string() } else { chance_da_config(c) };
            valor(ui, &texto_valor, 70.0);
            if passo(ui, "+", vizinho(c, true).is_some()) {
                self.config.desejos_chance_minima = vizinho(c, true).unwrap_or(c);
            }
        });
        dica(
            ui,
            "Item comum cai de centenas de monstros com chance minúscula: sem mínimo, quase todo chefe ganharia ★. \
             Vale também para o alerta.",
        );
        ui.add_space(10.0);
        interruptor(
            ui,
            "Só os chefes com desejo",
            "O filtro da tela Bosses: esconde os chefes que não derrubam nada da lista.",
            &mut self.config.desejos_so_com_desejo,
            acento,
        );
        ui.add_space(10.0);
        dica(
            ui,
            "No config.json vão só o código de cada item, a prioridade e o alerta. O nome e de onde ele vem chegam do \
             questlog a cada abertura, só na memória.",
        );
        ui.add_space(6.0);
        if botao(ui, "Abrir a lista", false).on_hover_text("Os itens, a prioridade e o 🔔 de cada um").clicked() {
            self.abrir_desejos();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn chance_anda_pela_lista_e_resumo_conta_os_itens() {
        assert_eq!(vizinho(0.5, true), Some(1.0));
        assert_eq!(vizinho(0.5, false), Some(0.25));
        assert_eq!((vizinho(10.0, true), vizinho(0.0, false)), (None, None));
        // Valor editado à mão fora da lista: o vizinho é o próximo da lista.
        assert_eq!(vizinho(0.7, false), Some(0.5));
        assert_eq!((chance_da_config(0.5), chance_da_config(0.25), chance_da_config(2.0)), ("0,5%".into(), "0,25%".into(), "2%".into()));
        let mut c = Config::default();
        assert_eq!(resumo_desejos(&c), "nenhum item");
        c.desejos.push(crate::config::Desejo { codigo: 210540076, prioridade: 1, alertar: true });
        assert_eq!(resumo_desejos(&c), "1 item");
    }
}
