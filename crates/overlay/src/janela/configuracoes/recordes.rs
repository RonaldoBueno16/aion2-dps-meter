//! Página Recordes: liga e desliga a gravação dos recordes de chefe e leva à tela deles.

use eframe::egui::Ui;

use super::super::{Overlay, Tela, botao};
use super::{dica, interruptor};
use crate::config::Config;
use crate::recordes::Guardados;

pub(super) fn resumo_recordes(c: &Config, guardados: Option<&Guardados>) -> String {
    if !c.recordes {
        return "desligados".into();
    }
    match guardados.map_or(0, |g| g.recordes.lista.len()) {
        0 => "nenhum ainda".into(),
        1 => "1 recorde".into(),
        n => format!("{n} recordes"),
    }
}

impl Overlay {
    pub(super) fn pagina_recordes(&mut self, ui: &mut Ui) {
        let acento = self.acento();
        interruptor(
            ui,
            "Guardar recordes de chefe",
            "O seu melhor tempo de kill e o seu melhor DPS em cada chefe, por classe, para comparar na próxima \
             luta. Ficam no recordes.json, só com o código do chefe, a sua classe e números.",
            &mut self.config.recordes,
            acento,
        );
        ui.add_space(8.0);
        dica(
            ui,
            "Conta o kill com o Axon aberto desde antes do primeiro golpe no chefe: o tempo exige o chefe com o \
             HP cheio e menos de 30 jogadores; o DPS, 20 s batendo nele. Desligar não apaga o arquivo.",
        );
        if self.config.recordes {
            ui.add_space(8.0);
            if botao(ui, "Ver recordes", false).on_hover_text("Lista, apagar um ou todos").clicked() {
                self.tela = Tela::Recordes;
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resumo_conta_os_recordes_ou_diz_desligados() {
        let mut c = Config::default();
        assert_eq!(resumo_recordes(&c, None), "nenhum ainda");
        c.recordes = false;
        assert_eq!(resumo_recordes(&c, None), "desligados");
    }
}
