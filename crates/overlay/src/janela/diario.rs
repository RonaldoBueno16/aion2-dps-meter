//! Diário e semanal (F6): a contagem até os resets, a Energia Odyle com a marca de valor velho e o
//! checklist dos eventos da tabela, marcado por clique, que desmarca sozinho no reset (todo dia às
//! 04h; o semanal na quarta às 04h). No config.json vão só o id do evento, o período e a hora da
//! marcação. As entradas de cada dungeon (0x610B) entram quando os nomes forem conferidos com print.

use eframe::egui::text::LayoutJob;
use eframe::egui::{Align, CursorIcon, Layout, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use nucleo::formato::n;
use nucleo::{Hora, TICKS_POR_SEGUNDO};

use super::{Overlay, Tela, botao, branco, fonte, montar, texto, trecho, visual};
use crate::config::{CHECKLIST_MAX, ItemChecklist};
use crate::eventos::{self, EVENTOS, Evento, Quando};

/// Os eventos do checklist: os que abrem (os resets só acontecem).
fn do_checklist() -> impl Iterator<Item = &'static Evento> {
    EVENTOS.iter().filter(|e| e.aberto > 0)
}

/// Sem escolha do jogador: semanal o evento de dias da semana (cerco, Nahma), diário o de ciclo.
fn semanal(item: Option<&ItemChecklist>, evento: &Evento) -> bool {
    item.map_or(matches!(evento.quando, Quando::Semanal { .. }), |i| i.periodo == "semanal")
}

/// Feito: marcado depois do último reset do período.
pub(super) fn feito(item: &ItemChecklist, agora: Hora) -> bool {
    let inicio = eventos::ultimo_reset(item.periodo == "semanal", agora);
    item.feito_em.is_some_and(|marcado| marcado >= inicio)
}

/// Valor que chegou antes do último reset diário: pode ter mudado no reset sem o servidor mandar.
pub(super) fn velho(chegou: Hora, agora: Hora) -> bool {
    chegou.div_euclid(TICKS_POR_SEGUNDO) < eventos::ultimo_reset(false, agora)
}

/// Quantos itens estão feitos agora, e quantos são.
fn feitos(checklist: &[ItemChecklist], agora: Hora) -> (usize, usize) {
    let feito_o = |e: &&Evento| checklist.iter().any(|i| i.nome == e.id && feito(i, agora));
    (do_checklist().filter(feito_o).count(), do_checklist().count())
}

/// O que um clique na lista pede; aplicado depois de desenhar.
enum Acao {
    Marcar(&'static Evento),
    Periodo(&'static Evento),
}

impl Overlay {
    /// Na área de eventos expandida: quantos itens feitos; o clique abre a tela.
    pub(super) fn linha_do_diario(&mut self, ui: &mut Ui, inicio: f32, fim: f32) {
        let agora = nucleo::agora();
        let (rect, resposta) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
        if resposta.hovered() {
            ui.painter().rect_filled(rect, 4, branco(0x0C));
        }
        let meio = rect.center().y;
        let mut job = LayoutJob::default();
        trecho(&mut job, "☑", 11.0, false, branco(0x99));
        let marca = montar(ui, job);
        ui.painter().galley(pos2(rect.min.x + 7.0 - marca.size().x / 2.0, meio - marca.size().y / 2.0), marca, texto());
        let (feitos, total) = feitos(&self.config.checklist, agora);
        let mut job = LayoutJob::default();
        trecho(&mut job, "Diário e semanal", 11.0, true, branco(0xCC));
        trecho(&mut job, &format!("   {feitos} de {total} feitos"), 10.0, false, branco(0x88));
        let titulo = montar(ui, job);
        ui.painter().galley(pos2(inicio, meio - titulo.size().y / 2.0), titulo, texto());
        let falta = eventos::ultimo_reset(false, agora) + 86_400 - agora.div_euclid(TICKS_POR_SEGUNDO);
        let mut job = LayoutJob::default();
        trecho(&mut job, &format!("reset em {}", eventos::contagem(falta)), 10.0, false, branco(0x77));
        let reset = montar(ui, job);
        ui.painter().galley(pos2(fim - reset.size().x, meio - reset.size().y / 2.0), reset, texto());
        let dica = "Abrir o checklist do dia e da semana, com a contagem até os resets e a Energia Odyle.";
        if resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            self.tela = Tela::Diario;
        }
    }

    pub(super) fn tela_diario(&mut self, ui: &mut Ui) {
        let agora = nucleo::agora();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Diário e semanal").font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "Voltar", true).on_hover_text("Volta ao medidor").clicked() {
                    self.tela = Tela::Medidor;
                }
            });
        });
        ui.add_space(4.0);
        let explicacao = "Marque o que você já fez: o diário desmarca sozinho às 04h e o semanal na quarta às 04h \
                          (horário de Brasília). No config.json fica só o evento e a hora da marcação.";
        ui.add(eframe::egui::Label::new(RichText::new(explicacao).font(fonte(10.0, false)).color(branco(0x88))).wrap());
        ui.add_space(6.0);

        for id in ["reset_diario", "reset_semanal"] {
            let Some(evento) = EVENTOS.iter().find(|e| e.id == id) else { continue };
            let falta = match eventos::estado(evento, agora) {
                eventos::Estado::Aberto(_) => 0,
                eventos::Estado::Fechado(s) => s,
            };
            let horario = eventos::horario(evento, agora);
            let dica = format!("{}\n\n{}", evento.dica, eventos::ORIGEM);
            linha_de_valor(ui, evento.nome, &format!("{horario}   em {}", eventos::contagem(falta)), branco(0xCC), &dica);
        }
        let (valor, cor, dica) = match (self.odyle, self.odyle_chegou) {
            (Some((basica, carregada)), Some(chegou)) => {
                let carregada = carregada.map_or_else(String::new, |c| format!(" (+{})", n(c as f64, 0)));
                let valor = format!("{}{carregada}", n(basica as f64, 0));
                if velho(chegou, agora) {
                    let dica = "Chegou antes do reset das 04h: atualiza no próximo login ou na próxima mudança \
                                (como ao usar essência OD).";
                    (format!("{valor}   antes do reset"), branco(0x66), dica)
                } else {
                    (valor, branco(0xCC), "A básica e, entre parênteses, a carregada, como o servidor manda.")
                }
            }
            _ => (
                "chega no login".to_string(),
                branco(0x66),
                "Os valores chegam no login: abra o Axon antes de entrar no personagem.",
            ),
        };
        linha_de_valor(ui, "Energia Odyle", &valor, cor, dica);
        ui.add_space(8.0);

        let mut acao = None;
        for evento in do_checklist() {
            if let Some(a) = self.linha_do_checklist(ui, evento, agora) {
                acao = Some(a);
            }
        }
        let agora_s = agora.div_euclid(TICKS_POR_SEGUNDO);
        let Some(acao) = acao else { return };
        let evento = match acao {
            Acao::Marcar(e) | Acao::Periodo(e) => e,
        };
        let posicao = self.config.checklist.iter().position(|i| i.nome == evento.id);
        let posicao = match posicao {
            Some(p) => p,
            None if self.config.checklist.len() < CHECKLIST_MAX => {
                let periodo = if semanal(None, evento) { "semanal" } else { "diario" };
                self.config.checklist.push(ItemChecklist { nome: evento.id.into(), periodo: periodo.into(), feito_em: None });
                self.config.checklist.len() - 1
            }
            None => return,
        };
        let item = &mut self.config.checklist[posicao];
        match acao {
            Acao::Marcar(_) => item.feito_em = if feito(item, agora) { None } else { Some(agora_s) },
            Acao::Periodo(_) => item.periodo = if item.periodo == "semanal" { "diario".into() } else { "semanal".into() },
        }
        self.aplicar_config();
    }

    /// A caixa, o ícone e o nome do evento; à direita, quando foi feito e o período (o clique troca).
    fn linha_do_checklist(&mut self, ui: &mut Ui, evento: &'static Evento, agora: Hora) -> Option<Acao> {
        let item = self.config.checklist.iter().find(|i| i.nome == evento.id).cloned();
        let semanal = semanal(item.as_ref(), evento);
        let feito = item.as_ref().is_some_and(|i| feito(i, agora));
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 24.0), Sense::click());
        if resposta.hovered() {
            ui.painter().rect_filled(rect, 3, branco(0x0C));
        }
        let meio = rect.center().y;
        let caixa = Rect::from_center_size(pos2(rect.min.x + 10.0, meio), Vec2::splat(14.0));
        if feito {
            ui.painter().rect_filled(caixa, 3, visual::DOURADO);
            let escuro = eframe::egui::Color32::from_rgb(0x10, 0x14, 0x18);
            let marca = ui.fonts_mut(|f| f.layout_no_wrap("✓".into(), fonte(11.0, true), escuro));
            ui.painter().galley(caixa.center() - marca.size() / 2.0, marca, escuro);
        } else {
            let borda = if resposta.hovered() { branco(0xAA) } else { branco(0x55) };
            ui.painter().rect_stroke(caixa, 3, Stroke::new(1.0_f32, borda), eframe::egui::StrokeKind::Inside);
        }
        let quadrado = Rect::from_center_size(pos2(rect.min.x + 34.0, meio), Vec2::splat(16.0));
        match &evento.icone {
            Some(icone) => self.icone_do_evento(ui, icone.nome, icone.recorte, quadrado),
            None => {
                ui.painter().rect_filled(quadrado, 3, branco(0x22));
            }
        }

        let cor_chip = if semanal { visual::DOURADO } else { branco(0xBB) };
        let rotulo = if semanal { "Semanal" } else { "Diário" };
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(rotulo.into(), fonte(10.0, true), cor_chip));
        let chip = Rect::from_center_size(pos2(rect.max.x - 4.0 - 28.0, meio), vec2(56.0, 18.0));
        let clique = ui.interact(chip, ui.id().with(("periodo", evento.id)), Sense::click());
        ui.painter().rect_filled(chip, 9, branco(if clique.hovered() { 0x2A } else { 0x16 }));
        ui.painter().galley(chip.center() - galley.size() / 2.0, galley, cor_chip);
        let quando = if semanal { "na quarta às 04h" } else { "todo dia às 04h" };
        let dica = format!("Desmarca sozinho {quando}. Clique para trocar entre diário e semanal.");
        let mut acao = None;
        if clique.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            acao = Some(Acao::Periodo(evento));
        }

        let mut job = LayoutJob::default();
        trecho(&mut job, evento.nome, 11.0, false, if feito { branco(0x88) } else { texto() });
        let nome = montar(ui, job);
        ui.painter().galley(pos2(quadrado.max.x + 8.0, meio - nome.size().y / 2.0), nome, texto());
        if let Some(marcado) = item.as_ref().and_then(|i| i.feito_em).filter(|_| feito) {
            let mut job = LayoutJob::default();
            trecho(&mut job, &format!("feito {}", eventos::horario_unix(marcado, agora)), 10.0, false, branco(0x77));
            let quando = montar(ui, job);
            ui.painter().galley(pos2(chip.min.x - 8.0 - quando.size().x, meio - quando.size().y / 2.0), quando, texto());
        }
        let dica = if feito { "Feito. Clique para desmarcar." } else { "Clique para marcar como feito." };
        if acao.is_none() && resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            acao = Some(Acao::Marcar(evento));
        }
        acao
    }
}

/// Nome à esquerda e valor à direita, com a dica na linha toda.
fn linha_de_valor(ui: &mut Ui, nome: &str, valor: &str, cor: eframe::egui::Color32, dica: &str) {
    let largura = ui.available_width();
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 20.0), Sense::hover());
    let meio = rect.center().y;
    let mut job = LayoutJob::default();
    trecho(&mut job, nome, 11.0, false, branco(0xBB));
    let esquerda = montar(ui, job);
    let mut job = LayoutJob::default();
    trecho(&mut job, valor, 11.0, true, cor);
    let direita = montar(ui, job);
    let pintor = ui.painter();
    pintor.galley(pos2(rect.min.x + 2.0, meio - esquerda.size().y / 2.0), esquerda, texto());
    pintor.galley(pos2(rect.max.x - 4.0 - direita.size().x, meio - direita.size().y / 2.0), direita, texto());
    resposta.on_hover_text(dica);
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Terça, 2026-10-06, 04:00 em Brasília (07:00 UTC): o mesmo instante dos testes de alerta.
    const TERCA_4H: i64 = 1_791_270_000;
    const DIA: i64 = 86_400;

    fn hora(unix: i64) -> Hora {
        unix * TICKS_POR_SEGUNDO
    }

    fn marcado(periodo: &str, unix: i64) -> ItemChecklist {
        ItemChecklist { nome: "nahma".into(), periodo: periodo.into(), feito_em: Some(unix) }
    }

    #[test]
    fn semanal_marcado_na_terca_desmarca_na_quarta_as_4h() {
        let item = marcado("semanal", TERCA_4H + 3600);
        assert!(feito(&item, hora(TERCA_4H + 19 * 3600)));
        assert!(feito(&item, hora(TERCA_4H + DIA - 1)));
        assert!(!feito(&item, hora(TERCA_4H + DIA)));
        // Marcado na quarta às 04h em ponto: vale para a semana que começou, até a próxima terça.
        assert!(feito(&marcado("semanal", TERCA_4H + DIA), hora(TERCA_4H + DIA + 60)));
        assert!(feito(&marcado("semanal", TERCA_4H + DIA), hora(TERCA_4H + 7 * DIA)));
        // O mesmo marcado como diário já não vale na quinta.
        assert!(!feito(&marcado("diario", TERCA_4H + DIA), hora(TERCA_4H + 2 * DIA)));
    }

    #[test]
    fn diario_marcado_as_3h59_desmarca_as_4h() {
        let item = marcado("diario", TERCA_4H - 60);
        assert!(feito(&item, hora(TERCA_4H - 30)));
        assert!(!feito(&item, hora(TERCA_4H)));
        assert!(!feito(&ItemChecklist { feito_em: None, ..item }, hora(TERCA_4H - 30)));
    }

    #[test]
    fn valor_de_antes_do_reset_e_velho() {
        assert!(velho(hora(TERCA_4H - 1), hora(TERCA_4H + 10)));
        assert!(!velho(hora(TERCA_4H), hora(TERCA_4H + 10)));
        assert!(!velho(hora(TERCA_4H + 5), hora(TERCA_4H + DIA - 1)));
    }

    #[test]
    fn contagem_de_feitos_ignora_evento_que_saiu_da_tabela() {
        let checklist = [
            ItemChecklist { nome: "nahma".into(), periodo: "semanal".into(), feito_em: Some(TERCA_4H) },
            ItemChecklist { nome: "sumiu".into(), periodo: "diario".into(), feito_em: Some(TERCA_4H) },
        ];
        assert_eq!(feitos(&checklist, hora(TERCA_4H + 60)), (1, 7));
    }
}
