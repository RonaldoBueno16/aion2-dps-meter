//! Faixas de alerta no topo do overlay: as 3 mais novas empilhadas (as outras viram "+N"), com o
//! ícone, o nome, o texto e a contagem até o alvo. Somem sozinhas em `banner_s` (com o clique
//! atravessando o overlay, o ✕ nem recebe clique) ou no ✕.

use std::time::Duration;

use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, CursorIcon, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2, pos2, vec2};
use nucleo::TICKS_POR_SEGUNDO;

use super::{Overlay, branco, fonte, montar, texto, trecho, uma_linha, visual};
use crate::alertas::{self, Alerta, Momento, Origem};
use crate::eventos;

const NA_TELA: usize = 3;
const ALTURA: f32 = 30.0;

impl Overlay {
    /// O que o fio da bandeja pôs na fila, menos o que passou do tempo (com o overlay escondido, a
    /// fila não anda).
    pub(super) fn receber_faixas(&mut self) {
        let duracao = Duration::from_secs(u64::from(self.config.alertas.banner_s));
        self.faixas.extend(alertas::retirar());
        self.faixas.retain(|(_, desde)| desde.elapsed() < duracao);
    }

    pub(super) fn faixas_de_alerta(&mut self, ui: &mut Ui) {
        if self.faixas.is_empty() {
            return;
        }
        let agora_s = nucleo::agora().div_euclid(TICKS_POR_SEGUNDO);
        let total = self.faixas.len();
        let mut fechar = None;
        // A mais nova em cima.
        for i in (total.saturating_sub(NA_TELA)..total).rev() {
            let alerta = self.faixas[i].0.clone();
            if self.faixa(ui, &alerta, agora_s, i) {
                fechar = Some(i);
            }
            ui.add_space(4.0);
        }
        if total > NA_TELA {
            let formato = TextFormat::simple(fonte(10.0, false), branco(0x99));
            let mais = montar(ui, LayoutJob::single_section(format!("+{} alertas", total - NA_TELA), formato));
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), mais.size().y), Sense::hover());
            ui.painter().galley(pos2(rect.max.x - mais.size().x, rect.min.y), mais, texto());
            ui.add_space(4.0);
        }
        if let Some(i) = fechar {
            self.faixas.remove(i);
        }
        ui.add_space(2.0);
    }

    /// Uma faixa; true no clique do ✕.
    fn faixa(&mut self, ui: &mut Ui, alerta: &Alerta, agora_s: i64, indice: usize) -> bool {
        let (rect, resposta) = ui.allocate_exact_size(vec2(ui.available_width(), ALTURA), Sense::hover());
        let pintor = ui.painter().clone();
        let dourado = visual::DOURADO;
        pintor.rect_filled(rect, 5, Color32::from_rgba_unmultiplied(dourado.r(), dourado.g(), dourado.b(), 0x1C));
        pintor.rect_stroke(rect, 5, Stroke::new(1.0_f32, dourado.gamma_multiply(0.7)), StrokeKind::Inside);
        let meio = rect.center().y;

        let quadrado = Rect::from_center_size(pos2(rect.min.x + 17.0, meio), Vec2::splat(20.0));
        match (&alerta.icone, alerta.origem) {
            (Some((nome, recorte)), Origem::Chefe(_)) => self.imagem_web(ui, nome, *recorte, quadrado),
            (Some((nome, recorte)), _) => self.icone_do_evento(ui, nome, *recorte, quadrado),
            (None, _) => {
                let sino = montar(ui, LayoutJob::single_section("🔔".into(), TextFormat::simple(fonte(14.0, false), dourado)));
                pintor.galley(quadrado.center() - sino.size() / 2.0, sino, texto());
            }
        }

        let fechar = Rect::from_center_size(pos2(rect.max.x - 14.0, meio), Vec2::splat(20.0));
        let clique = ui
            .interact(fechar, ui.id().with(("fechar_faixa", indice, alerta.alvo)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Fechar");
        if clique.hovered() {
            pintor.rect_filled(fechar, 3, branco(0x22));
        }
        let xis = montar(ui, LayoutJob::single_section("✕".into(), TextFormat::simple(fonte(11.0, false), branco(0xAA))));
        pintor.galley(fechar.center() - xis.size() / 2.0, xis, texto());

        // A contagem viva até o início ou o renascer, no pré-aviso.
        let mut direita = fechar.min.x - 6.0;
        if alerta.momento == Momento::Antes && alerta.alvo > agora_s {
            let formato = TextFormat::simple(fonte(12.0, true), dourado);
            let contagem = montar(ui, LayoutJob::single_section(eventos::contagem(alerta.alvo - agora_s), formato));
            direita -= contagem.size().x;
            pintor.galley(pos2(direita, meio - contagem.size().y / 2.0), contagem, texto());
            direita -= 8.0;
        }

        let inicio = quadrado.max.x + 8.0;
        let mut job = LayoutJob::default();
        trecho(&mut job, &alerta.titulo, 11.0, true, texto());
        trecho(&mut job, &format!("  {}", alerta.texto), 11.0, false, branco(0xCC));
        job.wrap = uma_linha((direita - inicio).max(40.0));
        let linha = montar(ui, job);
        pintor.galley(pos2(inicio, meio - linha.size().y / 2.0), linha, texto());
        resposta.on_hover_text(format!("{}: {}", alerta.titulo, alerta.texto));
        clique.clicked()
    }
}
