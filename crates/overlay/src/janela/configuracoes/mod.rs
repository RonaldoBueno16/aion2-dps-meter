//! Tela de configurações: um índice em grupos e uma página por item. Cada linha do índice mostra, à
//! direita, o estado atual em poucas palavras ("15 s", "4 arquivos, 2,1 MB"), e o clique abre a
//! página, que volta pelo "‹ Configurações". Tudo vale na hora e fica salvo.

mod medidor;
mod sistema;

use std::time::{Duration, Instant};

use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, CursorIcon, Rect, RichText, Sense, Stroke, TextFormat, Ui, pos2, vec2};

use super::{Overlay, Tela, botao, branco, cor_da_classe, fonte, montar, texto, uma_linha};


/// Uma página das configurações; `Indice` é a tela inicial, com a lista de todas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pagina {
    Indice,
    Jogadores,
    Luta,
    Visual,
    Atalhos,
    Dados,
}

/// Os grupos do índice, na ordem da tela. Página nova entra aqui e no `match` de `tela_configuracoes`.
const GRUPOS: &[(&str, &[Pagina])] = &[
    ("Medidor", &[Pagina::Jogadores, Pagina::Luta, Pagina::Visual]),
    ("Sistema", &[Pagina::Atalhos, Pagina::Dados]),
];

/// Quanto tempo o "Apagar?" espera o segundo clique.
const CONFIRMAR_EM: Duration = Duration::from_secs(5);

impl Pagina {
    fn titulo(self) -> &'static str {
        match self {
            Pagina::Indice => "Configurações",
            Pagina::Jogadores => "Números e jogadores",
            Pagina::Luta => "Luta",
            Pagina::Visual => "Visual",
            Pagina::Atalhos => "Atalhos",
            Pagina::Dados => "Dados no PC",
        }
    }

    /// Só no debug: `--config <página>` abre direto nela.
    pub(super) fn pelo_nome(nome: Option<&str>) -> Pagina {
        match nome {
            Some("jogadores") => Pagina::Jogadores,
            Some("luta") => Pagina::Luta,
            Some("visual") => Pagina::Visual,
            Some("atalhos") => Pagina::Atalhos,
            Some("dados") => Pagina::Dados,
            _ => Pagina::Indice,
        }
    }
}

/// O que as páginas guardam entre um quadro e outro.
#[derive(Default)]
pub(super) struct EstadoConfig {
    /// Arquivos da pasta de dados, medidos ao abrir as configurações (não a cada quadro).
    arquivos: Vec<sistema::ArquivoDoPc>,
    /// Ação que espera o segundo clique ("Apagar?") e desde quando.
    confirmando: Option<(&'static str, Instant)>,
}

impl EstadoConfig {
    /// Primeiro clique: arma a ação. Segundo clique na mesma, dentro de `CONFIRMAR_EM`: devolve true.
    fn confirmar(&mut self, acao: &'static str) -> bool {
        let armada = self.confirmando.is_some_and(|(qual, desde)| qual == acao && desde.elapsed() < CONFIRMAR_EM);
        self.confirmando = if armada { None } else { Some((acao, Instant::now())) };
        armada
    }

    /// A ação está esperando o segundo clique.
    fn esperando(&self, acao: &str) -> bool {
        self.confirmando.is_some_and(|(qual, desde)| qual == acao && desde.elapsed() < CONFIRMAR_EM)
    }
}

impl Overlay {
    /// Abre as configurações na página dada, medindo antes os arquivos da pasta de dados.
    pub(super) fn abrir_configuracoes(&mut self, pagina: Pagina) {
        self.tela = Tela::Configuracoes(pagina);
        self.estado_config.arquivos = sistema::medir();
        self.estado_config.confirmando = None;
        self.ler_placar();
    }

    pub(super) fn tela_configuracoes(&mut self, ui: &mut Ui, pagina: Pagina) {
        let antes = self.config.clone();
        if pagina == Pagina::Indice {
            self.indice(ui);
        } else {
            match cabecalho_da_pagina(ui, pagina.titulo()) {
                Some(Navegar::Indice) => self.abrir_configuracoes(Pagina::Indice),
                Some(Navegar::Medidor) => self.tela = Tela::Medidor,
                None => {}
            }
            ui.add_space(10.0);
            match pagina {
                Pagina::Indice => {}
                Pagina::Jogadores => self.pagina_jogadores(ui),
                Pagina::Luta => self.pagina_luta(ui),
                Pagina::Visual => self.pagina_visual(ui),
                Pagina::Atalhos => self.pagina_atalhos(ui),
                Pagina::Dados => self.pagina_dados(ui),
            }
            ui.add_space(4.0);
        }
        if self.config != antes {
            self.aplicar_config();
            // "Restaurar padrões" regravou o config.json: o tamanho na página muda.
            if pagina == Pagina::Dados {
                self.estado_config.arquivos = sistema::medir();
            }
        }
    }

    /// A tela inicial: os grupos e, em cada um, uma linha por página com o resumo à direita.
    fn indice(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Configurações").font(fonte(12.0, true)).color(texto()));
            ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                if botao(ui, "Pronto", true).on_hover_text("Volta ao medidor").clicked() {
                    self.tela = Tela::Medidor;
                }
            });
        });
        let acento = self.acento();
        let mut abrir = None;
        for (grupo, paginas) in GRUPOS {
            ui.add_space(10.0);
            ui.label(RichText::new(*grupo).font(fonte(10.0, false)).color(branco(0x88)));
            ui.add_space(2.0);
            for &pagina in *paginas {
                if item_do_indice(ui, pagina.titulo(), &self.resumo(pagina), acento) {
                    abrir = Some(pagina);
                }
            }
        }
        ui.add_space(4.0);
        if let Some(pagina) = abrir {
            self.tela = Tela::Configuracoes(pagina);
            self.estado_config.confirmando = None;
        }
    }

    /// O estado da página numa linha, para o índice.
    fn resumo(&self, pagina: Pagina) -> String {
        match pagina {
            Pagina::Indice => String::new(),
            Pagina::Jogadores => medidor::resumo_jogadores(&self.config),
            Pagina::Luta => medidor::resumo_luta(&self.config),
            Pagina::Visual => medidor::resumo_visual(&self.config),
            Pagina::Atalhos => sistema::resumo_atalhos(),
            Pagina::Dados => sistema::resumo_dados(&self.estado_config.arquivos),
        }
    }

    /// Cor do "ligado": a da sua classe, a mesma da sua barra; sem classe conhecida, o texto.
    fn acento(&self) -> Color32 {
        match self.amostra.as_ref().map(|j| j.classe) {
            Some(classe) if !classe.is_empty() => cor_da_classe(classe),
            _ => texto(),
        }
    }
}

enum Navegar {
    Indice,
    Medidor,
}

/// "‹ Configurações" à esquerda, o nome da página no meio e "Pronto" à direita, em toda subpágina.
fn cabecalho_da_pagina(ui: &mut Ui, titulo: &str) -> Option<Navegar> {
    let largura = ui.available_width();
    let mut navegar = None;
    let linha = ui
        .horizontal(|ui| {
            if botao(ui, "‹ Configurações", false).on_hover_text("Volta à lista das configurações").clicked() {
                navegar = Some(Navegar::Indice);
            }
            ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                if botao(ui, "Pronto", true).on_hover_text("Volta ao medidor").clicked() {
                    navegar = Some(Navegar::Medidor);
                }
            });
        })
        .response
        .rect;
    let nome = montar(ui, LayoutJob::single_section(titulo.to_string(), TextFormat::simple(fonte(12.0, true), texto())));
    let centro = pos2(linha.min.x + largura / 2.0, linha.center().y);
    ui.painter().galley(centro - nome.size() / 2.0, nome, texto());
    navegar
}

/// Linha do índice: nome, resumo no acento e "›"; a linha inteira é o botão.
fn item_do_indice(ui: &mut Ui, nome: &str, resumo: &str, acento: Color32) -> bool {
    let largura = ui.available_width();
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 30.0), Sense::click());
    let resposta = resposta.on_hover_cursor(CursorIcon::PointingHand);
    let pintor = ui.painter();
    if resposta.hovered() {
        pintor.rect_filled(rect, 4, branco(0x14));
    }
    let meio = rect.center().y;
    let nome = montar(ui, LayoutJob::single_section(nome.to_string(), TextFormat::simple(fonte(12.0, false), texto())));
    pintor.galley(pos2(rect.min.x + 10.0, meio - nome.size().y / 2.0), nome.clone(), texto());
    let seta = montar(ui, LayoutJob::single_section("›".into(), TextFormat::simple(fonte(15.0, false), branco(0x88))));
    let x_seta = rect.max.x - 8.0 - seta.size().x;
    pintor.galley(pos2(x_seta, meio - seta.size().y / 2.0), seta, texto());
    let mut job = LayoutJob::single_section(resumo.to_string(), TextFormat::simple(fonte(11.0, false), acento));
    job.wrap = uma_linha((x_seta - 12.0 - (rect.min.x + 10.0 + nome.size().x + 16.0)).max(40.0));
    let resumo = montar(ui, job);
    pintor.galley(pos2(x_seta - 10.0 - resumo.size().x, meio - resumo.size().y / 2.0), resumo, texto());
    resposta.clicked()
}

/// Nome de seção: semibold, sem caixa alta.
fn secao(ui: &mut Ui, titulo: &str) {
    ui.label(RichText::new(titulo).font(fonte(11.0, true)).color(branco(0xAA)));
}

fn dica(ui: &mut Ui, texto_dica: &str) {
    ui.add(eframe::egui::Label::new(RichText::new(texto_dica).font(fonte(10.0, false)).color(branco(0x88))).wrap());
}

fn frase(ui: &mut Ui, trecho: &str) {
    ui.label(RichText::new(trecho).font(fonte(12.0, false)).color(texto()));
    ui.add_space(6.0);
}

/// Número entre os botões − e +, centrado numa largura fixa (a frase não pula ao mudar).
fn valor(ui: &mut Ui, texto_valor: &str, largura: f32) {
    let galley = montar(ui, LayoutJob::single_section(texto_valor.to_string(), TextFormat::simple(fonte(12.0, true), texto())));
    let (rect, _) = ui.allocate_exact_size(vec2(largura, galley.size().y + 2.0), Sense::hover());
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, texto());
}

/// Botão − ou +; no limite da faixa fica apagado e não responde.
fn passo(ui: &mut Ui, simbolo: &str, habilitado: bool) -> bool {
    if habilitado {
        let clicou = botao(ui, simbolo, false).clicked();
        ui.add_space(2.0);
        return clicou;
    }
    let galley = montar(ui, LayoutJob::single_section(simbolo.to_string(), TextFormat::simple(fonte(12.0, false), branco(0x44))));
    let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(14.0, 2.0), Sense::hover());
    ui.painter().galley(rect.min + vec2(7.0, 1.0), galley, texto());
    ui.add_space(2.0);
    false
}

/// Interruptor com rótulo e dica; a linha inteira é clicável. A bolinha anima em 120 ms.
fn interruptor(ui: &mut Ui, rotulo: &str, texto_dica: &str, valor: &mut bool, acento: Color32) {
    let largura = ui.available_width();
    let titulo = montar(ui, LayoutJob::single_section(rotulo.to_string(), TextFormat::simple(fonte(12.0, false), texto())));
    let mut job = LayoutJob::single_section(texto_dica.to_string(), TextFormat::simple(fonte(10.0, false), branco(0x88)));
    job.wrap.max_width = largura - 48.0;
    let explicacao = montar(ui, job);
    let altura = titulo.size().y + explicacao.size().y + 2.0;
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, altura), Sense::click());
    let resposta = resposta.on_hover_cursor(CursorIcon::PointingHand);
    if resposta.clicked() {
        *valor = !*valor;
    }

    let t = ui.ctx().animate_bool_with_time(resposta.id, *valor, 0.12);
    let pintor = ui.painter();
    pintor.galley(rect.min, titulo.clone(), texto());
    pintor.galley(rect.min + vec2(0.0, titulo.size().y + 2.0), explicacao, texto());

    let pista = Rect::from_center_size(pos2(rect.max.x - 16.0, rect.min.y + titulo.size().y / 2.0), vec2(28.0, 16.0));
    let fundo = if resposta.hovered() { branco(0x44) } else { branco(0x33) };
    pintor.rect_filled(pista, 8, fundo.lerp_to_gamma(acento, t));
    let bolinha = pos2(pista.min.x + 8.0 + (pista.width() - 16.0) * t, pista.center().y);
    pintor.circle_filled(bolinha, 6.0, branco(0xCC).lerp_to_gamma(Color32::from_rgb(0x10, 0x14, 0x18), t));
}

/// Traço discreto entre os itens de uma lista.
fn divisoria(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, branco(0x1C)));
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn toda_pagina_aparece_no_indice_uma_vez() {
        let todas = [Pagina::Jogadores, Pagina::Luta, Pagina::Visual, Pagina::Atalhos, Pagina::Dados];
        for pagina in todas {
            let vezes = GRUPOS.iter().flat_map(|(_, paginas)| paginas.iter()).filter(|&&p| p == pagina).count();
            assert_eq!(vezes, 1, "{pagina:?}");
        }
        let no_indice: usize = GRUPOS.iter().map(|(_, paginas)| paginas.len()).sum();
        assert_eq!(no_indice, todas.len());
    }

    #[test]
    fn apagar_pede_o_segundo_clique_na_mesma_acao() {
        let mut estado = EstadoConfig::default();
        assert!(!estado.confirmar("memoria"));
        assert!(estado.esperando("memoria"));
        // Outro botão no meio desarma.
        assert!(!estado.confirmar("config"));
        assert!(!estado.esperando("memoria"));
        assert!(estado.confirmar("config"));
        assert!(!estado.esperando("config"));
    }

    #[test]
    fn debug_abre_a_pagina_pelo_nome() {
        assert_eq!(Pagina::pelo_nome(Some("dados")), Pagina::Dados);
        assert_eq!(Pagina::pelo_nome(Some("?")), Pagina::Indice);
        assert_eq!(Pagina::pelo_nome(None), Pagina::Indice);
    }
}
