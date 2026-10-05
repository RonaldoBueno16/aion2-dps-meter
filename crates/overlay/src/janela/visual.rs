//! Peças de desenho do visual da 0.8.0 (inspirado no medidor do TK): degradê, texto com sombra,
//! botão de ícone e a cor dourada dos detalhes.

use std::sync::Arc;

use eframe::egui::epaint::Mesh;
use eframe::egui::{Color32, CursorIcon, Galley, Painter, Pos2, Rect, Response, Sense, Ui, vec2};

use super::{branco, fonte, texto};

/// Dourado dos detalhes (borda, sua linha, a marca).
pub(super) const DOURADO: Color32 = Color32::from_rgb(0xE6, 0xC0, 0x6A);
/// Amarelo da % (como no TK).
pub(super) const AMARELO: Color32 = Color32::from_rgb(0xFF, 0xD8, 0x6B);
pub(super) const VERMELHO_CLARO: Color32 = Color32::from_rgb(0xFF, 0x9A, 0x9A);

/// Retângulo arredondado com degradê horizontal (o egui só pinta cor sólida): leque de triângulos a
/// partir do centro, cada vértice com a cor da sua posição x. Com a cor linear em x, a interpolação
/// dentro de cada triângulo dá o degradê exato.
pub(super) fn degrade(pintor: &Painter, rect: Rect, raio: f32, esquerda: Color32, direita: Color32) {
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }
    let raio = raio.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let cor = |x: f32| esquerda.lerp_to_gamma(direita, ((x - rect.min.x) / rect.width()).clamp(0.0, 1.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), cor(rect.center().x));
    // Sentido horário na tela (y para baixo): direita-baixo, esquerda-baixo, esquerda-cima, direita-cima.
    let cantos = [
        (rect.right_bottom() + vec2(-raio, -raio), 0.0_f32),
        (rect.left_bottom() + vec2(raio, -raio), 90.0),
        (rect.left_top() + vec2(raio, raio), 180.0),
        (rect.right_top() + vec2(-raio, raio), 270.0),
    ];
    const PASSOS: usize = 6;
    for (centro, inicio) in cantos {
        for k in 0..=PASSOS {
            let angulo = (inicio + 90.0 * k as f32 / PASSOS as f32).to_radians();
            let ponto = centro + vec2(angulo.cos(), angulo.sin()) * raio;
            mesh.colored_vertex(ponto, cor(ponto.x));
        }
    }
    let n = mesh.vertices.len() as u32;
    for i in 1..n {
        mesh.add_triangle(0, i, if i + 1 < n { i + 1 } else { 1 });
    }
    pintor.add(mesh);
}

/// Cor mais escura (fator 0 a 1), com o alfa dado.
pub(super) fn escurecer(cor: Color32, fator: f32, alfa: u8) -> Color32 {
    let canal = |c: u8| (f32::from(c) * fator).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(canal(cor.r()), canal(cor.g()), canal(cor.b()), alfa)
}

/// Texto por cima das barras coloridas: sombra preta 1 px abaixo e à direita.
pub(super) fn com_sombra(pintor: &Painter, posicao: Pos2, galley: Arc<Galley>) {
    pintor.galley_with_override_text_color(posicao + vec2(1.0, 1.0), galley.clone(), Color32::from_black_alpha(0xC8));
    pintor.galley(posicao, galley, texto());
}

/// Botão quadrado com um símbolo, realce redondo ao passar o mouse.
pub(super) fn botao_icone(ui: &mut Ui, simbolo: &str, tamanho: f32) -> Response {
    let (rect, resposta) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    let cor = if resposta.hovered() { texto() } else { branco(0xAA) };
    if resposta.hovered() {
        ui.painter().rect_filled(rect, 5, branco(0x1F));
    }
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(simbolo.to_string(), fonte(tamanho, false), cor));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, cor);
    resposta.on_hover_cursor(CursorIcon::PointingHand)
}

/// Aba: texto e, na ativa, um traço dourado embaixo.
pub(super) fn aba(ui: &mut Ui, rotulo: &str, ativa: bool) -> Response {
    let cor = if ativa { texto() } else { branco(0x88) };
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(rotulo.to_string(), fonte(11.5, true), cor));
    let (rect, resposta) = ui.allocate_exact_size(galley.size() + vec2(16.0, 8.0), Sense::click());
    if resposta.hovered() && !ativa {
        ui.painter().rect_filled(rect, 4, branco(0x14));
    }
    ui.painter().galley(rect.min + vec2(8.0, 3.0), galley, cor);
    if ativa {
        let y = rect.max.y - 1.5;
        ui.painter().hline(rect.x_range().shrink(6.0), y, eframe::egui::Stroke::new(2.0_f32, DOURADO));
    }
    resposta.on_hover_cursor(CursorIcon::PointingHand)
}
