//! Recordes de chefe na tela: a avaliação de cada kill (gravada na hora), a faixa de 16 px abaixo do
//! card do alvo (comparação ao vivo e, depois do kill, o resultado), a ★ e o motivo na lista ☰ e a
//! tela de recordes, com apagar um ou todos.

use std::time::{Duration, Instant};

use eframe::egui::load::SizedTexture;
use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align, Color32, CursorIcon, Layout, Rect, RichText, ScrollArea, Sense, TextFormat, Ui, Vec2, pos2, vec2,
};
use nucleo::TICKS_POR_SEGUNDO;
use nucleo::medicao::dados_jogo;
use nucleo::medicao::medidor::{Abate, LutaPassada};
use nucleo::medicao::recordes::{self, Recorde, Resultado};

use super::{
    Overlay, Tela, botao, branco, compacto, configuracoes, cor_da_classe, data_local, fonte, montar, texto, trecho,
    uma_linha, visual,
};
use crate::recordes::TENTAR_DE_NOVO;

const VERDE: Color32 = Color32::from_rgb(0x5B, 0xD1, 0x6B);
/// Quanto tempo o "Apagar?" espera o segundo clique.
const CONFIRMAR_EM: Duration = Duration::from_secs(5);

/// "2:41".
fn tempo(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// "+17 s" ou "−5 s".
fn diferenca_s(ms: i64) -> String {
    let s = (ms as f64 / 1000.0).round() as i64;
    if s >= 0 { format!("+{s} s") } else { format!("−{} s", -s) }
}

/// "−6%" ou "+3%".
fn diferenca_pct(meu: f64, melhor: f64) -> String {
    let pct = ((meu / melhor - 1.0) * 100.0).round() as i64;
    if pct >= 0 { format!("+{pct}%") } else { format!("−{}%", -pct) }
}

/// O kill contra o melhor de antes dele, em pedaços (texto, cor) para a faixa.
fn partes_do_kill(r: &Resultado) -> Vec<(String, Color32)> {
    let mut partes = Vec::new();
    match (&r.candidato.tempo, &r.tempo_antes) {
        (Ok(t), _) if r.novo_tempo => {
            let antes = r.tempo_antes.as_ref().map_or_else(String::new, |a| format!(" (antes {})", tempo(a.ms)));
            partes.push((format!("★ Novo recorde {}{antes}", tempo(t.ms)), visual::DOURADO));
        }
        (Ok(t), Some(a)) => {
            let cor = if t.ms > a.ms { visual::VERMELHO_CLARO } else { VERDE };
            let texto = format!("{}, recorde {} ({})", tempo(t.ms), tempo(a.ms), diferenca_s(t.ms as i64 - a.ms as i64));
            partes.push((texto, cor));
        }
        _ => {}
    }
    match (&r.candidato.dps, &r.dps_antes) {
        (Ok(d), _) if r.novo_dps => {
            let antes = r.dps_antes.as_ref().map_or_else(String::new, |a| format!(" (antes {})", compacto(a.valor)));
            partes.push((format!("★ DPS recorde {}{antes}", compacto(d.valor)), visual::DOURADO));
        }
        (Ok(d), Some(a)) => {
            let cor = if d.valor < a.valor { visual::VERMELHO_CLARO } else { VERDE };
            let texto = format!("DPS {}, recorde {} ({})", compacto(d.valor), compacto(a.valor), diferenca_pct(d.valor, a.valor));
            partes.push((texto, cor));
        }
        _ => {}
    }
    partes
}

/// O mouse da faixa e da linha em ☰: o que contou, o que não contou e por quê.
pub(super) fn dica_do_kill(r: &Resultado) -> String {
    let mut linhas: Vec<String> = partes_do_kill(r).into_iter().map(|(texto, _)| texto).collect();
    if let Err(motivo) = r.candidato.tempo {
        linhas.push(format!("Tempo não conta: {}", motivo.texto()));
    }
    if let Err(motivo) = r.candidato.dps {
        linhas.push(format!("DPS não conta: {}", motivo.texto()));
    }
    linhas.join("\n")
}

/// ★ na linha em ☰ (recorde novo) e o mouse; None sem chefe ou com os recordes desligados.
pub(super) fn marca_da_luta(
    luta: &LutaPassada,
    kills: &std::collections::HashMap<(u32, nucleo::Hora), Resultado>,
) -> Option<(bool, String)> {
    let abate = luta.abate.as_ref()?;
    if let Some(r) = abate.morte.and_then(|morte| kills.get(&(abate.codigo, morte))) {
        return Some((r.novo_dps || r.novo_tempo, dica_do_kill(r)));
    }
    let motivo = recordes::candidato(abate).err()?.texto();
    Some((false, format!("Não conta para o recorde: {motivo}")))
}

impl Overlay {
    /// Kill novo (chave: código do chefe e hora da morte) vira recorde na hora, e grava se mudou; o
    /// recusado fica de fora e é avaliado de novo, porque você pode ser reconhecido até a luta acabar.
    pub(super) fn avaliar_kills(&mut self, abates: Vec<Abate>) {
        let Some(guardados) = &mut self.recordes else { return };
        for abate in abates {
            let Some(morte) = abate.morte else { continue };
            let chave = (abate.codigo, morte);
            if self.kills.contains_key(&chave) {
                continue;
            }
            let Ok(candidato) = recordes::candidato(&abate) else { continue };
            let resultado = guardados.recordes.registrar(&candidato);
            if resultado.mudou && !self.replay {
                let _ = guardados.gravar();
            }
            self.kills.insert(chave, resultado);
        }
        if !self.replay {
            guardados.tentar_de_novo();
        }
    }

    /// Entre o card do alvo e o da morte: ao vivo, o melhor tempo, a previsão e o DPS contra o melhor;
    /// depois do kill, o resultado; numa luta passada, o resultado contra o melhor de antes dela.
    pub(super) fn faixa_de_recorde(&mut self, ui: &mut Ui) {
        let Some(guardados) = &self.recordes else { return };
        let Some(alvo) = &self.placar.alvo else { return };
        if !alvo.chefe || alvo.codigo == 0 {
            return;
        }
        let Some(eu) = self.placar.dano.jogadores.iter().find(|j| j.voce) else { return };
        let abate = self.abate_visto.as_ref().filter(|a| a.entidade == alvo.entidade);
        let kill = abate.and_then(|a| self.kills.get(&(a.codigo, a.morte?)));
        let melhor = guardados.recordes.melhor(alvo.codigo, eu.classe);

        let mut job = LayoutJob::default();
        let separador = |job: &mut LayoutJob| {
            if !job.sections.is_empty() {
                trecho(job, "  ·  ", 10.0, false, branco(0x88));
            }
        };
        let dica = match (kill, melhor) {
            (Some(r), _) => {
                for (parte, cor) in partes_do_kill(r) {
                    separador(&mut job);
                    trecho(&mut job, &parte, 10.0, true, cor);
                }
                if job.sections.is_empty() {
                    let motivo = match (&r.candidato.tempo, &r.candidato.dps) {
                        (Err(motivo), _) => motivo.texto(),
                        (_, Err(motivo)) => motivo.texto(),
                        _ => String::new(),
                    };
                    trecho(&mut job, &format!("Não conta para o recorde: {motivo}"), 10.0, false, branco(0x99));
                }
                dica_do_kill(r)
            }
            (None, Some(m)) if !alvo.morto => {
                if let Some(t) = &m.tempo {
                    trecho(&mut job, &format!("★ {} ({} jog.)", tempo(t.ms), t.jogadores), 10.0, true, visual::DOURADO);
                    if let Some(derrota) = alvo.derrota_em {
                        let previsao = self.placar.duracao / (TICKS_POR_SEGUNDO / 1000) + (derrota * 1000.0) as i64;
                        let atras = previsao > t.ms as i64;
                        separador(&mut job);
                        let texto = format!("previsão {} ({})", tempo(previsao.max(0) as u64), diferenca_s(previsao - t.ms as i64));
                        trecho(&mut job, &texto, 10.0, false, if atras { visual::VERMELHO_CLARO } else { VERDE });
                    }
                }
                if let Some(d) = &m.dps {
                    separador(&mut job);
                    let cor = if eu.por_segundo < d.valor { visual::VERMELHO_CLARO } else { VERDE };
                    let texto = format!(
                        "DPS {} de {} ({})",
                        compacto(eu.por_segundo),
                        compacto(d.valor),
                        diferenca_pct(eu.por_segundo, d.valor)
                    );
                    trecho(&mut job, &texto, 10.0, false, cor);
                }
                "Seu recorde neste chefe com esta classe. Previsão: o tempo da luta mais o \"derrota em\".".into()
            }
            (None, Some(_)) => {
                let motivo = abate.and_then(|a| recordes::candidato(a).err()).map(|m| m.texto());
                let texto = motivo.map_or_else(|| "Este kill não conta para o recorde".into(), |m| format!("Não conta: {m}"));
                trecho(&mut job, &texto, 10.0, false, branco(0x99));
                "Só conta o kill com o Axon aberto desde antes do primeiro golpe no chefe.".into()
            }
            (None, None) => return,
        };
        job.wrap = uma_linha(ui.available_width() - 12.0);
        let galley = montar(ui, job);
        let (rect, resposta) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
        ui.painter().galley(pos2(rect.min.x + 6.0, rect.center().y - galley.size().y / 2.0), galley, texto());
        resposta.on_hover_text(dica);
        ui.add_space(4.0);
    }

    /// A tela aberta pelo "Recordes" da ☰.
    pub(super) fn tela_recordes(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Recordes de chefe").font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "Voltar", true).on_hover_text("Volta às lutas anteriores").clicked() {
                    self.tela = Tela::Lutas;
                    self.ler_placar();
                }
            });
        });
        ui.add_space(4.0);
        let pequeno = |ui: &mut Ui, texto_dica: &str, cor: Color32| {
            ui.add(egui::Label::new(RichText::new(texto_dica).font(fonte(10.0, false)).color(cor)).wrap());
        };
        let Some(guardados) = &self.recordes else {
            pequeno(ui, "Os recordes estão desligados.", branco(0x99));
            ui.add_space(6.0);
            if botao(ui, "Abrir as configurações", false).clicked() {
                self.abrir_configuracoes(configuracoes::Pagina::Recordes);
            }
            return;
        };
        pequeno(
            ui,
            "O seu melhor tempo de kill e o seu melhor DPS em cada chefe, por classe. O tempo exige o chefe pego \
             com o HP cheio e menos de 30 jogadores; o DPS, 20 s batendo nele.",
            branco(0x88),
        );
        let mut avisos = Vec::new();
        if self.replay {
            avisos.push("Replay: nada é gravado.".to_string());
        }
        if guardados.so_leitura {
            avisos.push("O arquivo é de uma versão mais nova do Axon (ou não deu para ler): só leitura.".into());
        }
        if guardados.corrompido {
            avisos.push(
                "O recordes.json estava ilegível: ficou guardado como recordes.json.corrompido, e os recordes \
                 começaram do zero."
                    .into(),
            );
        }
        if guardados.removidos > 0 {
            avisos.push(format!("{} recordes inválidos ficaram de fora na leitura.", guardados.removidos));
        }
        if let Some((erro, _)) = &guardados.por_gravar {
            avisos.push(format!("Não deu para gravar ({erro}); tenta de novo a cada {} s.", TENTAR_DE_NOVO.as_secs()));
        }
        for aviso in &avisos {
            ui.add_space(2.0);
            pequeno(ui, aviso, visual::AMARELO);
        }
        ui.add_space(6.0);

        let mut lista: Vec<Recorde> = guardados.recordes.lista.clone();
        let caminho = guardados.caminho().display().to_string();
        if lista.is_empty() {
            pequeno(ui, "Nenhum recorde ainda: o primeiro kill de chefe que valer aparece aqui.", branco(0xAA));
        }
        lista.sort_by_key(|r| std::cmp::Reverse(r.dps.as_ref().map(|d| d.data).max(r.tempo.as_ref().map(|t| t.data))));
        let mut apagar = None;
        ScrollArea::vertical().max_height(340.0).auto_shrink([false, true]).show(ui, |ui| {
            for r in &lista {
                if self.linha_recorde(ui, r) {
                    apagar = Some((r.npc, r.classe.clone()));
                }
            }
        });
        if let Some((npc, classe)) = apagar {
            self.apagar_recorde(Some((npc, &classe)));
        }

        ui.add_space(8.0);
        if !lista.is_empty() {
            let armado = self.esperando_apagar(0, "");
            if armado {
                ui.ctx().request_repaint_after(CONFIRMAR_EM);
            }
            let rotulo = if armado { "Apagar todos mesmo?" } else { "Apagar todos" };
            if botao(ui, rotulo, armado).on_hover_text("Some com o recordes.json").clicked() && self.confirmar_apagar(0, "") {
                self.apagar_recorde(None);
            }
            ui.add_space(4.0);
        }
        pequeno(ui, &format!("Arquivo: {caminho}. Só o código do chefe, a sua classe e números."), branco(0x77));
    }

    /// Retrato, nome e Nv do chefe (do questlog), a classe, as marcas e o ✕. true no segundo clique do ✕.
    fn linha_recorde(&mut self, ui: &mut Ui, r: &Recorde) -> bool {
        let largura = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(largura, 38.0), Sense::hover());
        let pintor = ui.painter().clone();
        pintor.rect_filled(rect, 4, Color32::from_black_alpha(0x40));
        let info = dados_jogo::catalogo().and_then(|c| c.npc(r.npc));
        let retrato = info.as_ref().and_then(|n| dados_jogo::catalogo()?.caminho_icone(n.retrato.as_deref()));
        let circulo = Rect::from_center_size(pos2(rect.min.x + 20.0, rect.center().y), Vec2::splat(28.0));
        pintor.circle_filled(circulo.center(), 14.0, Color32::from_rgb(0x1A, 0x0A, 0x0D));
        if let Some(textura) = retrato.as_deref().and_then(|c| self.textura(ui.ctx(), c)) {
            egui::Image::new(SizedTexture::new(textura.id(), circulo.size())).corner_radius(14).paint_at(ui, circulo);
        }

        let chave = format!("{}:{}", r.npc, r.classe);
        let armado = self.esperando_apagar(r.npc, &r.classe);
        let xis = Rect::from_center_size(pos2(rect.max.x - 16.0, rect.center().y), vec2(if armado { 64.0 } else { 20.0 }, 20.0));
        let xis = xis.translate(vec2(if armado { -22.0 } else { 0.0 }, 0.0));
        let clique = ui
            .interact(xis, ui.id().with(("apagar_recorde", &chave)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Apagar este recorde (dois cliques)");
        if clique.hovered() || armado {
            pintor.rect_filled(xis, 3, branco(0x22));
        }
        let (rotulo, cor) = if armado { ("Apagar?", visual::VERMELHO_CLARO) } else { ("✕", branco(0xAA)) };
        let galley = montar(ui, LayoutJob::single_section(rotulo.into(), TextFormat::simple(fonte(10.0, armado), cor)));
        pintor.galley(xis.center() - galley.size() / 2.0, galley, texto());
        if armado {
            ui.ctx().request_repaint_after(CONFIRMAR_EM);
        }

        let inicio = circulo.max.x + 8.0;
        let fim = xis.min.x - 6.0;
        let mut nome = LayoutJob::default();
        let titulo = info.as_ref().map_or_else(|| format!("Chefe {}", r.npc), |n| n.nome.clone());
        trecho(&mut nome, &titulo, 12.0, true, texto());
        if let Some(nivel) = info.as_ref().map(|n| n.nivel).filter(|&n| n > 0) {
            trecho(&mut nome, &format!("  Nv {nivel}"), 10.0, false, branco(0xBB));
        }
        trecho(&mut nome, &format!("  ·  {}", r.classe), 10.0, true, cor_da_classe(&r.classe));
        nome.wrap = uma_linha((fim - inicio).max(40.0));
        let nome = montar(ui, nome);
        pintor.galley(pos2(inicio, rect.min.y + 4.0), nome, texto());

        let agora = nucleo::agora().div_euclid(TICKS_POR_SEGUNDO);
        let data = |d: i64| {
            if recordes::data_valida(d, agora) { data_local(d * TICKS_POR_SEGUNDO) } else { "data desconhecida".into() }
        };
        let mut marcas = LayoutJob::default();
        if let Some(t) = &r.tempo {
            trecho(&mut marcas, "Tempo ", 10.0, false, branco(0xAA));
            trecho(&mut marcas, &tempo(t.ms), 11.0, true, visual::DOURADO);
            trecho(&mut marcas, &format!(" ({} jog., {})", t.jogadores, data(t.data)), 10.0, false, branco(0xAA));
        }
        if let Some(d) = &r.dps {
            if !marcas.sections.is_empty() {
                trecho(&mut marcas, "  ·  ", 10.0, false, branco(0x77));
            }
            trecho(&mut marcas, "DPS ", 10.0, false, branco(0xAA));
            trecho(&mut marcas, &compacto(d.valor), 11.0, true, visual::DOURADO);
            trecho(&mut marcas, &format!(" ({})", data(d.data)), 10.0, false, branco(0xAA));
        }
        let kills = if r.kills == 1 { "  ·  1 kill".to_string() } else { format!("  ·  {} kills", r.kills) };
        trecho(&mut marcas, &kills, 10.0, false, branco(0x99));
        marcas.wrap = uma_linha((fim - inicio).max(40.0));
        let marcas = montar(ui, marcas);
        pintor.galley(pos2(inicio, rect.max.y - 4.0 - marcas.size().y), marcas, texto());
        ui.add_space(3.0);
        clique.clicked() && self.confirmar_apagar(r.npc, &r.classe)
    }

    /// Um (chefe, classe), ou todos com None. No replay, só a memória.
    fn apagar_recorde(&mut self, qual: Option<(u32, &str)>) {
        let Some(guardados) = &mut self.recordes else { return };
        // O erro de gravação fica no `por_gravar`, que a tela mostra.
        match (qual, self.replay) {
            (Some((npc, classe)), true) => {
                guardados.recordes.apagar(npc, classe);
            }
            (Some((npc, classe)), false) => {
                let _ = guardados.apagar(npc, classe);
            }
            (None, true) => guardados.recordes = Default::default(),
            (None, false) => {
                let _ = guardados.apagar_todos();
            }
        }
    }

    /// Primeiro clique arma; o segundo, no mesmo, em até `CONFIRMAR_EM`, confirma.
    fn confirmar_apagar(&mut self, npc: u32, classe: &str) -> bool {
        let armado = self.esperando_apagar(npc, classe);
        self.apagando_recorde = if armado { None } else { Some((npc, classe.to_string(), Instant::now())) };
        armado
    }

    fn esperando_apagar(&self, npc: u32, classe: &str) -> bool {
        self.apagando_recorde
            .as_ref()
            .is_some_and(|(n, c, desde)| *n == npc && c == classe && desde.elapsed() < CONFIRMAR_EM)
    }
}

#[cfg(test)]
mod testes {
    use nucleo::medicao::recordes::{Candidato, MarcaDps, MarcaTempo, SemTempo};

    use super::*;

    fn resultado(ms: u64, dps: f64, antes: Option<(u64, f64)>) -> Resultado {
        let data = 1_791_400_000;
        let candidato = Candidato {
            npc: 2400425,
            classe: "Ranger",
            data,
            jogadores: 4,
            dps: Ok(MarcaDps { valor: dps, data, ativo_ms: 100_000, jogadores: 4 }),
            tempo: Ok(MarcaTempo { ms, data, jogadores: 4, dps }),
        };
        let mut r = recordes::Recordes::default();
        if let Some((ms, dps)) = antes {
            r.registrar(&Candidato {
                dps: Ok(MarcaDps { valor: dps, data, ativo_ms: 100_000, jogadores: 4 }),
                tempo: Ok(MarcaTempo { ms, data, jogadores: 4, dps }),
                ..candidato.clone()
            });
        }
        r.registrar(&candidato)
    }

    #[test]
    fn faixa_diz_o_recorde_novo_ou_a_diferenca() {
        assert_eq!(tempo(161_000), "2:41");
        let novo = resultado(155_000, 13_500.0, Some((161_000, 13_100.0)));
        let textos: Vec<String> = partes_do_kill(&novo).into_iter().map(|(t, _)| t).collect();
        assert_eq!(textos, ["★ Novo recorde 2:35 (antes 2:41)", "★ DPS recorde 13,5K (antes 13,1K)"]);
        let pior = resultado(178_000, 12_300.0, Some((161_000, 13_100.0)));
        let textos: Vec<String> = partes_do_kill(&pior).into_iter().map(|(t, _)| t).collect();
        assert_eq!(textos, ["2:58, recorde 2:41 (+17 s)", "DPS 12,3K, recorde 13,1K (−6%)"]);
        let primeiro = resultado(161_000, 13_100.0, None);
        assert_eq!(partes_do_kill(&primeiro)[0].0, "★ Novo recorde 2:41");

        let mut parcial = resultado(178_000, 12_300.0, Some((161_000, 13_100.0)));
        parcial.candidato.tempo = Err(SemTempo::Parcial(Some(0.863)));
        assert_eq!(dica_do_kill(&parcial), "DPS 12,3K, recorde 13,1K (−6%)\nTempo não conta: o Axon viu o chefe com 86% do HP");
    }
}
