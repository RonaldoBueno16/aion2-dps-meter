//! Relatório da sua morte no overlay: o card abaixo do alvo (por `card_morte_s` desde que o overlay
//! viu a morte; numa luta passada, parado), a tela com o golpe final e os últimos segundos, o link na
//! sua linha da aba Tank e o "☠ 4.926 <mob>" da barra compacta.

use std::sync::Arc;
use std::time::Duration;

use eframe::egui::load::SizedTexture;
use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align, Color32, CursorIcon, Layout, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextFormat, Ui,
    Vec2, pos2, vec2,
};
use nucleo::formato::{f, n};
use nucleo::medicao::medidor::{LinhaMorte, RelatorioMorte, TipoRecebido};

use super::{
    MARGEM_DIREITA, Overlay, Tela, botao, branco, cor_da_classe, fonte, hora_local_s, montar, nome_oculto, texto,
    trecho, uma_linha, visual,
};

/// O do "☠N" da aba Tank.
const VERMELHO: Color32 = Color32::from_rgb(0xFF, 0x8B, 0x8B);
const VERDE: Color32 = Color32::from_rgb(0x5B, 0xD1, 0x6B);
const ALTURA_CARD: f32 = 56.0;
/// Daí para baixo a lista rola: com 30 s e 128 eventos, a janela passaria da tela.
const ALTURA_LISTA: f32 = 300.0;

/// Onde o card aparece: no medidor ao vivo (com ✕ e prazo), numa luta passada (parado) ou no topo da
/// tela do relatório (sem clique).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    AoVivo,
    Parado,
    NaTela,
}

/// Linhas seguidas do mesmo autor, skill, tipo e valor, no mesmo décimo de segundo, viram uma, com o
/// HP da última ("Efeito 2011101 (2×)"). O golpe final fica sempre sozinho.
pub(super) fn agrupar(linhas: &[LinhaMorte]) -> Vec<(&LinhaMorte, usize, Option<u64>)> {
    let decimo = |l: &LinhaMorte| (l.antes_s * 10.0).round() as i64;
    let mut grupos: Vec<(&LinhaMorte, usize, Option<u64>)> = Vec::new();
    for l in linhas {
        if let Some((primeira, vezes, hp)) = grupos.last_mut()
            && !l.golpe_final
            && !primeira.golpe_final
            && (primeira.autor, primeira.skill, primeira.tipo, primeira.valor) == (l.autor, l.skill, l.tipo, l.valor)
            && decimo(primeira) == decimo(l)
        {
            *vezes += 1;
            *hp = l.hp_depois;
            continue;
        }
        grupos.push((l, 1, l.hp_depois));
    }
    grupos
}

/// "Ocultar nomes" no relatório: o matador jogador e quem curou ou bateu em você (PvP) pela classe;
/// você e os monstros continuam.
pub(super) fn ocultar_na_morte(r: &mut RelatorioMorte) {
    if r.matador_jogador {
        r.nome_matador = nome_oculto(r.classe_matador);
    }
    let morto = r.morto;
    let de_jogador = |l: &&mut LinhaMorte| matches!(l.tipo, TipoRecebido::Cura | TipoRecebido::Efeito) && l.autor != morto;
    for l in r.linhas.iter_mut().filter(de_jogador) {
        l.quem = nome_oculto(l.classe);
    }
}

/// A barra compacta no lugar do alvo: "☠ 4.926 <mob>" (sem o dano, só o matador).
pub(super) fn texto_compacto(r: &RelatorioMorte) -> String {
    match r.dano_final {
        Some(dano) => format!("☠ {} {}", n(dano as f64, 0), r.nome_matador),
        None => format!("☠ {}", r.nome_matador),
    }
}

/// Embaixo do golpe final: "HP antes do golpe 4.850 (passou 76)  ·  2 golpes do monstro em 4,5 s".
pub(super) fn resumo_do_card(r: &RelatorioMorte) -> String {
    let mut partes = Vec::new();
    match (r.hp_antes_final, r.dano_final) {
        (Some(hp), Some(dano)) if dano >= hp => {
            partes.push(format!("HP antes do golpe {} (passou {})", n(hp as f64, 0), n((dano - hp) as f64, 0)));
        }
        (Some(hp), _) => partes.push(format!("HP antes do golpe {}", n(hp as f64, 0))),
        (None, _) => {}
    }
    if r.matador_jogador {
        partes.push("golpe de jogador: o dano não vem no pacote".into());
    }
    let golpes: Vec<&LinhaMorte> = r.linhas.iter().filter(|l| com_valor(l.tipo)).collect();
    if let Some(primeiro) = golpes.first() {
        let quantos = if golpes.len() == 1 { "1 golpe".to_string() } else { format!("{} golpes", golpes.len()) };
        let de = if r.monstros == 1 { " do monstro".to_string() } else { format!(" de {} monstros", r.monstros) };
        partes.push(format!("{quantos}{de} em {} s", f(-primeiro.antes_s, 1)));
    }
    partes.join("  ·  ")
}

fn com_valor(tipo: TipoRecebido) -> bool {
    matches!(tipo, TipoRecebido::Golpe | TipoRecebido::Periodico)
}

/// "-2.386" no golpe, "+1.306" na cura, "sem valor" no efeito de jogador.
fn valor_da_linha(l: &LinhaMorte, vezes: usize) -> (String, Color32) {
    let total = l.valor.map(|v| v * vezes as u64);
    match (l.tipo, total) {
        (TipoRecebido::Cura, Some(v)) => (format!("+{}", n(v as f64, 0)), VERDE),
        (_, Some(v)) => (format!("-{}", n(v as f64, 0)), visual::VERMELHO_CLARO),
        (_, None) => ("sem valor".into(), branco(0x77)),
    }
}

impl Overlay {
    /// A morte do card agora: ao vivo, a última, por `card_morte_s` desde que o overlay a viu; numa
    /// luta passada, a última dela, parada.
    pub(super) fn morte_do_card(&self) -> Option<Arc<RelatorioMorte>> {
        if !self.config.relatorio_morte {
            return None;
        }
        if self.vendo.is_some() {
            return self.placar.mortes.last().cloned();
        }
        let desde = self.card_morte?;
        (desde.elapsed() < Duration::from_secs(u64::from(self.config.card_morte_s))).then(|| self.morte.clone()).flatten()
    }

    pub(super) fn abrir_morte(&mut self, numero: u64) {
        self.tela = Tela::Morte(numero);
        self.ler_placar();
    }

    /// O card no medidor, entre o alvo e as abas.
    pub(super) fn card_da_morte(&mut self, ui: &mut Ui) {
        let Some(r) = self.morte_do_card() else { return };
        let modo = if self.vendo.is_some() { Modo::Parado } else { Modo::AoVivo };
        let (abrir, fechar) = self.desenhar_card(ui, &r, modo);
        if fechar {
            self.card_morte = None;
        } else if abrir {
            self.abrir_morte(r.numero);
        }
        ui.add_space(6.0);
    }

    /// Título, matador com o golpe e o dano, e o resumo. Devolve (abrir o relatório, fechar).
    fn desenhar_card(&mut self, ui: &mut Ui, r: &RelatorioMorte, modo: Modo) -> (bool, bool) {
        let largura = ui.available_width();
        let sentido = if modo == Modo::NaTela { Sense::hover() } else { Sense::click() };
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, ALTURA_CARD), sentido);
        let pintor = ui.painter().clone();
        let forte = Color32::from_rgba_unmultiplied(0x5A, 0x10, 0x16, 0xF0);
        let fraco = Color32::from_rgba_unmultiplied(0x1E, 0x08, 0x0B, 0xE6);
        visual::degrade(&pintor, rect, 6.0, forte, fraco);
        let borda = if resposta.hovered() && modo != Modo::NaTela { VERMELHO } else { VERMELHO.gamma_multiply(0.55) };
        pintor.rect_stroke(rect, 6, Stroke::new(1.0_f32, borda), StrokeKind::Inside);
        let (linha1, linha2, linha3) = (rect.min.y + 11.0, rect.min.y + 28.0, rect.min.y + 45.0);

        // ✕ só ao vivo: depois do card, para ganhar o clique em cima dele.
        let mut fechar = false;
        let mut direita = rect.max.x - 10.0;
        if modo == Modo::AoVivo {
            let xis = Rect::from_center_size(pos2(rect.max.x - 13.0, linha1), Vec2::splat(18.0));
            let clique = ui
                .interact(xis, ui.id().with(("fechar_morte", r.numero)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Fechar");
            if clique.hovered() {
                pintor.rect_filled(xis, 3, branco(0x22));
            }
            let galley = montar(ui, LayoutJob::single_section("✕".into(), TextFormat::simple(fonte(10.0, false), branco(0xAA))));
            pintor.galley(xis.center() - galley.size() / 2.0, galley, texto());
            fechar = clique.clicked();
            direita = xis.min.x - 4.0;
        }

        let mut titulo = LayoutJob::default();
        match modo {
            Modo::NaTela => trecho(&mut titulo, "Golpe final", 10.0, true, branco(0xBB)),
            _ => {
                trecho(&mut titulo, "☠ Você morreu", 11.0, true, VERMELHO);
                trecho(&mut titulo, &format!("  {}", hora_local_s(r.hora)), 10.0, false, branco(0xBB));
            }
        }
        let titulo = montar(ui, titulo);
        let esquerda = rect.min.x + 10.0;
        pintor.galley(pos2(esquerda, linha1 - titulo.size().y / 2.0), titulo, texto());
        if modo != Modo::NaTela {
            let link = TextFormat::simple(fonte(10.0, true), visual::DOURADO);
            let link = montar(ui, LayoutJob::single_section("ver relatório ›".into(), link));
            pintor.galley(pos2(direita - link.size().x, linha1 - link.size().y / 2.0), link, texto());
        }

        // Linha do meio: retrato (ou ⚔), quem matou e o golpe; o dano à direita.
        let circulo = Rect::from_center_size(pos2(esquerda + 8.0, linha2), Vec2::splat(16.0));
        match r.retrato_matador.as_deref().and_then(|caminho| self.textura(ui.ctx(), caminho)) {
            Some(textura) => {
                egui::Image::new(SizedTexture::new(textura.id(), circulo.size())).corner_radius(8).paint_at(ui, circulo);
            }
            None => {
                let galley = montar(ui, LayoutJob::single_section("⚔".into(), TextFormat::simple(fonte(11.0, false), branco(0x99))));
                pintor.galley(circulo.center() - galley.size() / 2.0, galley, texto());
            }
        }
        let dano = match r.dano_final {
            Some(dano) => montar(
                ui,
                LayoutJob::single_section(n(dano as f64, 0), TextFormat::simple(fonte(13.0, true), visual::VERMELHO_CLARO)),
            ),
            None => montar(ui, LayoutJob::single_section("sem o dano".into(), TextFormat::simple(fonte(10.0, false), branco(0x88)))),
        };
        let x_dano = rect.max.x - 10.0 - dano.size().x;
        pintor.galley(pos2(x_dano, linha2 - dano.size().y / 2.0), dano, texto());
        let mut quem = LayoutJob::default();
        let cor = if r.matador_jogador { cor_da_classe(r.classe_matador) } else { texto() };
        trecho(&mut quem, &r.nome_matador, 12.0, true, cor);
        trecho(&mut quem, &format!("   {}", r.nome_skill_final), 10.0, false, branco(0xBB));
        let inicio = circulo.max.x + 6.0;
        quem.wrap = uma_linha((x_dano - 8.0 - inicio).max(40.0));
        let quem = montar(ui, quem);
        visual::com_sombra(&pintor, pos2(inicio, linha2 - quem.size().y / 2.0), quem);

        let mut resumo = LayoutJob::single_section(resumo_do_card(r), TextFormat::simple(fonte(10.0, false), branco(0xCC)));
        resumo.wrap = uma_linha(rect.width() - 20.0);
        let resumo = montar(ui, resumo);
        pintor.galley(pos2(esquerda, linha3 - resumo.size().y / 2.0), resumo, texto());

        let abrir = modo != Modo::NaTela
            && resposta
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Abrir o relatório: os últimos segundos antes da morte")
                .clicked();
        (abrir && !fechar, fechar)
    }

    /// Na sua linha expandida da aba Tank: "☠ Morte às 14:32:05" para cada morte da luta.
    pub(super) fn mortes_na_linha(&mut self, ui: &mut Ui) {
        let mortes = self.placar.mortes.clone();
        for r in mortes {
            let largura = ui.available_width();
            let mut job = LayoutJob::default();
            trecho(&mut job, &format!("☠ Morte às {}", hora_local_s(r.hora)), 11.0, true, VERMELHO);
            trecho(&mut job, &format!("   {}", texto_compacto(&r).trim_start_matches("☠ ")), 10.0, false, branco(0xBB));
            trecho(&mut job, "  ›", 11.0, true, visual::DOURADO);
            job.wrap = uma_linha((largura - 26.0).max(40.0));
            let galley = montar(ui, job);
            let (rect, resposta) = ui.allocate_exact_size(vec2(largura, galley.size().y + 6.0), Sense::click());
            let resposta = resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Abrir o relatório da morte");
            if resposta.hovered() {
                ui.painter().rect_filled(rect, 3, branco(0x1A));
            }
            ui.painter().galley(pos2(rect.min.x + 18.0, rect.min.y + 3.0), galley, texto());
            if resposta.clicked() {
                self.abrir_morte(r.numero);
            }
        }
    }

    /// O relatório: golpe final, os eventos dos últimos segundos com o HP depois de cada um e o resumo.
    pub(super) fn tela_morte(&mut self, ui: &mut Ui) {
        let Some(r) = self.relatorio.clone() else {
            self.tela = Tela::Medidor;
            return;
        };
        ui.horizontal(|ui| {
            let titulo = format!("Morte às {}", hora_local_s(r.hora));
            ui.label(RichText::new(titulo).font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "Voltar", true).on_hover_text("Volta ao medidor").clicked() {
                    self.tela = Tela::Medidor;
                }
            });
        });
        ui.add_space(6.0);
        self.desenhar_card(ui, &r, Modo::NaTela);
        ui.add_space(8.0);

        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
        let formato = TextFormat::simple(fonte(10.0, false), branco(0x99));
        let titulo = montar(ui, LayoutJob::single_section(format!("Últimos {} s", self.config.janela_morte_s), formato.clone()));
        let hp = montar(ui, LayoutJob::single_section("HP depois".into(), formato));
        ui.painter().galley(pos2(rect.min.x + 4.0, rect.min.y), titulo, texto());
        ui.painter().galley(pos2(rect.max.x - MARGEM_DIREITA - hp.size().x, rect.min.y), hp, texto());

        if r.linhas.is_empty() {
            let vazio = format!("Nada chegou em você nos {} s antes da morte.", self.config.janela_morte_s);
            ui.label(RichText::new(vazio).font(fonte(11.0, false)).color(branco(0x88)));
        }
        ScrollArea::vertical().max_height(ALTURA_LISTA).auto_shrink([false, true]).show(ui, |ui| {
            for (l, vezes, hp) in agrupar(&r.linhas) {
                self.linha_da_morte(ui, l, vezes, hp);
            }
        });

        ui.add_space(6.0);
        let mut job = LayoutJob::default();
        trecho(&mut job, "Recebido ", 10.0, false, branco(0xBB));
        trecho(&mut job, &n(r.recebido as f64, 0), 11.0, true, visual::VERMELHO_CLARO);
        trecho(&mut job, &format!("  ·  maior golpe {}", n(r.maior as f64, 0)), 10.0, false, branco(0xBB));
        let monstros = if r.monstros == 1 { "1 monstro".to_string() } else { format!("{} monstros", r.monstros) };
        trecho(&mut job, &format!("  ·  {monstros}  ·  curas "), 10.0, false, branco(0xBB));
        trecho(&mut job, &n(r.curado as f64, 0), 11.0, true, VERDE);
        job.wrap = uma_linha(ui.available_width() - 8.0);
        let resumo = montar(ui, job);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), resumo.size().y + 4.0), Sense::hover());
        ui.painter().galley(pos2(rect.min.x + 4.0, rect.min.y + 2.0), resumo, texto());
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                RichText::new(
                    "Skill de monstro não tem nome em nenhuma base pública: vai o código. Efeito de jogador (regeneração, \
                     poção, golpe de PvP) entra sem valor, porque o número dele não é dano. O HP é o que o servidor \
                     mandou logo depois de cada evento.",
                )
                .font(fonte(10.0, false))
                .color(branco(0x88)),
            )
            .wrap(),
        );
    }

    /// "-4,5 s  [ícone] quem  golpe (2×)  -2.386  2.612"; o golpe final em destaque.
    fn linha_da_morte(&mut self, ui: &mut Ui, l: &LinhaMorte, vezes: usize, hp: Option<u64>) {
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 20.0), Sense::hover());
        let pintor = ui.painter().clone();
        if l.golpe_final {
            pintor.rect_filled(rect, 3, Color32::from_rgba_unmultiplied(0x8E, 0x1B, 0x22, 0x70));
            pintor.rect_stroke(rect, 3, Stroke::new(1.0_f32, visual::DOURADO.gamma_multiply(0.6)), StrokeKind::Inside);
        } else if resposta.hovered() {
            pintor.rect_filled(rect, 3, branco(0x14));
        }
        let meio = rect.center().y;
        let simples = |texto: String, tamanho: f32, forte: bool, cor: Color32| {
            LayoutJob::single_section(texto, TextFormat::simple(fonte(tamanho, forte), cor))
        };

        let tempo = montar(ui, simples(format!("{} s", f(l.antes_s, 1)), 10.0, false, branco(0x99)));
        pintor.galley(pos2(rect.min.x + 44.0 - tempo.size().x, meio - tempo.size().y / 2.0), tempo, texto());

        let quadrado = Rect::from_center_size(pos2(rect.min.x + 58.0, meio), Vec2::splat(16.0));
        pintor.rect_filled(quadrado, 3, branco(0x22));
        if let Some(textura) = l.icone.as_deref().or(l.retrato.as_deref()).and_then(|c| self.textura(ui.ctx(), c)) {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            pintor.image(textura.id(), quadrado, uv, Color32::WHITE);
        }

        let hp_texto = montar(ui, simples(hp.map_or_else(|| "?".into(), |hp| n(hp as f64, 0)), 11.0, false, texto()));
        let x_hp = rect.max.x - MARGEM_DIREITA - hp_texto.size().x;
        pintor.galley(pos2(x_hp, meio - hp_texto.size().y / 2.0), hp_texto, texto());
        let (valor, cor) = valor_da_linha(l, vezes);
        let valor = montar(ui, simples(valor, 11.0, com_valor(l.tipo), cor));
        let x_valor = rect.max.x - MARGEM_DIREITA - 58.0 - valor.size().x;
        pintor.galley(pos2(x_valor, meio - valor.size().y / 2.0), valor, texto());

        let cor_quem = match l.quem.as_str() {
            "você" => visual::DOURADO,
            _ if l.classe.is_empty() => texto(),
            _ => cor_da_classe(l.classe),
        };
        let mut quem = simples(l.quem.clone(), 11.0, true, cor_quem);
        let inicio_quem = quadrado.max.x + 6.0;
        quem.wrap = uma_linha(110.0);
        let quem = montar(ui, quem);
        let largura_quem = quem.size().x;
        pintor.galley(pos2(inicio_quem, meio - quem.size().y / 2.0), quem, texto());

        let mut golpe = LayoutJob::default();
        trecho(&mut golpe, &l.nome_skill, 11.0, false, branco(0xCC));
        if vezes > 1 {
            trecho(&mut golpe, &format!(" ({vezes}×)"), 10.0, true, branco(0xAA));
        }
        let inicio_golpe = inicio_quem + largura_quem + 8.0;
        golpe.wrap = uma_linha((x_valor - 8.0 - inicio_golpe).max(30.0));
        let golpe = montar(ui, golpe);
        pintor.galley(pos2(inicio_golpe, meio - golpe.size().y / 2.0), golpe, texto());

        let mut dica = format!("{}: {}", l.quem, l.nome_skill);
        if vezes > 1 {
            dica += &format!(", {vezes} vezes");
        }
        let detalhes: Vec<&str> = [
            (l.tipo == TipoRecebido::Periodico, "dano periódico"),
            (l.tipo == TipoRecebido::Efeito, "efeito de jogador, sem valor"),
            (l.critico, "crítico"),
            (l.aparo, "aparado"),
            (l.golpe_final, "golpe final"),
        ]
        .into_iter()
        .filter_map(|(sim, texto)| sim.then_some(texto))
        .collect();
        if !detalhes.is_empty() {
            dica += &format!(" ({})", detalhes.join(", "));
        }
        resposta.on_hover_text(dica);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use TipoRecebido::{Cura, Efeito, Golpe};

    fn linha(antes_s: f64, autor: u32, quem: &str, skill: u32, valor: Option<u64>, tipo: TipoRecebido, hp: u64) -> LinhaMorte {
        LinhaMorte {
            antes_s,
            autor,
            quem: quem.into(),
            classe: "",
            retrato: None,
            skill,
            nome_skill: String::new(),
            icone: None,
            valor,
            tipo,
            critico: false,
            aparo: false,
            hp_depois: Some(hp),
            golpe_final: false,
        }
    }

    /// As 7 linhas da morte do #16201 (os 19 pacotes do teste do núcleo).
    fn morte_do_16201() -> RelatorioMorte {
        let mut linhas = vec![
            linha(-4.451, 21799, "Axios", 1235300, Some(2386), Golpe, 2612),
            linha(-4.201, 16201, "você", 18170000, Some(1306), Cura, 3918),
            linha(-3.501, 16201, "você", 2011101, None, Efeito, 4374),
            linha(-3.501, 16201, "você", 2011101, None, Efeito, 4374),
            linha(-1.502, 16201, "você", 2011101, None, Efeito, 4830),
            linha(-1.502, 16201, "você", 2011101, None, Efeito, 4830),
            linha(-0.040, 21799, "Axios", 1235330, Some(4926), Golpe, 0),
        ];
        linhas[6].golpe_final = true;
        RelatorioMorte {
            numero: 1,
            hora: 0,
            morto: 16201,
            matador: 21799,
            matador_jogador: false,
            nome_matador: "Axios".into(),
            classe_matador: "",
            npc_matador: 2400425,
            retrato_matador: None,
            skill_final: 1235330,
            nome_skill_final: "Golpe 1235330".into(),
            dano_final: Some(4926),
            hp_antes_final: Some(4850),
            linhas,
            recebido: 7312,
            maior: 4926,
            monstros: 1,
            curado: 1306,
        }
    }

    #[test]
    fn efeitos_iguais_no_mesmo_instante_viram_uma_linha_e_o_golpe_final_fica_sozinho() {
        let r = morte_do_16201();
        let grupos = agrupar(&r.linhas);
        let resumo: Vec<(f64, usize, Option<u64>)> = grupos.iter().map(|(l, vezes, hp)| (l.antes_s, *vezes, *hp)).collect();
        assert_eq!(
            resumo,
            [
                (-4.451, 1, Some(2612)),
                (-4.201, 1, Some(3918)),
                (-3.501, 2, Some(4374)),
                (-1.502, 2, Some(4830)),
                (-0.040, 1, Some(0))
            ]
        );
        assert!(grupos[4].0.golpe_final);

        // Dois golpes iguais seguidos, o segundo sendo o final: não se juntam.
        let mut dois = vec![linha(-0.04, 9, "Mob", 7, Some(10), Golpe, 5), linha(-0.04, 9, "Mob", 7, Some(10), Golpe, 0)];
        dois[1].golpe_final = true;
        assert_eq!(agrupar(&dois).len(), 2);
        assert_eq!(valor_da_linha(&dois[0], 2), ("-20".to_string(), visual::VERMELHO_CLARO));
    }

    #[test]
    fn card_diz_o_hp_antes_o_excesso_e_quantos_golpes() {
        let r = morte_do_16201();
        assert_eq!(resumo_do_card(&r), "HP antes do golpe 4.850 (passou 76)  ·  2 golpes do monstro em 4,5 s");
        assert_eq!(texto_compacto(&r), "☠ 4.926 Axios");
        let pvp = RelatorioMorte { dano_final: None, hp_antes_final: None, linhas: Vec::new(), matador_jogador: true, ..r };
        assert_eq!(resumo_do_card(&pvp), "golpe de jogador: o dano não vem no pacote");
        assert_eq!(texto_compacto(&pvp), "☠ Axios");
    }

    #[test]
    fn ocultar_nomes_troca_o_matador_jogador_e_quem_curou_e_guarda_voce_e_o_monstro() {
        let mut r = morte_do_16201();
        r.matador_jogador = true;
        r.nome_matador = "Beltrano".into();
        r.classe_matador = "Gladiator";
        let mut cura = linha(-2.0, 22222, "Fulano", 17120000, Some(500), Cura, 4000);
        cura.classe = "Cleric";
        r.linhas.insert(0, cura);
        ocultar_na_morte(&mut r);
        assert_eq!(r.nome_matador, "Gladiator");
        let quem: Vec<&str> = r.linhas.iter().map(|l| l.quem.as_str()).collect();
        assert_eq!(quem, ["Cleric", "Axios", "você", "você", "você", "você", "você", "Axios"]);
    }
}
