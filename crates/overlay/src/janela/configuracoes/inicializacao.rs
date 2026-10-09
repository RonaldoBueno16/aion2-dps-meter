//! Página Inicialização: o Axon junto com o Windows (a tarefa do Agendador) e escondido sem o jogo.
//! O schtasks leva uns 100 ms, então a consulta e a troca rodam numa thread; a página mostra
//! "consultando" até a resposta chegar.

use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui::{Color32, RichText, Ui};

use super::super::{Overlay, botao, fonte};
use super::{dica, interruptor, secao};
use crate::config::Config;
use crate::inicio::{self, Estado};

const VERMELHO: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);

/// O estado da tarefa e o erro da última troca; None enquanto o schtasks responde.
pub(super) type Tarefa = Arc<Mutex<Option<(Estado, Option<String>)>>>;

pub(super) fn resumo_inicializacao(tarefa: &Tarefa, c: &Config) -> String {
    let windows = match ler(tarefa).map(|(estado, _)| estado) {
        Some(Estado::Ligado) => "com o Windows",
        Some(Estado::OutroExe(_)) => "outro exe no Windows",
        Some(Estado::Desligado) => "à mão",
        None => "...",
    };
    if c.esconder_sem_jogo { format!("{windows}, some sem o jogo") } else { windows.into() }
}

fn ler(tarefa: &Tarefa) -> Option<(Estado, Option<String>)> {
    tarefa.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Consulta a tarefa (e antes liga ou desliga, se pedido) numa thread.
pub(super) fn consultar(tarefa: &Tarefa, acao: Option<fn() -> Result<(), String>>) {
    let alvo = tarefa.clone();
    *alvo.lock().unwrap_or_else(PoisonError::into_inner) = None;
    std::thread::spawn(move || {
        let erro = acao.and_then(|a| a().err());
        let estado = inicio::estado();
        *alvo.lock().unwrap_or_else(PoisonError::into_inner) = Some((estado, erro));
    });
}

impl Overlay {
    pub(super) fn pagina_inicializacao(&mut self, ui: &mut Ui) {
        let acento = self.acento();
        let tarefa = self.estado_config.tarefa.clone();
        match ler(&tarefa) {
            None => {
                secao(ui, "Iniciar com o Windows");
                dica(ui, "Consultando o Agendador de Tarefas...");
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
            Some((estado, erro)) => {
                let antes = estado != Estado::Desligado;
                let mut ligado = antes;
                interruptor(
                    ui,
                    "Iniciar com o Windows",
                    "Abre escondido no seu logon e aparece quando o AION 2 abre. Vai pela tarefa \"Axon\" do \
                     Agendador de Tarefas, que roda como administrador sem pedir de novo.",
                    &mut ligado,
                    acento,
                );
                if ligado != antes {
                    consultar(&tarefa, Some(if ligado { inicio::ligar } else { inicio::desligar }));
                }
                if let Estado::OutroExe(outro) = &estado {
                    ui.add_space(4.0);
                    dica(ui, &format!("A tarefa abre outro Axon: {}", outro.display()));
                    if botao(ui, "Usar este", false).on_hover_text("A tarefa passa a abrir este exe").clicked() {
                        consultar(&tarefa, Some(inicio::ligar));
                    }
                }
                if let Some(erro) = erro {
                    ui.add_space(4.0);
                    ui.add(
                        eframe::egui::Label::new(
                            RichText::new(format!("Não deu: {erro}")).font(fonte(10.0, false)).color(VERMELHO),
                        )
                        .wrap(),
                    );
                }
            }
        }
        ui.add_space(10.0);
        interruptor(
            ui,
            "Esconder sem o jogo",
            "Some 5 s depois de o AION 2 fechar e volta quando ele abre. O ícone ao lado do relógio e os \
             alertas continuam; o clique no ícone mostra a janela mesmo sem o jogo.",
            &mut self.config.esconder_sem_jogo,
            acento,
        );
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resumo_diz_como_o_axon_abre_e_se_some_sem_o_jogo() {
        let mut c = Config::default();
        let tarefa: Tarefa = Arc::new(Mutex::new(Some((Estado::Desligado, None))));
        assert_eq!(resumo_inicializacao(&tarefa, &c), "à mão, some sem o jogo");
        *tarefa.lock().unwrap() = Some((Estado::Ligado, None));
        c.esconder_sem_jogo = false;
        assert_eq!(resumo_inicializacao(&tarefa, &c), "com o Windows");
        *tarefa.lock().unwrap() = None;
        assert_eq!(resumo_inicializacao(&tarefa, &c), "...");
    }
}
