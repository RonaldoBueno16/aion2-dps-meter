//! Recolher para a borda: a janela desliza até a borda mais próxima da área do jogo (ou da área de
//! trabalho do monitor, sem a barra de tarefas, no replay) e vira uma aba "Overlay ›" colada nela; clicar na aba traz a janela de
//! volta ao lugar de antes. Posições em pixels físicos, como o Windows devolve.

use std::time::{Duration, Instant};

use eframe::egui::{self, Pos2, Vec2, ViewportCommand, pos2};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SPI_GETCLIENTAREAANIMATION, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, SystemParametersInfoW,
};

use crate::jogo;

const DURACAO: Duration = Duration::from_millis(220);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lado {
    Esquerda,
    Direita,
}

#[derive(Clone, Copy, Debug)]
pub enum Dobra {
    Aberto,
    /// Deslizando a janela inteira até a borda.
    Indo { inicio: Instant, de: [i32; 2], para: [i32; 2], lado: Lado, borda: i32, volta: [i32; 2] },
    /// Só a aba, colada na borda (`borda` = x da borda da área de trabalho).
    Recolhido { lado: Lado, borda: i32, volta: [i32; 2] },
    /// Janela inteira de novo, deslizando da borda até `para`.
    Voltando { inicio: Instant, de: [i32; 2], para: [i32; 2] },
}

#[derive(Clone, Copy, Debug)]
struct Retangulo {
    esquerda: i32,
    topo: i32,
    direita: i32,
    base: i32,
}

impl Dobra {
    pub fn aberto(&self) -> bool {
        matches!(self, Dobra::Aberto)
    }

    pub fn recolher(&mut self, janela: isize) {
        let (Some(r), Some(area)) = (retangulo(janela), area_de_trabalho(janela)) else { return };
        let lado = lado_de(r, area);
        let (largura, altura) = (r.direita - r.esquerda, r.base - r.topo);
        let (borda, x) = match lado {
            Lado::Esquerda => (area.esquerda, area.esquerda),
            Lado::Direita => (area.direita, area.direita - largura),
        };
        let y = r.topo.clamp(area.topo, (area.base - altura).max(area.topo));
        let volta = [r.esquerda, r.topo];
        *self = Dobra::Indo { inicio: Instant::now(), de: volta, para: [x, y], lado, borda, volta };
    }

    /// `largura_aberta` em pixels: a janela volta inteira, a partir da borda onde estava a aba.
    pub fn abrir(&mut self, janela: isize, largura_aberta: i32) {
        let Dobra::Recolhido { lado, borda, volta } = *self else { return };
        let topo = retangulo(janela).map_or(volta[1], |r| r.topo);
        let x = match lado {
            Lado::Esquerda => borda,
            Lado::Direita => borda - largura_aberta,
        };
        *self = Dobra::Voltando { inicio: Instant::now(), de: [x, topo], para: volta };
    }

    /// Avança a animação e manda os comandos de janela. `aba` e `aberta` em pontos do egui.
    /// Devolve true enquanto anima (o quadro seguinte precisa vir logo).
    pub fn quadro(&mut self, ctx: &egui::Context, aba: Vec2, aberta: Vec2) -> bool {
        let ppp = ctx.pixels_per_point();
        let pontos = |p: [i32; 2]| -> Pos2 { pos2(p[0] as f32 / ppp, p[1] as f32 / ppp) };
        match *self {
            Dobra::Aberto | Dobra::Recolhido { .. } => false,
            Dobra::Indo { inicio, de, para, lado, borda, volta } => {
                let t = progresso(inicio);
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pontos(misturar(de, para, t))));
                if t >= 1.0 {
                    let largura_aba = (aba.x * ppp).round() as i32;
                    let x = match lado {
                        Lado::Esquerda => borda,
                        Lado::Direita => borda - largura_aba,
                    };
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(aba));
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pontos([x, para[1]])));
                    *self = Dobra::Recolhido { lado, borda, volta };
                }
                true
            }
            Dobra::Voltando { inicio, de, para } => {
                let t = progresso(inicio);
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(aberta));
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pontos(misturar(de, para, t))));
                if t >= 1.0 {
                    *self = Dobra::Aberto;
                }
                true
            }
        }
    }
}

/// Para que lado a janela recolheria agora (a seta do botão aponta para ele).
pub fn lado_mais_perto(janela: isize) -> Lado {
    match (retangulo(janela), area_de_trabalho(janela)) {
        (Some(r), Some(area)) => lado_de(r, area),
        _ => Lado::Esquerda,
    }
}

/// Onde o painel de drops (`painel` pixels, com o vão) abre: à direita se couber na área; senão à
/// esquerda se couber; senão do lado com mais espaço. Devolve o lado e quanto a janela anda (pixels;
/// negativo: para a esquerda), com a janela de `largura_total` presa dentro da área.
pub fn lugar_do_painel(janela: isize, painel: i32, largura_total: i32) -> (Lado, i32) {
    match (retangulo(janela), area_de_trabalho(janela)) {
        (Some(r), Some(area)) => lugar([r.esquerda, r.direita], [area.esquerda, area.direita], painel, largura_total),
        _ => (Lado::Direita, 0),
    }
}

fn lugar(janela: [i32; 2], area: [i32; 2], painel: i32, largura_total: i32) -> (Lado, i32) {
    let (livre_esquerda, livre_direita) = (janela[0] - area[0], area[1] - janela[1]);
    let lado = if livre_direita >= painel || (livre_esquerda < painel && livre_direita >= livre_esquerda) {
        Lado::Direita
    } else {
        Lado::Esquerda
    };
    let x = match lado {
        Lado::Direita => janela[0],
        Lado::Esquerda => janela[0] - painel,
    };
    let x = x.clamp(area[0], (area[1] - largura_total).max(area[0]));
    (lado, x - janela[0])
}

/// Muda a largura da janela (pixels) e a anda `dx` na horizontal na hora (SetWindowPos, sem ativar):
/// quem ler o retângulo logo depois (o recolher) já vê o novo.
pub fn ajustar_largura(janela: isize, largura: i32, dx: i32) {
    let Some(r) = retangulo(janela) else { return };
    let (altura, opcoes) = (r.base - r.topo, SWP_NOZORDER | SWP_NOACTIVATE);
    unsafe { SetWindowPos(janela as HWND, std::ptr::null_mut(), r.esquerda + dx, r.topo, largura, altura, opcoes) };
}

/// Altura da área (do jogo ou do monitor), em pixels: o painel de drops não passa dela.
pub fn altura_da_area(janela: isize) -> Option<i32> {
    area_de_trabalho(janela).map(|a| a.base - a.topo)
}

fn lado_de(r: Retangulo, area: Retangulo) -> Lado {
    if r.esquerda + r.direita < area.esquerda + area.direita { Lado::Esquerda } else { Lado::Direita }
}

/// 0 a 1 com desaceleração no fim. Com as animações do Windows desligadas, vai direto ao fim.
fn progresso(inicio: Instant) -> f32 {
    if !animacoes_ligadas() {
        return 1.0;
    }
    let t = (inicio.elapsed().as_secs_f32() / DURACAO.as_secs_f32()).min(1.0);
    1.0 - (1.0 - t).powi(3)
}

fn misturar(de: [i32; 2], para: [i32; 2], t: f32) -> [i32; 2] {
    let um = |a: i32, b: i32| a + ((b - a) as f32 * t).round() as i32;
    [um(de[0], para[0]), um(de[1], para[1])]
}

/// "Animar controles e elementos dentro das janelas" (Acessibilidade do Windows).
fn animacoes_ligadas() -> bool {
    let mut ligadas: i32 = 1;
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut ligadas).cast(), 0) };
    ok == 0 || ligadas != 0
}

fn retangulo(janela: isize) -> Option<Retangulo> {
    if janela == 0 {
        return None;
    }
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let ok = unsafe { GetWindowRect(janela as HWND, &mut r) };
    (ok != 0).then_some(Retangulo { esquerda: r.left, topo: r.top, direita: r.right, base: r.bottom })
}

/// Área do jogo, quando o overlay segue o jogo; senão, a área de trabalho (sem a barra de
/// tarefas) do monitor onde a janela está.
fn area_de_trabalho(janela: isize) -> Option<Retangulo> {
    if janela == 0 {
        return None;
    }
    if jogo::seguindo()
        && let Some([esquerda, topo, direita, base]) = jogo::area()
    {
        return Some(Retangulo { esquerda, topo, direita, base });
    }
    unsafe {
        let monitor = MonitorFromWindow(janela as HWND, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let r = info.rcWork;
        Some(Retangulo { esquerda: r.left, topo: r.top, direita: r.right, base: r.bottom })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn painel_abre_do_lado_que_cabe_e_a_janela_anda_o_que_falta() {
        // Área de 0 a 1920, janela de 470 px, painel de 336 com o vão: 806 no total.
        assert_eq!(lugar([100, 570], [0, 1920], 336, 806), (Lado::Direita, 0));
        assert_eq!(lugar([1400, 1870], [0, 1920], 336, 806), (Lado::Esquerda, -336));
        // Área de 1000: não cabe de lado nenhum. Mais espaço à direita: abre à direita e anda 56.
        assert_eq!(lugar([250, 720], [0, 1000], 336, 806), (Lado::Direita, -56));
        // Mais espaço à esquerda: abre à esquerda e para na borda.
        assert_eq!(lugar([300, 770], [0, 1000], 336, 806), (Lado::Esquerda, -300));
    }
}
