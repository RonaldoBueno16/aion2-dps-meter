//! Busca de itens (F7b, 0.15.0): o texto vai ao questlog (getItems), com filtro de categoria e
//! páginas de 40; a ★ põe o item na lista de desejos e o clique abre a ficha com "Onde conseguir".
//! O overlay não recebe teclado (WS_EX_NOACTIVATE, para não tirar o foco do jogo): o clique no campo
//! pega o foco só enquanto você digita e o devolve ao jogo no Enter, no Esc ou ao clicar fora
//! (F7-P2, opção (a)). Se o Windows não passar o foco, nada é digitado: as letras iriam para o jogo.

use std::time::{Duration, Instant};

use eframe::egui::text::LayoutJob;
use eframe::egui::{Align, CursorIcon, Key, Layout, Rect, RichText, ScrollArea, Sense, TextEdit, Ui, Vec2, pos2, vec2};
use nucleo::medicao::catalogo::{Busca, ItemDrop, PAGINAS_DA_BUSCA};
use nucleo::medicao::dados_jogo;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

use super::drops::{nota, raridade};
use super::{Overlay, Tela, botao, branco, definir_sem_ativar, fonte, montar, texto, trecho, uma_linha, visual};
use crate::config::{DESEJOS_MAX, Desejo, PRIORIDADE_PADRAO};
use crate::jogo;

/// As cinco categorias que o getItems aceita (conferido em 2026-10-09; outros nomes dão zero
/// páginas): (mainCategory, rótulo, explicação).
const CATEGORIAS: [(&str, &str, &str); 6] = [
    ("", "Tudo", "Todas as categorias"),
    ("armor", "Armadura", "Peças de armadura"),
    ("weapon", "Arma", "Armas"),
    ("accessory", "Acessório", "Colar, anel, brinco..."),
    ("usable", "Usável", "Baús, poções, títulos, pedras..."),
    ("misc", "Materiais", "Materiais de craft e outros (\"misc\" no questlog)"),
];
/// A busca sai meio segundo depois da última tecla (ou no Enter): cada prefixo digitado seria um
/// pedido ao questlog, na fila urgente, na frente do nome do chefe numa luta.
const ESPERA_DIGITAR: Duration = Duration::from_millis(500);
const MINIMO_DE_LETRAS: usize = 2;
const LISTA_MAX: f32 = 380.0;
const ICONE_INTEIRO: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

#[derive(Default)]
pub(super) struct EstadoBusca {
    texto: String,
    categoria: &'static str,
    /// O que está pedido ao questlog: (termo, categoria) e quantas páginas.
    pedido: Option<(String, &'static str)>,
    paginas: u32,
    /// Última tecla, enquanto a espera corre.
    mudou_em: Option<Instant>,
    /// Digitando: quem estava na frente quando o campo foi clicado (o jogo, quase sempre).
    pub(super) digitando: Option<isize>,
    /// O Windows não passou o foco no último clique no campo.
    sem_foco: bool,
}

/// O pedido para o texto e a categoria: com 2 letras, ou só a categoria.
fn pedido_valido(texto: &str, categoria: &'static str) -> Option<(String, &'static str)> {
    let termo = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    (termo.chars().count() >= MINIMO_DE_LETRAS || !categoria.is_empty()).then_some((termo, categoria))
}

/// Só digita com o Axon de fato na frente: com o foco recusado, as letras iriam para o jogo.
fn pode_digitar(frente: isize, overlay: isize) -> bool {
    overlay != 0 && frente == overlay
}

/// A quem devolver o foco ao terminar: só ao jogo que estava na frente no clique, e só com o Axon
/// ainda na frente (quem clicou no jogo ou em outro programa já deu o foco a ele).
fn devolver_a(antes: isize, jogo: isize, frente: isize, overlay: isize) -> Option<isize> {
    (jogo != 0 && antes == jogo && frente == overlay && overlay != 0).then_some(jogo)
}

/// Nomes iguais numa linha só (as variantes de um item, como as duas Luvas de Newbold: uma cai do
/// chefe, a outra vem no baú), na ordem do questlog.
fn agrupar(itens: &[ItemDrop]) -> Vec<Vec<&ItemDrop>> {
    let mut grupos: Vec<Vec<&ItemDrop>> = Vec::new();
    for item in itens {
        match grupos.iter_mut().find(|g| g[0].nome == item.nome) {
            Some(grupo) => grupo.push(item),
            None => grupos.push(vec![item]),
        }
    }
    grupos
}

/// A ★ de uma linha: com alguma variante na lista, tira todas; sem nenhuma, põe todas que couberem.
fn alternar_variantes(desejos: &mut Vec<Desejo>, codigos: &[u32]) {
    if desejos.iter().any(|d| codigos.contains(&d.codigo)) {
        desejos.retain(|d| !codigos.contains(&d.codigo));
        return;
    }
    for &codigo in codigos {
        if desejos.len() < DESEJOS_MAX {
            desejos.push(Desejo { codigo, prioridade: PRIORIDADE_PADRAO, alertar: true });
        }
    }
}

fn frente() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

/// O que a lista mostra embaixo dos itens.
enum Fim {
    Buscando,
    Falhou,
    Mais(u32, u32),
    /// O questlog só entrega as 25 primeiras páginas.
    Limite,
    Acabou,
}

impl Overlay {
    pub(super) fn abrir_busca(&mut self) {
        self.tela = Tela::Busca;
        dados_jogo::repetir_falhas();
    }

    /// O clique no campo: tira o WS_EX_NOACTIVATE e traz o Axon para a frente, e só digita se deu.
    fn comecar_a_digitar(&mut self) -> bool {
        let antes = frente();
        definir_sem_ativar(self.janela, false);
        unsafe { SetForegroundWindow(self.janela as _) };
        if pode_digitar(frente(), self.janela) {
            self.busca.digitando = Some(antes);
            self.busca.sem_foco = false;
            return true;
        }
        definir_sem_ativar(self.janela, true);
        self.busca.sem_foco = true;
        false
    }

    /// Enter, Esc, clique fora ou saída da tela: o bit volta e, se for o caso, o foco volta ao jogo.
    pub(super) fn parar_de_digitar(&mut self, devolver: bool) {
        let Some(antes) = self.busca.digitando.take() else { return };
        let jogo = if devolver { devolver_a(antes, jogo::janela_do_jogo(), frente(), self.janela) } else { None };
        definir_sem_ativar(self.janela, true);
        if let Some(jogo) = jogo {
            unsafe { SetForegroundWindow(jogo as _) };
        }
    }

    pub(super) fn tela_busca(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Buscar item").font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "‹ Lista de desejos", false).on_hover_text("Volta à lista de desejos").clicked() {
                    self.parar_de_digitar(true);
                    self.tela = Tela::Desejos;
                }
            });
        });
        ui.add_space(6.0);
        let campo = TextEdit::singleline(&mut self.busca.texto)
            .hint_text("Clique e digite: luvas gartua (acento e maiúscula não importam)")
            .font(fonte(12.0, false))
            .text_color(texto())
            .desired_width(f32::INFINITY);
        let resposta = ui.add(campo);
        if resposta.changed() {
            self.busca.mudou_em = Some(Instant::now());
        }
        let enter = resposta.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if resposta.gained_focus() && self.busca.digitando.is_none() && !self.comecar_a_digitar() {
            resposta.surrender_focus();
        }
        if self.busca.digitando.is_some() {
            if resposta.lost_focus() {
                self.parar_de_digitar(true);
            } else if frente() != self.janela {
                // Você clicou no jogo ou em outro programa: ele já tem o foco.
                self.parar_de_digitar(false);
                resposta.surrender_focus();
            }
        }
        if self.busca.sem_foco {
            nota(ui, "O Windows deixou o foco com o jogo: clique no campo de novo para digitar.", branco(0x99));
        } else if self.busca.digitando.is_some() {
            nota(ui, "Digitando: Enter ou Esc devolvem o foco ao jogo.", branco(0x77));
        }
        ui.add_space(4.0);

        let mut categoria = None;
        ui.horizontal_wrapped(|ui| {
            for (valor, rotulo, dica) in CATEGORIAS {
                if botao(ui, rotulo, self.busca.categoria == valor).on_hover_text(dica).clicked() {
                    categoria = Some(valor);
                }
                ui.add_space(2.0);
            }
        });
        let agora = Instant::now();
        let esperou = self.busca.mudou_em.is_some_and(|t| agora.duration_since(t) >= ESPERA_DIGITAR);
        if let Some(valor) = categoria {
            self.busca.categoria = valor;
        }
        if enter || esperou || categoria.is_some() {
            self.busca.mudou_em = None;
            self.busca.pedido = pedido_valido(&self.busca.texto, self.busca.categoria);
            self.busca.paginas = 1;
        } else if self.busca.mudou_em.is_some() {
            ui.ctx().request_repaint_after(ESPERA_DIGITAR);
        }
        ui.add_space(6.0);

        let Some((termo, categoria)) = self.busca.pedido.clone() else {
            let dica = "Digite 2 letras ou escolha uma categoria. Os nomes vêm do questlog (base comunitária), \
                        em português; o texto buscado vai para o questlog.";
            nota(ui, dica, branco(0x88));
            return;
        };
        let mut itens = Vec::new();
        let mut fim = Fim::Acabou;
        for pagina in 1..=self.busca.paginas {
            match dados_jogo::buscar_itens(&termo, categoria, pagina) {
                Busca::Pronto(resultado) => {
                    let cheia = !resultado.itens.is_empty();
                    itens.extend(resultado.itens);
                    let ultima = resultado.paginas.min(PAGINAS_DA_BUSCA);
                    fim = if !cheia || pagina >= resultado.paginas {
                        Fim::Acabou
                    } else if pagina >= ultima {
                        Fim::Limite
                    } else {
                        Fim::Mais(pagina + 1, ultima)
                    };
                }
                Busca::Buscando => {
                    fim = Fim::Buscando;
                    break;
                }
                Busca::Falhou => {
                    fim = Fim::Falhou;
                    break;
                }
            }
        }
        if itens.is_empty() && matches!(fim, Fim::Acabou) {
            nota(ui, "Nada encontrado.", branco(0x99));
            return;
        }
        let grupos = agrupar(&itens);
        let mut abrir = None;
        let mut marcar = None;
        ScrollArea::vertical().id_salt("busca").max_height(LISTA_MAX).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for grupo in &grupos {
                match self.linha_da_busca(ui, grupo) {
                    Some(true) => marcar = Some(grupo.iter().map(|i| i.codigo).collect::<Vec<_>>()),
                    Some(false) => abrir = Some(grupo[0].codigo),
                    None => {}
                }
            }
        });
        ui.add_space(4.0);
        match fim {
            Fim::Buscando => {
                nota(ui, "Buscando no questlog...", branco(0x99));
                ui.ctx().request_repaint_after(Duration::from_millis(200));
            }
            Fim::Falhou => {
                nota(ui, "O questlog não respondeu.", branco(0x99));
                if botao(ui, "Tentar de novo", false).clicked() {
                    dados_jogo::repetir_falhas();
                }
            }
            Fim::Mais(proxima, ultima) => {
                if botao(ui, "Mais", false).on_hover_text(format!("Página {proxima} de {ultima}")).clicked() {
                    self.busca.paginas = proxima;
                }
            }
            Fim::Limite => nota(ui, "O questlog mostra só os 1.000 primeiros: escreva mais do nome.", branco(0x99)),
            Fim::Acabou => {}
        }
        if let Some(codigos) = marcar {
            alternar_variantes(&mut self.config.desejos, &codigos);
            self.aplicar_config();
        }
        if let Some(codigo) = abrir {
            self.alternar_ficha(codigo, ui.ctx().pixels_per_point());
        }
    }

    /// Ícone, nome na cor da raridade (e "2 variantes") e a ★ à direita. Some(true): a ★;
    /// Some(false): o resto da linha (a ficha).
    fn linha_da_busca(&mut self, ui: &mut Ui, grupo: &[&ItemDrop]) -> Option<bool> {
        let item = grupo[0];
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 24.0), Sense::click());
        let meio = rect.center().y;
        let estrela = Rect::from_center_size(pos2(rect.max.x - 14.0, meio), Vec2::splat(22.0));
        let na_estrela = ui.interact(estrela, ui.id().with(("busca_estrela", item.codigo)), Sense::click());
        let marcado = grupo.iter().any(|i| self.eh_desejo(i.codigo));
        if resposta.hovered() && !na_estrela.hovered() {
            ui.painter().rect_filled(rect, 3, branco(0x0C));
        }
        if na_estrela.hovered() {
            ui.painter().rect_filled(estrela, 4, branco(0x1A));
        }
        let quadrado = Rect::from_center_size(pos2(rect.min.x + 12.0, meio), Vec2::splat(18.0));
        match item.icone.as_deref() {
            Some(icone) => self.imagem_web(ui, icone, ICONE_INTEIRO, quadrado),
            None => {
                ui.painter().rect_filled(quadrado, 3, branco(0x22));
            }
        }
        let (nome_raridade, cor) = raridade(item.raridade);
        let x = quadrado.max.x + 6.0;
        let mut job = LayoutJob::default();
        trecho(&mut job, &item.nome, 11.0, false, cor);
        if grupo.len() > 1 {
            trecho(&mut job, &format!("   {} variantes", grupo.len()), 10.0, false, branco(0x88));
        }
        job.wrap = uma_linha((estrela.min.x - 6.0 - x).max(40.0));
        let nome = montar(ui, job);
        let (simbolo, cor_estrela) = match (marcado, na_estrela.hovered()) {
            (true, _) => ("★", visual::DOURADO),
            (false, true) => ("☆", texto()),
            (false, false) => ("☆", branco(0x88)),
        };
        let simbolo = ui.fonts_mut(|f| f.layout_no_wrap(simbolo.into(), fonte(14.0, false), cor_estrela));
        let pintor = ui.painter();
        pintor.galley(pos2(x, meio - nome.size().y / 2.0), nome, texto());
        pintor.galley(estrela.center() - simbolo.size() / 2.0, simbolo, cor_estrela);

        let codigos: Vec<String> = grupo.iter().map(|i| i.codigo.to_string()).collect();
        let variantes = match grupo.len() {
            1 => String::new(),
            n => format!("\n{n} variantes com o mesmo nome ({}): a ★ marca todas.", codigos.join(", ")),
        };
        let raridade = nome_raridade.map_or_else(String::new, |r| format!("\n{r}"));
        let na_estrela = na_estrela
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(if marcado { "Tirar da lista de desejos" } else { "Pôr na lista de desejos" });
        if na_estrela.clicked() {
            return Some(true);
        }
        let dica = format!("{}{raridade}{variantes}\n\nClique para ver de onde vem.", item.nome);
        let resposta = resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica);
        resposta.clicked().then_some(false)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn item(codigo: u32, nome: &str) -> ItemDrop {
        ItemDrop {
            codigo,
            nome: nome.into(),
            icone: None,
            raridade: 31,
            categoria: "armor".into(),
            tipo: String::new(),
            chance: None,
            quantidade: None,
        }
    }

    #[test]
    fn so_busca_com_duas_letras_ou_uma_categoria() {
        assert_eq!(pedido_valido("  l  ", ""), None);
        assert_eq!(pedido_valido("lu", ""), Some(("lu".to_string(), "")));
        assert_eq!(pedido_valido(" luvas   gartua ", "armor"), Some(("luvas gartua".to_string(), "armor")));
        assert_eq!(pedido_valido("", "usable"), Some((String::new(), "usable")));
    }

    #[test]
    fn so_digita_com_o_axon_na_frente_e_so_devolve_ao_jogo_que_estava_la() {
        let (overlay, jogo, outro) = (10, 20, 30);
        assert!(pode_digitar(overlay, overlay));
        // O Windows recusou: o jogo continua na frente e as letras iriam para ele.
        assert!(!pode_digitar(jogo, overlay));
        assert!(!pode_digitar(0, 0));
        assert_eq!(devolver_a(jogo, jogo, overlay, overlay), Some(jogo));
        // Você clicou no jogo (ou em outro programa) no meio: o foco já está lá.
        assert_eq!(devolver_a(jogo, jogo, jogo, overlay), None);
        assert_eq!(devolver_a(jogo, jogo, outro, overlay), None);
        // Na frente antes estava outro programa, ou o jogo fechou: não mexe.
        assert_eq!(devolver_a(outro, jogo, overlay, overlay), None);
        assert_eq!(devolver_a(jogo, 0, overlay, overlay), None);
    }

    #[test]
    fn variantes_do_mesmo_nome_numa_linha_e_a_estrela_marca_todas() {
        let itens = [item(210540076, "Luvas de Newbold"), item(210140076, "Peitoral de Newbold"), item(210540129, "Luvas de Newbold")];
        let grupos = agrupar(&itens);
        let codigos: Vec<Vec<u32>> = grupos.iter().map(|g| g.iter().map(|i| i.codigo).collect()).collect();
        assert_eq!(codigos, [vec![210540076, 210540129], vec![210140076]]);

        let mut desejos = Vec::new();
        alternar_variantes(&mut desejos, &[210540076, 210540129]);
        assert_eq!(desejos.iter().map(|d| d.codigo).collect::<Vec<_>>(), [210540076, 210540129]);
        // Uma das variantes já na lista: a ★ tira todas.
        desejos.retain(|d| d.codigo != 210540129);
        alternar_variantes(&mut desejos, &[210540076, 210540129]);
        assert!(desejos.is_empty());
        // Lista cheia: entra o que couber.
        let mut cheia: Vec<Desejo> =
            (0..DESEJOS_MAX as u32 - 1).map(|c| Desejo { codigo: c, prioridade: 2, alertar: true }).collect();
        alternar_variantes(&mut cheia, &[210540076, 210540129]);
        assert_eq!(cheia.len(), DESEJOS_MAX);
        assert!(cheia.iter().any(|d| d.codigo == 210540076) && !cheia.iter().any(|d| d.codigo == 210540129));
    }
}
