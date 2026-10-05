//! Tela de configurações. Os números de cada linha se escolhem clicando numa linha de amostra (a
//! sua, ou um esboço com o seu nome): o que está escondido fica no lugar, apagado e riscado. O resto
//! é quem entra na medição, quando a luta zera, o tamanho e a transparência do fundo. Tudo vale na
//! hora e fica salvo.

use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Align, Color32, CursorIcon, Galley, Layout, Rect, RichText, Sense, Stroke, StrokeKind, TextFormat, Ui, pos2, vec2,
};

use super::{
    ALTURA_LINHA, Aba, Overlay, Tela, Tres, botao, branco, cor_da_classe, fonte, montar, numeros_da_linha, texto,
    textos,
};
use crate::bandeja;
use crate::config::{
    self, ATUALIZACAO_MAX, ATUALIZACAO_MIN, INATIVIDADE_MAX, INATIVIDADE_MIN, TRANSPARENCIA_MAX, TRANSPARENCIA_PADRAO,
    ZOOM_MAX, ZOOM_MIN,
};

const NOMES_DOS_NUMEROS: [&str; 3] = ["o total", "o por segundo", "a %"];

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
        secao(ui, "Números de cada jogador");
        dica(
            ui,
            "Clique num número da amostra para mostrar ou esconder; vale nas três abas. Classe, level, GS, CRIT, \
             AVG e MAX ficam no mouse e na ficha que abre com o clique no jogador.",
        );
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
        secao(ui, "Exibição");
        ui.add_space(4.0);
        interruptor(
            ui,
            "Ocultar nomes",
            "Os outros jogadores aparecem pelo nome da classe, no medidor, nas lutas anteriores e no resumo \
             copiado. O seu nome continua. Bom para print e live.",
            &mut self.config.ocultar_nomes,
            acento,
        );
        ui.add_space(6.0);
        interruptor(
            ui,
            "Resumo em linhas",
            "\"Copiar resumo\" (o ⧉ no topo): um jogador por linha. Desligado, sai tudo numa linha só, \
             que cabe numa mensagem do chat do jogo.",
            &mut self.config.resumo_em_linhas,
            acento,
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            frase(ui, "Atualizar a cada");
            let ms = self.config.atualizacao_ms;
            if passo(ui, "−", ms > ATUALIZACAO_MIN) {
                self.config.atualizacao_ms = (ms - 100).max(ATUALIZACAO_MIN);
            }
            valor(ui, &format!("{ms} ms"), 56.0);
            if passo(ui, "+", ms < ATUALIZACAO_MAX) {
                self.config.atualizacao_ms = (ms + 100).min(ATUALIZACAO_MAX);
            }
        });
        dica(ui, "Menos ms: o placar anda mais liso e o Axon usa um pouco mais de CPU.");

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

        ui.add_space(14.0);
        secao(ui, "Transparência do fundo");
        dica(ui, "Texto e barras continuam opacos.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let t = self.config.transparencia;
            if passo(ui, "−", t > 0) {
                self.config.transparencia = t.saturating_sub(5);
            }
            valor(ui, &format!("{t}%"), 48.0);
            if passo(ui, "+", t < TRANSPARENCIA_MAX) {
                self.config.transparencia = (t + 5).min(TRANSPARENCIA_MAX);
            }
            if t != TRANSPARENCIA_PADRAO {
                ui.add_space(6.0);
                let volta = format!("Volta a {TRANSPARENCIA_PADRAO}%");
                if botao(ui, "Restaurar", false).on_hover_text(volta).clicked() {
                    self.config.transparencia = TRANSPARENCIA_PADRAO;
                }
            }
        });

        ui.add_space(14.0);
        self.atalhos(ui);
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

    /// Os atalhos globais, lidos do config.json na abertura. O overlay não recebe teclado (não tira o
    /// foco do jogo), então a troca é pelo arquivo.
    fn atalhos(&self, ui: &mut Ui) {
        secao(ui, "Atalhos");
        dica(ui, "Valem com o jogo na frente; fora dele, a combinação volta para os outros programas.");
        ui.add_space(4.0);
        let c = &self.config;
        let escritos = [&c.atalho_mostrar, &c.atalho_atravessar, &c.atalho_resumo, &c.atalho_compacta];
        let acoes = [
            (bandeja::MOSTRAR, "mostra ou esconde o overlay"),
            (bandeja::ALTERNAR_CLIQUE, "o clique atravessa o overlay até o jogo, ou volta"),
            (bandeja::COPIAR_RESUMO, "copia o resumo da luta"),
            (bandeja::ALTERNAR_COMPACTA, "barra compacta, ou volta ao medidor"),
        ];
        for (qual, acao) in acoes {
            let (tecla, cor) = match bandeja::atalho(qual) {
                Some((atalho, false)) => (atalho.to_string(), texto()),
                Some((atalho, true)) => {
                    (format!("{atalho} (em uso por outro programa)"), Color32::from_rgb(0xFF, 0x6B, 0x6B))
                }
                None if escritos[qual].trim().is_empty() => ("desligado".to_string(), branco(0x77)),
                None => (format!("\"{}\" não vale", escritos[qual].trim()), Color32::from_rgb(0xFF, 0x6B, 0x6B)),
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(tecla).font(fonte(12.0, true)).color(cor));
                ui.add_space(8.0);
                ui.label(RichText::new(acao).font(fonte(12.0, false)).color(texto()));
            });
        }
        ui.add_space(2.0);
        dica(
            ui,
            "Para trocar, feche o Axon e edite atalho_mostrar, atalho_atravessar, atalho_resumo e \
             atalho_compacta no config.json (%LOCALAPPDATA%\\Aion2Meter). Ex.: \"Ctrl+Shift+F9\"; precisa de \
             Ctrl, Alt ou Win; vazio desliga (o resumo e a barra compacta vêm desligados).",
        );
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

    /// A sua linha como aparece na aba DPS; cada um dos três números é um botão.
    fn linha_de_amostra(&mut self, ui: &mut Ui) {
        let Some(j) = self.amostra.clone() else { return };
        let largura = ui.available_width();
        let todas = Tres::medir(ui, [true; 3]);
        let ligados = self.config.numeros;
        let escondido = |tamanho: f32| TextFormat {
            font_id: fonte(tamanho, false),
            color: branco(0x55),
            strikethrough: Stroke::new(1.0_f32, branco(0x77)),
            ..Default::default()
        };
        let [a, b, c] = numeros_da_linha(ui, textos(j.total, j.por_segundo, j.porcentagem), 12.0);
        let mut normais = [Some(a), Some(b), Some(c)];
        let numeros_texto = textos(j.total, j.por_segundo, j.porcentagem);
        let numeros: [Arc<Galley>; 3] = std::array::from_fn(|i| match normais[i].take() {
            Some(g) if ligados[i] => g,
            _ => montar(ui, LayoutJob::single_section(numeros_texto[i].clone(), escondido(12.0))),
        });
        let legendas: [Arc<Galley>; 3] = std::array::from_fn(|i| {
            let formato = if ligados[i] { TextFormat::simple(fonte(9.5, false), branco(0x88)) } else { escondido(9.5) };
            montar(ui, LayoutJob::single_section(Aba::Dps.rotulos()[i].to_string(), formato))
        });

        let altura_legenda = legendas.iter().fold(0.0_f32, |m, g| m.max(g.size().y));
        let (bloco, _) = ui.allocate_exact_size(vec2(largura, altura_legenda + 3.0 + ALTURA_LINHA), Sense::hover());
        let faixa = Rect::from_min_size(bloco.min, vec2(largura, altura_legenda));
        let linha = Rect::from_min_max(pos2(bloco.min.x, bloco.max.y - ALTURA_LINHA), bloco.max);

        let direitas = todas.direitas();
        let mut realces: Vec<Rect> = Vec::new();
        for i in 0..3 {
            let direita = bloco.max.x - direitas[i];
            let area = Rect::from_min_max(pos2(direita - todas.larguras[i] - 4.0, bloco.min.y), pos2(direita + 4.0, bloco.max.y));
            let acao = if ligados[i] { "Esconder" } else { "Mostrar" };
            let resposta = ui
                .interact(area, ui.id().with(("numero", i)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!("{acao} {}", NOMES_DOS_NUMEROS[i]));
            if resposta.hovered() {
                realces.push(area);
            }
            if resposta.clicked() {
                self.config.numeros[i] = !ligados[i];
            }
        }

        self.pintar_linha(ui, linha, &j, None, 0.55, numeros, &todas, false);
        let pintor = ui.painter();
        for area in realces {
            pintor.rect_filled(area, 3, branco(0x1A));
        }
        todas.pintar(pintor, faixa, legendas, false);
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
