//! Lutas anteriores: as últimas que acabaram nesta execução do Axon (só na memória), da mais nova
//! para a mais velha. Clicar numa abre o placar dela no lugar da luta de agora, com as mesmas abas
//! e skills; "Ao vivo" no cabeçalho volta.

use eframe::egui::text::LayoutJob;
use eframe::egui::{Align, Color32, CursorIcon, Layout, RichText, Sense, Ui, pos2, vec2};
use nucleo::Hora;
use nucleo::formato::p;
use nucleo::medicao::medidor::{LUTAS_GUARDADAS, LutaPassada};

use super::{
    MARGEM_DIREITA, Overlay, Tela, botao, branco, compacto, cor_da_classe, fonte, hora_local, minutos_e_segundos,
    montar, nome_oculto, texto, trecho, uma_linha, visual,
};

/// O que a lista mostra de cada luta: hora, duração, dano total e o primeiro do DPS.
pub(super) struct ResumoLuta {
    numero: u64,
    inicio: Hora,
    duracao: i64,
    total: f64,
    jogadores: usize,
    /// Nome, classe e parte do dano de quem mais bateu.
    primeiro: Option<(String, &'static str, f64)>,
    /// Nome do alvo da luta (o chefe ou o mob que mais apanhou), se o questlog tiver.
    alvo: Option<String>,
    /// Quantas vezes você morreu nela.
    mortes: usize,
}

impl ResumoLuta {
    pub(super) fn de(luta: &LutaPassada, ocultar_nomes: bool) -> Self {
        let dano = &luta.placar.dano;
        Self {
            numero: luta.numero,
            inicio: luta.inicio,
            duracao: luta.placar.duracao,
            total: dano.total,
            jogadores: dano.jogadores.len(),
            primeiro: dano.jogadores.first().map(|j| {
                let nome = if ocultar_nomes && !j.voce { nome_oculto(j.classe) } else { j.nome.clone() };
                (nome, j.classe, j.porcentagem)
            }),
            alvo: luta.placar.alvo.as_ref().map(|a| a.nome.clone()).filter(|nome| !nome.is_empty()),
            mortes: luta.placar.mortes.len(),
        }
    }
}

impl Overlay {
    pub(super) fn tela_lutas(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Lutas anteriores").font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "Voltar", true).on_hover_text("Volta ao medidor").clicked() {
                    self.tela = Tela::Medidor;
                }
            });
        });
        ui.add_space(4.0);
        let explicacao = format!(
            "As {LUTAS_GUARDADAS} últimas desta execução; fechar o Axon apaga. Uma luta acaba quando os mobs dela \
             saem de combate, depois de {} s sem dano ou no Zerar. Clique numa para ver o placar dela.",
            self.config.inatividade
        );
        ui.add(eframe::egui::Label::new(RichText::new(explicacao).font(fonte(10.0, false)).color(branco(0x88))).wrap());
        ui.add_space(6.0);

        if self.lutas.is_empty() {
            let vazio = RichText::new("Nenhuma luta terminou ainda.")
                .font(fonte(12.0, false))
                .color(texto().gamma_multiply(0.6));
            ui.label(vazio);
            return;
        }
        let vista = self.vendo.map(|(numero, _)| numero);
        let mut escolhida = None;
        for luta in &self.lutas {
            if linha_luta(ui, luta, vista == Some(luta.numero)) {
                escolhida = Some((luta.numero, luta.inicio));
            }
        }
        if escolhida.is_some() {
            self.ver(escolhida);
        }
    }
}

/// "14:32   01:45   1,23M   Yoshi 45%" e, à direita, o alvo (ou quantos jogadores): a luta aberta
/// agora fica destacada. Devolve o clique.
fn linha_luta(ui: &mut Ui, luta: &ResumoLuta, vista: bool) -> bool {
    let largura = ui.available_width();
    let mut job = LayoutJob::default();
    trecho(&mut job, &hora_local(luta.inicio), 12.0, true, texto());
    trecho(&mut job, &format!("   {}   ", minutos_e_segundos(luta.duracao)), 12.0, false, branco(0xBB));
    trecho(&mut job, &compacto(luta.total), 12.0, true, texto());
    if let Some((nome, classe, parte)) = &luta.primeiro {
        trecho(&mut job, "   ", 12.0, false, texto());
        trecho(&mut job, nome, 12.0, false, cor_da_classe(classe));
        trecho(&mut job, &format!(" {}", p(*parte, 0)), 12.0, false, branco(0xBB));
    }
    match luta.mortes {
        0 => {}
        1 => trecho(&mut job, "   ☠", 11.0, true, Color32::from_rgb(0xFF, 0x8B, 0x8B)),
        m => trecho(&mut job, &format!("   ☠{m}"), 11.0, true, Color32::from_rgb(0xFF, 0x8B, 0x8B)),
    }
    let jogadores = if luta.jogadores == 1 { "1 jogador".to_string() } else { format!("{} jogadores", luta.jogadores) };
    let mut direita = LayoutJob::default();
    match &luta.alvo {
        Some(alvo) => trecho(&mut direita, alvo, 10.0, true, visual::DOURADO),
        None => trecho(&mut direita, &jogadores, 10.0, false, branco(0x88)),
    }
    direita.wrap = uma_linha(largura * 0.4);
    let direita = montar(ui, direita);
    job.wrap = uma_linha((largura - 12.0 - direita.size().x - MARGEM_DIREITA - 8.0).max(40.0));
    let esquerda = montar(ui, job);

    let altura = esquerda.size().y + 6.0;
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, altura), Sense::click());
    let resposta = resposta
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(if luta.mortes > 0 {
            format!("Ver o placar desta luta ({jogadores}). ☠: você morreu nela; o card da morte abre o relatório.")
        } else {
            format!("Ver o placar desta luta ({jogadores})")
        });
    let fundo = if resposta.hovered() {
        Some(branco(0x33))
    } else if vista {
        Some(branco(0x22))
    } else {
        None
    };
    let pintor = ui.painter();
    if let Some(cor) = fundo {
        pintor.rect_filled(rect, 3, cor);
    }
    pintor.galley(pos2(rect.min.x + 6.0, rect.center().y - esquerda.size().y / 2.0), esquerda, texto());
    let x = rect.max.x - MARGEM_DIREITA - direita.size().x;
    pintor.galley(pos2(x, rect.center().y - direita.size().y / 2.0), direita, texto());
    resposta.clicked()
}
