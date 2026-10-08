//! Página Alertas: a chave geral, como o alerta chega, a antecedência de cada evento e os chefes de
//! campo marcados (o 🔔 da tela Bosses marca; aqui só desmarca).

use eframe::egui::{Align, Layout, RichText, Ui};

use super::super::{Overlay, botao, chefes_marcados, fonte, texto, travar, visual};
use super::{dica, divisoria, frase, interruptor, passo, secao, valor};
use crate::alertas;
use crate::config::{BANNER_MAX_S, BANNER_MIN_S, Balao, Config};
use crate::eventos::EVENTOS;

/// Antecedências que o −/+ percorre, em minutos (0 = só na hora).
const ANTECEDENCIAS: [u32; 8] = [0, 1, 3, 5, 10, 15, 30, 60];

pub(super) fn resumo_alertas(c: &Config) -> String {
    let a = &c.alertas;
    if !a.ligados {
        return "desligados".into();
    }
    let plural = |n: usize, um: &str, varios: &str| format!("{n} {}", if n == 1 { um } else { varios });
    let eventos = a.eventos.keys().filter(|id| EVENTOS.iter().any(|e| e.id == id.as_str())).count();
    match (eventos, a.chefes.len()) {
        (0, 0) => "nenhum marcado".into(),
        (e, 0) => plural(e, "evento", "eventos"),
        (0, ch) => plural(ch, "chefe", "chefes"),
        (e, ch) => format!("{}, {}", plural(e, "evento", "eventos"), plural(ch, "chefe", "chefes")),
    }
}

/// "na hora" ou "10 min antes".
fn antecedencia(minutos: u32) -> String {
    if minutos == 0 { "na hora".into() } else { format!("{minutos} min antes") }
}

/// O vizinho na lista de antecedências: o próximo maior (+) ou menor (−).
fn vizinho(minutos: u32, mais: bool) -> Option<u32> {
    if mais {
        ANTECEDENCIAS.into_iter().find(|&m| m > minutos)
    } else {
        ANTECEDENCIAS.into_iter().rev().find(|&m| m < minutos)
    }
}

impl Overlay {
    pub(super) fn pagina_alertas(&mut self, ui: &mut Ui) {
        let acento = self.acento();
        interruptor(
            ui,
            "Alertas ligados",
            "Também no menu do ícone do Axon, ao lado do relógio, sem abrir o overlay.",
            &mut self.config.alertas.ligados,
            acento,
        );

        ui.add_space(14.0);
        secao(ui, "Como o alerta chega");
        dica(ui, "Uma faixa no topo do overlay, que não tira o foco do jogo, mais o que estiver ligado aqui.");
        ui.add_space(6.0);
        let a = &mut self.config.alertas;
        interruptor(ui, "Som", "Duas notas curtas, no volume do Axon no mixer do Windows.", &mut a.som, acento);
        if !a.som {
            ui.add(
                eframe::egui::Label::new(
                    RichText::new(
                        "Sem som, com o overlay escondido ou recolhido durante o jogo, o alerta pode passar sem você \
                         ver: com o jogo aberto, o Windows costuma segurar o balão.",
                    )
                    .font(fonte(10.0, false))
                    .color(visual::AMARELO),
                )
                .wrap(),
            );
        }
        ui.add_space(6.0);
        interruptor(
            ui,
            "Também na hora",
            "Quando o evento começa e quando o chefe renasce, além do aviso antes.",
            &mut a.na_hora,
            acento,
        );
        ui.add_space(6.0);
        interruptor(
            ui,
            "Só com o jogo aberto",
            "Desligado, os eventos avisam também com o jogo fechado (lembra de abrir).",
            &mut a.so_com_jogo_aberto,
            acento,
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            frase(ui, "Balão do Windows");
            let atual = a.balao();
            for (opcao, rotulo) in [(Balao::Nunca, "nunca"), (Balao::SemBanner, "sem a faixa"), (Balao::Sempre, "sempre")] {
                if botao(ui, rotulo, atual == opcao).clicked() {
                    a.balao = opcao.texto().into();
                }
                ui.add_space(2.0);
            }
        });
        dica(ui, "\"Sem a faixa\": só quando a faixa não aparece (overlay escondido ou recolhido na borda).");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            frase(ui, "Faixa por");
            let s = a.banner_s;
            if passo(ui, "−", s > BANNER_MIN_S) {
                a.banner_s = (s - 5).max(BANNER_MIN_S);
            }
            valor(ui, &format!("{s} s"), 40.0);
            if passo(ui, "+", s < BANNER_MAX_S) {
                a.banner_s = (s + 5).min(BANNER_MAX_S);
            }
        });
        ui.add_space(8.0);
        if botao(ui, "Testar alerta", false).on_hover_text("Sai em até 1 s, pelo mesmo caminho dos alertas").clicked() {
            alertas::testar();
        }

        ui.add_space(14.0);
        secao(ui, "Eventos");
        dica(ui, "Pelo horário de Brasília, como na lista de eventos (o 🔔 dela liga com 5 min).");
        ui.add_space(4.0);
        for evento in EVENTOS {
            let atual = self.config.alertas.eventos.get(evento.id).copied();
            ui.horizontal(|ui| {
                ui.label(RichText::new(evento.nome).font(fonte(12.0, false)).color(texto()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Da direita para a esquerda: +, o valor, −.
                    let mais = match atual {
                        None => Some(0),
                        Some(m) => vizinho(m, true),
                    };
                    if passo(ui, "+", mais.is_some())
                        && let Some(m) = mais
                    {
                        self.config.alertas.eventos.insert(evento.id.into(), m);
                    }
                    let rotulo = atual.map_or_else(|| "desligado".into(), antecedencia);
                    valor(ui, &rotulo, 86.0);
                    if passo(ui, "−", atual.is_some()) {
                        match atual.and_then(|m| vizinho(m, false)) {
                            Some(m) => self.config.alertas.eventos.insert(evento.id.into(), m),
                            None => self.config.alertas.eventos.remove(evento.id),
                        };
                    }
                });
            });
        }

        ui.add_space(14.0);
        secao(ui, "Chefes de campo");
        dica(
            ui,
            "Marque no 🔔 da tela Bosses (♛). Fora da região, o aviso usa a última lista que o jogo mandou, e a \
             lista some ao fechar o Axon.",
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            frase(ui, "Avisar");
            let m = self.config.alertas.chefes_antes_min;
            if passo(ui, "−", vizinho(m, false).is_some()) {
                self.config.alertas.chefes_antes_min = vizinho(m, false).unwrap_or(m);
            }
            valor(ui, &antecedencia(m), 86.0);
            if passo(ui, "+", vizinho(m, true).is_some()) {
                self.config.alertas.chefes_antes_min = vizinho(m, true).unwrap_or(m);
            }
            frase(ui, "de renascer");
        });
        ui.add_space(4.0);
        dica(ui, "O chefe que derruba um item da lista de desejos com o 🔔 ligado nela também avisa (★).");
        ui.add_space(4.0);
        let marcados = self.config.alertas.chefes.clone();
        if marcados.is_empty() {
            dica(ui, "Nenhum chefe marcado.");
            return;
        }
        // O nome vem da lista da região, se ela já chegou nesta execução; sem ela, o número.
        let conhecidos = {
            let sessao = travar(&self.sessao);
            chefes_marcados(&sessao.medidor.chefes_por_regiao, &marcados, &[], 0.0, nucleo::agora())
        };
        for (i, id) in marcados.into_iter().enumerate() {
            if i > 0 {
                divisoria(ui);
            }
            let nome = conhecidos
                .iter()
                .find(|c| c.id == id)
                .map_or_else(|| format!("Chefe {} da região {}", id % 100, id / 100), |c| c.nome.clone());
            ui.horizontal(|ui| {
                ui.label(RichText::new(nome).font(fonte(12.0, false)).color(texto()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if botao(ui, "✕", false).on_hover_text("Desmarcar").clicked() {
                        self.config.alertas.chefes.remove(&id);
                    }
                });
            });
        }
        ui.add_space(2.0);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn antecedencia_anda_pela_lista_e_resumo_conta_o_marcado() {
        assert_eq!(vizinho(5, true), Some(10));
        assert_eq!(vizinho(5, false), Some(3));
        assert_eq!(vizinho(60, true), None);
        assert_eq!(vizinho(0, false), None);
        // Valor editado à mão fora da lista: o vizinho é o próximo da lista.
        assert_eq!(vizinho(7, true), Some(10));
        assert_eq!(vizinho(7, false), Some(5));
        assert_eq!((antecedencia(0), antecedencia(10)), ("na hora".to_string(), "10 min antes".to_string()));

        let mut c = Config::default();
        assert_eq!(resumo_alertas(&c), "nenhum marcado");
        c.alertas.eventos.insert("nahma".into(), 10);
        c.alertas.eventos.insert("sumiu".into(), 10);
        assert_eq!(resumo_alertas(&c), "1 evento");
        c.alertas.chefes.extend([111021, 111013]);
        assert_eq!(resumo_alertas(&c), "1 evento, 2 chefes");
        c.alertas.ligados = false;
        assert_eq!(resumo_alertas(&c), "desligados");
    }
}
