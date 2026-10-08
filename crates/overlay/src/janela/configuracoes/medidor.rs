//! Páginas do medidor: os números e quem entra na medição, a luta e o visual. Os números de cada
//! linha se escolhem clicando numa linha de amostra (a sua, ou um esboço com o seu nome): o que está
//! escondido fica no lugar, apagado e riscado.

use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{CursorIcon, Galley, Rect, RichText, Sense, Stroke, StrokeKind, TextFormat, Ui, pos2, vec2};

use super::super::{ALTURA_LINHA, Aba, Overlay, Tres, botao, branco, fonte, montar, numeros_da_linha, texto, textos};
use super::{dica, frase, interruptor, passo, secao, valor};
use crate::config::{
    self, ATUALIZACAO_MAX, ATUALIZACAO_MIN, Config, INATIVIDADE_MAX, INATIVIDADE_MIN, TRANSPARENCIA_MAX,
    TRANSPARENCIA_PADRAO, ZOOM_MAX, ZOOM_MIN,
};

const NOMES_DOS_NUMEROS: [&str; 3] = ["o total", "o por segundo", "a %"];

/// Os números como o índice mostra: "total /s %", só os ligados.
pub(super) fn resumo_jogadores(c: &Config) -> String {
    let numeros: Vec<&str> = ["total", "/s", "%"].into_iter().zip(c.numeros).filter(|(_, ligado)| *ligado).map(|(nome, _)| nome).collect();
    let mut partes = vec![if numeros.is_empty() { "sem números".to_string() } else { numeros.join(" ") }];
    if c.so_meu_dano {
        partes.push("só o seu dano".into());
    }
    if c.ocultar_nomes {
        partes.push("nomes ocultos".into());
    }
    partes.join(", ")
}

pub(super) fn resumo_luta(c: &Config) -> String {
    let mut resumo = format!("luta nova em {} s", c.inatividade);
    if c.resumo_em_linhas {
        resumo.push_str(", resumo em linhas");
    }
    resumo
}

pub(super) fn resumo_visual(c: &Config) -> String {
    format!("{:.0}%, fundo {}%, {} ms", c.zoom * 100.0, c.transparencia, c.atualizacao_ms)
}

impl Overlay {
    pub(super) fn pagina_jogadores(&mut self, ui: &mut Ui) {
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
        secao(ui, "Nomes");
        ui.add_space(4.0);
        interruptor(
            ui,
            "Ocultar nomes",
            "Os outros jogadores aparecem pelo nome da classe, no medidor, nas lutas anteriores e no resumo \
             copiado. O seu nome continua. Bom para print e live.",
            &mut self.config.ocultar_nomes,
            acento,
        );
    }

    pub(super) fn pagina_luta(&mut self, ui: &mut Ui) {
        secao(ui, "Quando a luta zera");
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
        secao(ui, "Resumo copiado");
        ui.add_space(4.0);
        let acento = self.acento();
        interruptor(
            ui,
            "Resumo em linhas",
            "\"Copiar resumo\" (o ⧉ no topo): um jogador por linha. Desligado, sai tudo numa linha só, \
             que cabe numa mensagem do chat do jogo.",
            &mut self.config.resumo_em_linhas,
            acento,
        );
    }

    pub(super) fn pagina_visual(&mut self, ui: &mut Ui) {
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
        secao(ui, "Atualização do placar");
        ui.add_space(4.0);
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resumos_do_indice_seguem_a_config() {
        let mut c = Config::default();
        assert_eq!(resumo_jogadores(&c), "total /s %");
        assert_eq!(resumo_luta(&c), "luta nova em 15 s");
        assert_eq!(resumo_visual(&c), "100%, fundo 15%, 500 ms");
        c.numeros = [false, true, false];
        c.so_meu_dano = true;
        c.ocultar_nomes = true;
        c.resumo_em_linhas = true;
        assert_eq!(resumo_jogadores(&c), "/s, só o seu dano, nomes ocultos");
        assert_eq!(resumo_luta(&c), "luta nova em 15 s, resumo em linhas");
        c.numeros = [false; 3];
        assert!(resumo_jogadores(&c).starts_with("sem números"));
    }
}
