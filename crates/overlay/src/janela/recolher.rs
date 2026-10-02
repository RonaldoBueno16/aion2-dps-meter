//! Recolher para a borda: a janela desliza até a borda mais próxima da área do jogo (ou da área de
//! trabalho do monitor, sem a barra de tarefas, no replay) e vira uma aba "Overlay ›" colada nela; clicar na aba traz a janela de
//! volta ao lugar de antes. Posições em pixels físicos, como o Windows devolve.

use std::time::{Duration, Instant};

use eframe::egui::{self, Pos2, Vec2, ViewportCommand, pos2};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};

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
