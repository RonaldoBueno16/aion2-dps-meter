//! A janela do AION 2: os atalhos só valem com ela em primeiro plano, e o recolher e o painel de
//! drops usam a área dela quando o overlay está em cima.
//! Achada pelo gerenciador de janelas (classe UnrealWindow, título "AION2"): só título, classe e
//! retângulo, sem abrir handle no processo do jogo.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetClientRect, GetForegroundWindow, GetWindowTextW, IsIconic, IsWindow,
    IsWindowVisible,
};

/// Desligado no replay de debug, que roda sem o jogo.
static SEGUIR: AtomicBool = AtomicBool::new(false);
static JOGO: AtomicIsize = AtomicIsize::new(0);

pub fn seguir(sim: bool) {
    SEGUIR.store(sim, Ordering::Relaxed);
}

pub fn seguindo() -> bool {
    SEGUIR.load(Ordering::Relaxed)
}

pub fn em_primeiro_plano() -> bool {
    let jogo = janela();
    jogo != 0 && unsafe { GetForegroundWindow() } as isize == jogo
}

/// Com o jogo aberto, mesmo minimizado ou atrás de outra janela.
pub fn aberto() -> bool {
    janela() != 0
}

/// Área do jogo na tela (esquerda, topo, direita, base), em pixels físicos. None com o jogo
/// fechado ou minimizado.
pub fn area() -> Option<[i32; 4]> {
    let jogo = janela() as HWND;
    if jogo.is_null() || unsafe { IsIconic(jogo) } != 0 {
        return None;
    }
    let mut cliente = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut canto = POINT { x: 0, y: 0 };
    let ok = unsafe { GetClientRect(jogo, &mut cliente) != 0 && ClientToScreen(jogo, &mut canto) != 0 };
    (ok && cliente.right > 0 && cliente.bottom > 0)
        .then(|| [canto.x, canto.y, canto.x + cliente.right, canto.y + cliente.bottom])
}

/// HWND do jogo (0 = não está aberto). Guardado enquanto a janela existir.
fn janela() -> isize {
    let guardada = JOGO.load(Ordering::Relaxed);
    if guardada != 0 && unsafe { IsWindow(guardada as HWND) } != 0 {
        return guardada;
    }
    let mut achada: isize = 0;
    unsafe { EnumWindows(Some(conferir), (&raw mut achada) as LPARAM) };
    JOGO.store(achada, Ordering::Relaxed);
    achada
}

unsafe extern "system" fn conferir(janela: HWND, achada: LPARAM) -> i32 {
    let mut classe = [0_u16; 64];
    let mut titulo = [0_u16; 64];
    unsafe {
        if IsWindowVisible(janela) == 0 {
            return 1;
        }
        let n = GetClassNameW(janela, classe.as_mut_ptr(), classe.len() as i32).max(0) as usize;
        let m = GetWindowTextW(janela, titulo.as_mut_ptr(), titulo.len() as i32).max(0) as usize;
        // O título vem como "AION2  ", com espaços no fim.
        if String::from_utf16_lossy(&classe[..n]) == "UnrealWindow" && String::from_utf16_lossy(&titulo[..m]).starts_with("AION2") {
            *(achada as *mut isize) = janela as isize;
            return 0;
        }
    }
    1
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Precisa do AION 2 aberto: `cargo test -p overlay -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn acha_a_janela_do_jogo_aberto() {
        let jogo = janela();
        assert_ne!(jogo, 0, "AION 2 não encontrado");
        let area = area().expect("área do jogo");
        println!("jogo 0x{jogo:X}, área {area:?}, em primeiro plano: {}", em_primeiro_plano());
        assert!(area[2] - area[0] >= 640 && area[3] - area[1] >= 360);
    }
}
