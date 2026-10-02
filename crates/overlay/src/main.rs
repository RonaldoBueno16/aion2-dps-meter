//! Overlay do medidor: janela sempre no topo, sem borda e transparente, com o placar da luta.
// Sem console no build de release; o de debug mantém o console para diagnóstico.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod janela;

use eframe::egui;

fn main() {
    let opcoes = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AION2 DPS")
            .with_inner_size([janela::LARGURA, 120.0])
            .with_position([40.0, 220.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_active(false),
        ..Default::default()
    };
    let resultado =
        eframe::run_native("Aion2Meter", opcoes, Box::new(|cc| Ok(Box::new(janela::Overlay::novo(cc)?))));
    // Sem console no release: falha de OpenGL (VM, área de trabalho remota, driver velho) vira aviso na tela.
    if let Err(erro) = resultado {
        janela::avisar(&format!("O medidor não abriu: {erro}"));
    }
}
