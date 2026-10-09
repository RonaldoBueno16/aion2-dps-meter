//! Overlay do medidor: janela sempre no topo, sem borda e transparente, com o placar da luta.
// Sem console no build de release; o de debug mantém o console para diagnóstico.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alertas;
mod atalho;
mod atualizacao;
mod bandeja;
mod config;
mod eventos;
mod firewall;
mod inicio;
mod instancia;
mod janela;
mod jogo;
mod recordes;

use std::time::Duration;

use eframe::egui;

fn main() {
    let argumentos: Vec<String> = std::env::args().collect();
    let tem = |opcao: &str| argumentos.iter().any(|a| a == opcao);
    // Um Axon só (o replay de debug não conta). Depois do Atualizar, o velho ainda está fechando.
    let _unica = if cfg!(debug_assertions) && tem("--replay") {
        None
    } else {
        let esperar = if tem(atualizacao::ATUALIZADO) { Duration::from_secs(10) } else { Duration::ZERO };
        match instancia::garantir(esperar) {
            Some(unica) => Some(unica),
            None => {
                instancia::pedir_para_aparecer();
                return;
            }
        }
    };
    // Aberto pelo Windows com o jogo fechado: já nasce escondido, sem piscar na tela.
    let escondido = tem(inicio::SEGUNDO_PLANO) && !jogo::aberto();
    // Só no debug: --posicao x y abre a janela em outro lugar (para testar o recolher nas duas bordas).
    let posicao = if cfg!(debug_assertions) {
        let numeros: Vec<f32> = std::env::args().skip_while(|a| a != "--posicao").skip(1).take(2).filter_map(|n| n.parse().ok()).collect();
        <[f32; 2]>::try_from(numeros).unwrap_or([40.0, 220.0])
    } else {
        [40.0, 220.0]
    };
    let opcoes = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Axon")
            .with_inner_size([janela::LARGURA, 120.0])
            .with_position(posicao)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_active(false)
            .with_visible(!escondido)
            // Fora da barra de tarefas: o Axon fica na bandeja, ao lado do relógio.
            .with_taskbar(false)
            // Sem o logo do egui: a janela usa o ícone do exe.
            .with_icon(egui::IconData::default()),
        ..Default::default()
    };
    let resultado =
        eframe::run_native("Axon", opcoes, Box::new(|cc| Ok(Box::new(janela::Overlay::novo(cc)?))));
    // Sem console no release: falha de OpenGL (VM, área de trabalho remota, driver velho) vira aviso na tela.
    if let Err(erro) = resultado {
        janela::avisar(&format!("O medidor não abriu: {erro}"));
    }
}
