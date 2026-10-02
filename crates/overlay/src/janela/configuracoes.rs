//! Tela de configurações. O que aparece em cada linha se escolhe clicando numa linha de amostra (a
//! sua, ou um esboço com o seu nome): o que está escondido fica no lugar, apagado e riscado. O resto
//! é quem entra na medição, quando a luta zera e o tamanho. Tudo vale na hora e fica salvo.

use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Align, Color32, CursorIcon, Galley, Layout, Rect, RichText, Sense, Stroke, StrokeKind, TextFormat, Ui, pos2, vec2,
};

use super::{
    COLUNAS, Colunas, Overlay, Tela, botao, branco, celulas, cor_da_classe, fonte, montar, segmentos_do_perfil, texto,
};
use crate::config::{self, INATIVIDADE_MAX, INATIVIDADE_MIN, ZOOM_MAX, ZOOM_MIN};

const NOMES_DO_PERFIL: [&str; 3] = ["a classe", "o level", "o GS"];

impl Overlay {
    pub(super) fn tela_configuracoes(&mut self, ui: &mut Ui) {
        let antes = self.config.clone();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Configurações").font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "Pronto", true).on_hover_text("Volta ao medidor").clicked() {
                    self.tela = Tela::Medidor;
                }
            });
        });

        ui.add_space(10.0);
        secao(ui, "Linha de cada jogador");
        dica(ui, "Clique numa coluna ou num dado para mostrar ou esconder.");
        ui.add_space(6.0);
        self.linha_de_amostra(ui);

        ui.add_space(14.0);
        secao(ui, "Quem entra na medição");
        ui.add_space(4.0);
        let acento = self.acento();
        interruptor(
            ui,
            "Só o meu dano",
            "Esconde os outros jogadores nas três abas; a % passa a ser só sua.",
            &mut self.config.so_meu_dano,
            acento,
        );
        ui.add_space(8.0);
        self.alcance(ui);

        ui.add_space(14.0);
        secao(ui, "Luta");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            frase(ui, "Luta nova depois de");
            let s = self.config.inatividade;
            if passo(ui, "−", s > INATIVIDADE_MIN) {
                self.config.inatividade = (s - 5).max(INATIVIDADE_MIN);
            }
            valor(ui, &format!("{s} s"), 40.0);
            if passo(ui, "+", s < INATIVIDADE_MAX) {
                self.config.inatividade = (s + 5).min(INATIVIDADE_MAX);
            }
            frase(ui, "sem dano");
        });

        ui.add_space(14.0);
        secao(ui, "Tamanho");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let z = self.config.zoom;
            if passo(ui, "−", z > ZOOM_MIN + 0.001) {
                self.config.zoom = config::arredondar_zoom(z - 0.1);
            }
            valor(ui, &format!("{:.0}%", z * 100.0), 48.0);
            if passo(ui, "+", z < ZOOM_MAX - 0.001) {
                self.config.zoom = config::arredondar_zoom(z + 0.1);
            }
            if (z - 1.0).abs() > 0.001 {
                ui.add_space(6.0);
                if botao(ui, "Restaurar", false).on_hover_text("Volta a 100%").clicked() {
                    self.config.zoom = 1.0;
                }
            }
        });
        ui.add_space(2.0);
        dica(ui, "Também dá para arrastar o canto de baixo à direita.");
        ui.add_space(4.0);

        if self.config != antes {
            self.aplicar_config();
        }
    }

    /// Cor do "ligado": a da sua classe, a mesma da sua barra; sem classe conhecida, o texto.
    fn acento(&self) -> Color32 {
        match self.amostra.as_ref().map(|j| j.classe) {
            Some(classe) if !classe.is_empty() => cor_da_classe(classe),
            _ => texto(),
        }
    }

    /// Proximidade | Party. Party fica desligado até haver uma captura em grupo para ler o 0x9702.
    fn alcance(&mut self, ui: &mut Ui) {
        let sem_efeito = self.config.so_meu_dano;
        ui.horizontal(|ui| {
            let cor = if sem_efeito { branco(0x55) } else { texto() };
            ui.label(RichText::new("Alcance").font(fonte(12.0, false)).color(cor));
            ui.add_space(10.0);
            let opcoes = [
                ("Proximidade", !sem_efeito, "Todos que aparecem perto de você."),
                ("Party", false, "Só quem está no seu grupo. Ainda não: falta uma captura em grupo para ler quem está nele."),
            ];
            let galleys: Vec<Arc<Galley>> = opcoes
                .iter()
                .enumerate()
                .map(|(i, (rotulo, habilitado, _))| {
                    let cor = if *habilitado { texto() } else { branco(0x55) };
                    montar(ui, LayoutJob::single_section(rotulo.to_string(), TextFormat::simple(fonte(12.0, i == 0), cor)))
                })
                .collect();
            let larguras: Vec<f32> = galleys.iter().map(|g| g.size().x + 20.0).collect();
            let altura = galleys[0].size().y + 4.0;
            let (caixa, _) = ui.allocate_exact_size(vec2(larguras.iter().sum(), altura), Sense::hover());
            ui.painter().rect_stroke(caixa, 4, Stroke::new(1.0_f32, branco(0x33)), StrokeKind::Inside);
            let mut x = caixa.min.x;
            for (i, (galley, largura)) in galleys.into_iter().zip(larguras).enumerate() {
                let segmento = Rect::from_min_size(pos2(x, caixa.min.y), vec2(largura, altura));
                let resposta = ui.interact(segmento, ui.id().with(("alcance", i)), Sense::hover()).on_hover_text(opcoes[i].2);
                // Proximidade é o único que existe: fica marcado.
                if i == 0 {
                    ui.painter().rect_filled(segmento.shrink(1.0), 3, branco(if sem_efeito { 0x22 } else { 0x44 }));
                } else if resposta.hovered() {
                    ui.painter().rect_filled(segmento.shrink(1.0), 3, branco(0x11));
                }
                let centro = segmento.center() - galley.size() / 2.0;
                ui.painter().galley(centro, galley, texto());
                x += largura;
            }
        });
        ui.add_space(2.0);
        if sem_efeito {
            dica(ui, "Com \"Só o meu dano\" ligado, o alcance não muda nada.");
        } else {
            dica(ui, "Party chega numa próxima versão: falta uma captura em grupo para ler quem está nele.");
        }
    }

    /// A sua linha como aparece na aba DPS. Cada coluna e cada dado abaixo do nome é um botão.
    fn linha_de_amostra(&mut self, ui: &mut Ui) {
        let Some(j) = self.amostra.clone() else { return };
        let largura = ui.available_width();
        let todas = Colunas::medir(ui, [true; 5]);
        let escondido = |tamanho: f32, negrito: bool| TextFormat {
            font_id: fonte(tamanho, negrito),
            color: branco(0x44),
            strikethrough: Stroke::new(1.0_f32, branco(0x66)),
            ..Default::default()
        };

        let titulos: Vec<Arc<Galley>> = COLUNAS
            .iter()
            .enumerate()
            .map(|(i, (titulo, _))| {
                let formato = if self.config.colunas[i] {
                    TextFormat::simple(fonte(10.0, false), branco(0x99))
                } else {
                    escondido(10.0, false)
                };
                montar(ui, LayoutJob::single_section(titulo.to_string(), formato))
            })
            .collect();
        let numeros: Vec<Arc<Galley>> = celulas(j.por_segundo, j.total, j.porcentagem, j.criticos, j.golpes, j.maximo)
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let formato = if self.config.colunas[i] {
                    TextFormat::simple(fonte(12.0, i == 0), texto())
                } else {
                    escondido(12.0, i == 0)
                };
                montar(ui, LayoutJob::single_section(t, formato))
            })
            .collect();

        let nome = montar(
            ui,
            LayoutJob::single_section(format!("▸ {} (você)", j.nome), TextFormat::simple(fonte(12.0, true), texto())),
        );
        let visiveis = self.config.perfil();
        let segmentos: Vec<Arc<Galley>> = segmentos_do_perfil(&j)
            .into_iter()
            .zip(visiveis)
            .map(|(pedacos, visivel)| {
                let mut job = LayoutJob::default();
                for (texto_pedaco, cor) in pedacos {
                    let formato = if visivel { TextFormat::simple(fonte(10.0, false), cor) } else { escondido(10.0, false) };
                    job.append(&texto_pedaco, 0.0, formato);
                }
                montar(ui, job)
            })
            .collect();
        let separador = montar(ui, LayoutJob::single_section("  ·  ".into(), TextFormat::simple(fonte(10.0, false), branco(0x77))));

        let altura_titulos = titulos.iter().fold(0.0_f32, |m, g| m.max(g.size().y));
        let altura_esquerda = nome.size().y + segmentos[0].size().y + 4.0;
        let altura_linha = altura_esquerda.max(numeros.iter().fold(0.0_f32, |m, g| m.max(g.size().y))) + 6.0;
        let (bloco, _) = ui.allocate_exact_size(vec2(largura, altura_titulos + 4.0 + altura_linha), Sense::hover());
        let faixa_titulos = Rect::from_min_size(bloco.min, vec2(largura, altura_titulos));
        let linha = Rect::from_min_max(pos2(bloco.min.x, bloco.max.y - altura_linha), bloco.max);

        // Primeiro as áreas clicáveis (o realce vai por baixo do texto), depois o desenho.
        let direitas = todas.direitas();
        let mut realces: Vec<Rect> = Vec::new();
        for i in 0..5 {
            let direita = bloco.max.x - direitas[i];
            let area = Rect::from_min_max(pos2(direita - todas.larguras[i] - 4.0, bloco.min.y), pos2(direita + 4.0, bloco.max.y));
            let acao = if self.config.colunas[i] { "Esconder" } else { "Mostrar" };
            let resposta = ui
                .interact(area, ui.id().with(("coluna", i)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!("{acao} {}", COLUNAS[i].0));
            if resposta.hovered() {
                realces.push(area);
            }
            if resposta.clicked() {
                self.config.colunas[i] = !self.config.colunas[i];
            }
        }

        let topo_esquerda = linha.min.y + (linha.height() - altura_esquerda) / 2.0;
        let y_perfil = topo_esquerda + 2.0 + nome.size().y;
        let mut x = linha.min.x + 18.0;
        let mut posicoes = Vec::new();
        for (k, segmento) in segmentos.iter().enumerate() {
            if k > 0 {
                x += separador.size().x;
            }
            let area = Rect::from_min_size(pos2(x, y_perfil), segmento.size()).expand2(vec2(4.0, 2.0));
            posicoes.push(pos2(x, y_perfil));
            let acao = if visiveis[k] { "Esconder" } else { "Mostrar" };
            let resposta = ui
                .interact(area, ui.id().with(("perfil", k)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!("{acao} {}", NOMES_DO_PERFIL[k]));
            if resposta.hovered() {
                realces.push(area);
            }
            if resposta.clicked() {
                match k {
                    0 => self.config.classe = !self.config.classe,
                    1 => self.config.nivel = !self.config.nivel,
                    _ => self.config.gs = !self.config.gs,
                }
            }
            x += segmento.size().x;
        }

        let pintor = ui.painter();
        let cor = cor_da_classe(j.classe);
        let barra = Rect::from_min_size(linha.min, vec2(linha.width() * 0.55, linha.height()));
        pintor.rect_filled(barra, 3, Color32::from_rgba_unmultiplied(cor.r(), cor.g(), cor.b(), 0x66));
        pintor.rect_stroke(linha, 3, Stroke::new(1.0_f32, branco(0x22)), StrokeKind::Inside);
        for area in realces {
            pintor.rect_filled(area, 3, branco(0x1A));
        }
        todas.pintar(pintor, faixa_titulos, titulos);
        todas.pintar(pintor, linha, numeros);
        pintor.galley(pos2(linha.min.x + 6.0, topo_esquerda + 2.0), nome, texto());
        for (k, (segmento, posicao)) in segmentos.into_iter().zip(posicoes).enumerate() {
            if k > 0 {
                pintor.galley(posicao - vec2(separador.size().x, 0.0), separador.clone(), texto());
            }
            pintor.galley(posicao, segmento, texto());
        }
    }
}

/// Nome de seção: semibold, sem caixa alta.
fn secao(ui: &mut Ui, titulo: &str) {
    ui.label(RichText::new(titulo).font(fonte(11.0, true)).color(branco(0xAA)));
}

fn dica(ui: &mut Ui, texto_dica: &str) {
    ui.add(eframe::egui::Label::new(RichText::new(texto_dica).font(fonte(10.0, false)).color(branco(0x88))).wrap());
}

fn frase(ui: &mut Ui, trecho: &str) {
    ui.label(RichText::new(trecho).font(fonte(12.0, false)).color(texto()));
    ui.add_space(6.0);
}

/// Número entre os botões − e +, centrado numa largura fixa (a frase não pula ao mudar).
fn valor(ui: &mut Ui, texto_valor: &str, largura: f32) {
    let galley = montar(ui, LayoutJob::single_section(texto_valor.to_string(), TextFormat::simple(fonte(12.0, true), texto())));
    let (rect, _) = ui.allocate_exact_size(vec2(largura, galley.size().y + 2.0), Sense::hover());
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, texto());
}

/// Botão − ou +; no limite da faixa fica apagado e não responde.
fn passo(ui: &mut Ui, simbolo: &str, habilitado: bool) -> bool {
    if habilitado {
        let clicou = botao(ui, simbolo, false).clicked();
        ui.add_space(2.0);
        return clicou;
    }
    let galley = montar(ui, LayoutJob::single_section(simbolo.to_string(), TextFormat::simple(fonte(12.0, false), branco(0x44))));
    let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(14.0, 2.0), Sense::hover());
    ui.painter().galley(rect.min + vec2(7.0, 1.0), galley, texto());
    ui.add_space(2.0);
    false
}

/// Interruptor com rótulo e dica; a linha inteira é clicável. A bolinha anima em 120 ms.
fn interruptor(ui: &mut Ui, rotulo: &str, texto_dica: &str, valor: &mut bool, acento: Color32) {
    let largura = ui.available_width();
    let titulo = montar(ui, LayoutJob::single_section(rotulo.to_string(), TextFormat::simple(fonte(12.0, false), texto())));
    let mut job = LayoutJob::single_section(texto_dica.to_string(), TextFormat::simple(fonte(10.0, false), branco(0x88)));
    job.wrap.max_width = largura - 48.0;
    let explicacao = montar(ui, job);
    let altura = titulo.size().y + explicacao.size().y + 2.0;
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, altura), Sense::click());
    let resposta = resposta.on_hover_cursor(CursorIcon::PointingHand);
    if resposta.clicked() {
        *valor = !*valor;
    }

    let t = ui.ctx().animate_bool_with_time(resposta.id, *valor, 0.12);
    let pintor = ui.painter();
    pintor.galley(rect.min, titulo.clone(), texto());
    pintor.galley(rect.min + vec2(0.0, titulo.size().y + 2.0), explicacao, texto());

    let pista = Rect::from_center_size(pos2(rect.max.x - 16.0, rect.min.y + titulo.size().y / 2.0), vec2(28.0, 16.0));
    let fundo = if resposta.hovered() { branco(0x44) } else { branco(0x33) };
    pintor.rect_filled(pista, 8, fundo.lerp_to_gamma(acento, t));
    let bolinha = pos2(pista.min.x + 8.0 + (pista.width() - 16.0) * t, pista.center().y);
    pintor.circle_filled(bolinha, 6.0, branco(0xCC).lerp_to_gamma(Color32::from_rgb(0x10, 0x14, 0x18), t));
}
