//! Páginas de sistema: os atalhos globais e os arquivos que o Axon guarda no PC.

use std::path::Path;

use eframe::egui::{Align, Color32, Layout, RichText, Ui};
use nucleo::formato::f;
use nucleo::medicao::catalogo;

use super::super::{Overlay, arquivo_memoria, botao, branco, fonte, texto, travar};
use super::{dica, divisoria, secao};
use crate::bandeja;
use crate::config::Config;

/// Um arquivo (ou pasta) da pasta de dados, medido ao abrir as configurações.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ArquivoDoPc {
    nome: String,
    bytes: u64,
    /// Só nas pastas: quantos arquivos tem dentro.
    itens: Option<usize>,
}

const VERMELHO: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x6B);

/// O que existe hoje em %LOCALAPPDATA%\Aion2Meter; pasta ausente ou ilegível = lista vazia.
pub(super) fn medir() -> Vec<ArquivoDoPc> {
    medir_em(&catalogo::pasta_dados())
}

fn medir_em(pasta: &Path) -> Vec<ArquivoDoPc> {
    let Ok(entradas) = std::fs::read_dir(pasta) else { return Vec::new() };
    let mut arquivos: Vec<ArquivoDoPc> = entradas
        .flatten()
        .filter_map(|entrada| {
            let nome = entrada.file_name().to_string_lossy().into_owned();
            let dados = entrada.metadata().ok()?;
            if !dados.is_dir() {
                return Some(ArquivoDoPc { nome, bytes: dados.len(), itens: None });
            }
            let (bytes, itens) = std::fs::read_dir(entrada.path())
                .ok()?
                .flatten()
                .filter_map(|dentro| dentro.metadata().ok())
                .filter(|dados| dados.is_file())
                .fold((0, 0), |(bytes, itens), dados| (bytes + dados.len(), itens + 1));
            Some(ArquivoDoPc { nome, bytes, itens: Some(itens) })
        })
        .collect();
    arquivos.sort_by(|a, b| ordem(&a.nome).cmp(&ordem(&b.nome)).then_with(|| a.nome.cmp(&b.nome)));
    arquivos
}

/// Os que têm botão primeiro; depois os caches; o resto no fim.
fn ordem(nome: &str) -> u8 {
    match nome {
        "config.json" => 0,
        "jogadores.json" => 1,
        "icones" => 3,
        _ if descricao(nome).is_empty() => 4,
        _ => 2,
    }
}

fn descricao(nome: &str) -> &'static str {
    match nome {
        "config.json" => "Estas configurações.",
        "jogadores.json" => {
            "O seu nome e o nível e GS de quem você já viu, para aparecerem na hora na próxima luta."
        }
        "icones" => "Ícones das skills e retratos dos monstros, do site da NCSoft.",
        "recordes.json" => "Seus recordes de chefe: o código do chefe, a sua classe e números, nenhum nome.",
        "recordes.json.tmp" => "Cópia de quando os recordes estavam sendo gravados; sobra se o PC desligou no meio.",
        "recordes.json.corrompido" => "Recordes que não deu para ler, guardados como estavam.",
        _ if nome.starts_with("skills-") => "Nomes das skills, do questlog.gg.",
        _ if nome.starts_with("npcs-") => "Nomes dos monstros e chefes, do questlog.gg.",
        _ => "",
    }
}

/// "375 B", "189 KB", "37,9 MB".
fn tamanho(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} MB", f(b / (KB * KB), 1))
    }
}

pub(super) fn resumo_dados(arquivos: &[ArquivoDoPc]) -> String {
    let total: u64 = arquivos.iter().map(|a| a.bytes).sum();
    match arquivos.len() {
        0 => "nada guardado".into(),
        1 => format!("1 item, {}", tamanho(total)),
        n => format!("{n} itens, {}", tamanho(total)),
    }
}

/// Os atalhos que valem agora, ou "desligados".
pub(super) fn resumo_atalhos() -> String {
    let valendo: Vec<&str> = (0..bandeja::ATALHOS_TOTAL).filter_map(|qual| bandeja::atalho(qual).map(|(texto, _)| texto)).collect();
    if valendo.is_empty() { "desligados".into() } else { valendo.join(", ") }
}

impl Overlay {
    /// Os atalhos globais, lidos do config.json na abertura. O overlay não recebe teclado (não tira o
    /// foco do jogo), então a troca é pelo arquivo.
    pub(super) fn pagina_atalhos(&mut self, ui: &mut Ui) {
        dica(ui, "Valem com o jogo na frente; fora dele, a combinação volta para os outros programas.");
        ui.add_space(6.0);
        let c = &self.config;
        let escritos = [&c.atalho_mostrar, &c.atalho_atravessar, &c.atalho_resumo, &c.atalho_compacta];
        let acoes = [
            (bandeja::MOSTRAR, "mostra ou esconde o overlay"),
            (bandeja::ALTERNAR_CLIQUE, "o clique atravessa o overlay até o jogo, ou volta"),
            (bandeja::COPIAR_RESUMO, "copia o resumo da luta"),
            (bandeja::ALTERNAR_COMPACTA, "barra compacta, ou volta ao medidor"),
        ];
        for (qual, acao) in acoes {
            let (tecla, cor) = match bandeja::atalho(qual) {
                Some((atalho, false)) => (atalho.to_string(), texto()),
                Some((atalho, true)) => (format!("{atalho} (em uso por outro programa)"), VERMELHO),
                None if escritos[qual].trim().is_empty() => ("desligado".to_string(), branco(0x77)),
                None => (format!("\"{}\" não vale", escritos[qual].trim()), VERMELHO),
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(tecla).font(fonte(12.0, true)).color(cor));
                ui.add_space(8.0);
                ui.label(RichText::new(acao).font(fonte(12.0, false)).color(texto()));
            });
        }
        ui.add_space(6.0);
        secao(ui, "Para trocar");
        dica(
            ui,
            "Feche o Axon e edite atalho_mostrar, atalho_atravessar, atalho_resumo e atalho_compacta no \
             config.json (em \"Dados no PC\", o \"Abrir a pasta\" leva até ele). Ex.: \"Ctrl+Shift+F9\"; precisa \
             de Ctrl, Alt ou Win; vazio desliga (o resumo e a barra compacta vêm desligados).",
        );
    }

    /// Cada arquivo da pasta de dados com o tamanho, para que serve e, nos que se pode mexer com o
    /// Axon aberto, o botão. Apagar e restaurar pedem um segundo clique.
    pub(super) fn pagina_dados(&mut self, ui: &mut Ui) {
        dica(ui, "Tudo fica em %LOCALAPPDATA%\\Aion2Meter. Nenhum destes arquivos sai do seu PC.");
        ui.add_space(8.0);
        let arquivos = self.estado_config.arquivos.clone();
        if arquivos.is_empty() {
            dica(ui, "A pasta ainda está vazia.");
        }
        let mut mexeu = false;
        for (i, arquivo) in arquivos.iter().enumerate() {
            if i > 0 {
                divisoria(ui);
            }
            ui.horizontal(|ui| {
                let nome = match arquivo.itens {
                    Some(1) => format!("{}  (1 arquivo)", arquivo.nome),
                    Some(n) => format!("{}  ({n} arquivos)", arquivo.nome),
                    None => arquivo.nome.clone(),
                };
                ui.label(RichText::new(nome).font(fonte(12.0, false)).color(texto()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(tamanho(arquivo.bytes)).font(fonte(11.0, false)).color(branco(0x99)));
                });
            });
            let sobre = descricao(&arquivo.nome);
            if !sobre.is_empty() {
                dica(ui, sobre);
            }
            match arquivo.nome.as_str() {
                "config.json" => {
                    ui.add_space(2.0);
                    if self.botao_de_confirmar(ui, "config", "Restaurar padrões", "Restaurar mesmo?") {
                        self.config = Config::default();
                    }
                    if self.estado_config.esperando("config") {
                        dica(
                            ui,
                            "Volta tudo ao padrão, inclusive os alertas, os chefes marcados e a lista de desejos. \
                             Atalhos trocados voltam a Ctrl+H e Ctrl+T na próxima abertura.",
                        );
                    }
                }
                "jogadores.json" => {
                    ui.add_space(2.0);
                    if self.botao_de_confirmar(ui, "memoria", "Apagar", "Apagar mesmo?") {
                        self.esquecer_jogadores();
                        mexeu = true;
                    }
                    if self.estado_config.esperando("memoria") {
                        dica(ui, "Quem aparecer de novo volta a ser guardado.");
                    }
                }
                _ => {}
            }
        }
        if arquivos.iter().any(|a| ordem(&a.nome) >= 2) {
            ui.add_space(10.0);
            dica(
                ui,
                "Nomes e ícones são cópia do que já foi baixado. Para limpar, feche o Axon e apague o arquivo ou \
                 a pasta; ele baixa de novo o que faltar.",
            );
        }
        ui.add_space(8.0);
        if botao(ui, "Abrir a pasta", false).on_hover_text("Abre %LOCALAPPDATA%\\Aion2Meter no Explorer").clicked() {
            // O explorer não abre pasta inexistente; a do Axon some só se o usuário apagar.
            let _ = std::process::Command::new("explorer").arg(catalogo::pasta_dados()).spawn();
        }
        if mexeu {
            self.estado_config.arquivos = medir();
        }
    }

    /// Primeiro clique arma (o rótulo vira a pergunta); o segundo, em até 5 s, confirma.
    fn botao_de_confirmar(&mut self, ui: &mut Ui, acao: &'static str, rotulo: &str, pergunta: &str) -> bool {
        let armada = self.estado_config.esperando(acao);
        let resposta = botao(ui, if armada { pergunta } else { rotulo }, armada);
        if armada {
            // Sem outro evento, a pergunta precisa sumir sozinha quando o prazo vencer.
            ui.ctx().request_repaint_after(super::CONFIRMAR_EM);
        }
        resposta.clicked() && self.estado_config.confirmar(acao)
    }

    /// Esquece os jogadores guardados: a memória do medidor e o jogadores.json. No replay só a
    /// memória: o arquivo é o do jogo de verdade.
    fn esquecer_jogadores(&mut self) {
        travar(&self.sessao).medidor.esquecer_memoria();
        if !self.replay {
            let _ = std::fs::remove_file(arquivo_memoria());
        }
        self.ler_placar();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tamanho_em_b_kb_e_mb_com_virgula() {
        assert_eq!(tamanho(375), "375 B");
        assert_eq!(tamanho(193_641), "189 KB");
        assert_eq!(tamanho(39_741_000), "37,9 MB");
    }

    #[test]
    fn mede_arquivos_e_pastas_na_ordem_da_tela() {
        let pasta = std::env::temp_dir().join(format!("axon-teste-dados-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(pasta.join("icones")).unwrap();
        std::fs::write(pasta.join("icones").join("a.png"), [0u8; 10]).unwrap();
        std::fs::write(pasta.join("icones").join("b.png"), [0u8; 5]).unwrap();
        std::fs::write(pasta.join("skills-pt.json"), b"{}").unwrap();
        std::fs::write(pasta.join("jogadores.json"), b"{}").unwrap();
        std::fs::write(pasta.join("config.json"), b"{}").unwrap();
        std::fs::write(pasta.join("estranho.txt"), b"x").unwrap();
        let arquivos = medir_em(&pasta);
        let _ = std::fs::remove_dir_all(&pasta);

        let nomes: Vec<&str> = arquivos.iter().map(|a| a.nome.as_str()).collect();
        assert_eq!(nomes, ["config.json", "jogadores.json", "skills-pt.json", "icones", "estranho.txt"]);
        let icones = &arquivos[3];
        assert_eq!((icones.bytes, icones.itens), (15, Some(2)));
        assert_eq!(resumo_dados(&arquivos), "5 itens, 22 B");
        assert_eq!(resumo_dados(&[]), "nada guardado");
    }
}
