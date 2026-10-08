//! Drops do chefe de campo, num painel ao lado da janela: o que ele deixa cair segundo o questlog
//! (getNpc e o getItem do baú de saque), separado para decidir se vale ir. Primeiro o conjunto do
//! chefe (as peças com o nome dele, que somam 100% nos 24 chefes de Altgard), depois o que cai sempre,
//! o resto que não é equipamento e os outros equipamentos por raridade.

use std::cmp::Ordering;
use std::collections::HashSet;

use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, CursorIcon, Rect, ScrollArea, Sense, Stroke, Ui, Vec2, pos2, vec2};
use nucleo::formato::p;
use nucleo::medicao::catalogo::{Busca, DropsNpc, ItemDrop};
use nucleo::medicao::dados_jogo;

use super::recolher::Lado;
use super::{Overlay, branco, montar, texto, trecho, uma_linha, visual};
use crate::eventos;

/// Largura do painel e o vão transparente entre ele e a janela do Axon, em pontos.
pub(super) const LARGURA_PAINEL: f32 = 330.0;
pub(super) const VAO: f32 = 6.0;

/// Diferença aceita na soma das chances do conjunto (o questlog dá 8 casas).
const TOLERANCIA: f64 = 0.001;
const ICONE_INTEIRO: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

const ORIGEM_DROPS: &str = "Drops e % do questlog (base comunitária), não conferidos no jogo.";

/// O painel aberto: de qual chefe (código do NPC), de que lado e o que está expandido nele.
pub(super) struct PainelDrops {
    /// None: o painel abriu direto na ficha de um item (lista de desejos), sem chefe.
    pub codigo: Option<u32>,
    pub lado: Lado,
    drops: Option<DropsNpc>,
    falhou: bool,
    bau_aberto: bool,
    raridades_abertas: HashSet<u8>,
    /// Fichas abertas no lugar da lista, a de cima na tela, com a chance do item aqui: a ficha leva ao
    /// baú ou ao ingrediente, e o "‹" volta uma.
    fichas: Vec<(u32, Option<f64>)>,
    /// Na ficha de cima: todos os monstros que derrubam, e as receitas abertas.
    pub(super) todos_os_npcs: bool,
    pub(super) receitas_abertas: HashSet<u32>,
}

impl PainelDrops {
    pub(super) fn novo(codigo: Option<u32>, lado: Lado, fichas: Vec<(u32, Option<f64>)>) -> Self {
        Self {
            codigo,
            lado,
            drops: None,
            falhou: false,
            bau_aberto: false,
            raridades_abertas: HashSet::new(),
            fichas,
            todos_os_npcs: false,
            receitas_abertas: HashSet::new(),
        }
    }

    /// O item da ficha de cima, se há ficha aberta.
    pub(super) fn ficha(&self) -> Option<u32> {
        self.fichas.last().map(|(item, _)| *item)
    }
}

/// Para onde a ficha leva.
pub(super) enum Navegar {
    /// Fecha a ficha de cima.
    Voltar,
    /// Abre a ficha de outro item (o baú, um ingrediente), com a chance dele aqui.
    Item(u32, Option<f64>),
    /// Abre os drops de um chefe de campo.
    Chefe(u32),
}

/// Cor da raridade ("grade" do questlog), a que o questlog usa, e o nome que o jogo em português dá
/// a ela, só onde foi conferido num item do jogo (41 Único, 71 Especial); nas outras, só a cor.
pub(super) fn raridade(grade: u8) -> (Option<&'static str>, Color32) {
    match grade {
        11 => (None, Color32::from_rgb(128, 128, 128)),
        21 => (None, Color32::from_rgb(73, 139, 53)),
        31 => (None, Color32::from_rgb(19, 131, 182)),
        41 => (Some("Único"), Color32::from_rgb(211, 173, 15)),
        51 => (None, Color32::from_rgb(206, 86, 10)),
        61 => (None, Color32::from_rgb(210, 16, 70)),
        71 => (Some("Especial"), Color32::from_rgb(4, 221, 217)),
        _ => (None, Color32::from_rgb(183, 183, 183)),
    }
}

/// "17,3%" a partir de 1%; abaixo, dois algarismos ("0,46%", "0,042%"); "100%" no garantido.
pub(super) fn porcentagem(chance: f64) -> String {
    let pct = chance * 100.0;
    if pct >= 99.95 {
        "100%".into()
    } else if pct >= 1.0 {
        p(chance, 1).replace(",0%", "%")
    } else if pct > 0.0 {
        let casas = (1 - pct.log10().floor() as i32).clamp(1, 6) as usize;
        p(chance, casas)
    } else {
        "0%".into()
    }
}

/// O conjunto do chefe: o sufixo do nome ("de Newbold"), a raridade e as peças, da mais provável à
/// menos.
pub(super) struct Conjunto<'a> {
    pub nome: &'a str,
    pub raridade: u8,
    pub pecas: Vec<&'a ItemDrop>,
}

pub(super) struct Secoes<'a> {
    pub conjunto: Option<Conjunto<'a>>,
    /// Chance de 100% (o baú de saque).
    pub sempre: Vec<&'a ItemDrop>,
    /// O que não é equipamento, da maior chance à menor; sem chance no fim.
    pub tambem: Vec<&'a ItemDrop>,
    /// Os outros equipamentos por raridade, da mais alta à mais baixa.
    pub outros: Vec<(u8, Vec<&'a ItemDrop>)>,
}

fn equipamento(item: &ItemDrop) -> bool {
    matches!(item.categoria.as_str(), "armor" | "weapon" | "accessory")
}

fn garantido(item: &ItemDrop) -> bool {
    item.chance.is_some_and(|c| c >= 1.0 - TOLERANCIA)
}

fn maior_chance(a: &&ItemDrop, b: &&ItemDrop) -> Ordering {
    b.chance.unwrap_or(-1.0).total_cmp(&a.chance.unwrap_or(-1.0))
}

/// "de Newbold" em "Luvas de Newbold": do primeiro " de ", " da ", " do ", " das " ou " dos " ao fim.
pub(super) fn sufixo(nome: &str) -> Option<&str> {
    [" de ", " da ", " do ", " das ", " dos "].iter().filter_map(|s| nome.find(s)).min().map(|i| &nome[i + 1..])
}

/// Separa os drops nas seções do painel. Conjunto: o grupo de equipamentos com o mesmo sufixo cujas
/// chances somam 100% (um por chefe nos 24 de Altgard; sem ele, tudo vai para os outros).
pub(super) fn separar(itens: &[ItemDrop]) -> Secoes<'_> {
    let mut grupos: Vec<(&str, Vec<&ItemDrop>)> = Vec::new();
    for item in itens.iter().filter(|i| equipamento(i)) {
        let Some(s) = sufixo(&item.nome) else { continue };
        match grupos.iter_mut().find(|(g, _)| *g == s) {
            Some((_, pecas)) => pecas.push(item),
            None => grupos.push((s, vec![item])),
        }
    }
    let soma = |pecas: &[&ItemDrop]| pecas.iter().filter_map(|i| i.chance).sum::<f64>();
    let conjunto = grupos
        .into_iter()
        .find(|(_, pecas)| pecas.len() >= 2 && (soma(pecas) - 1.0).abs() <= TOLERANCIA)
        .map(|(nome, mut pecas)| {
            pecas.sort_by(maior_chance);
            Conjunto { nome, raridade: pecas.iter().map(|i| i.raridade).max().unwrap_or(0), pecas }
        });
    let no_conjunto =
        |item: &ItemDrop| conjunto.as_ref().is_some_and(|c| c.pecas.iter().any(|i| i.codigo == item.codigo));

    let sempre: Vec<_> = itens.iter().filter(|i| garantido(i) && !no_conjunto(i)).collect();
    let mut tambem: Vec<_> = itens.iter().filter(|i| !equipamento(i) && !garantido(i)).collect();
    tambem.sort_by(maior_chance);
    let mut outros: Vec<(u8, Vec<&ItemDrop>)> = Vec::new();
    for item in itens.iter().filter(|i| equipamento(i) && !garantido(i) && !no_conjunto(i)) {
        match outros.iter_mut().find(|(r, _)| *r == item.raridade) {
            Some((_, lista)) => lista.push(item),
            None => outros.push((item.raridade, vec![item])),
        }
    }
    outros.sort_by_key(|(r, _)| std::cmp::Reverse(*r));
    for (_, lista) in &mut outros {
        lista.sort_by(maior_chance);
    }
    Secoes { conjunto, sempre, tambem, outros }
}

impl Overlay {
    /// O painel ao lado da janela. `limite`: altura máxima, em pontos (a da área do jogo ou do monitor).
    pub(super) fn painel_drops(&mut self, ui: &mut Ui, limite: f32) {
        let Some(painel) = &self.painel else { return };
        let chefe = painel.codigo;
        if let Some(codigo) = chefe
            && painel.drops.is_none()
            && !painel.falhou
        {
            let resposta = dados_jogo::drops(codigo);
            if let Some(painel) = &mut self.painel {
                match resposta {
                    Busca::Pronto(drops) => painel.drops = Some(drops),
                    Busca::Falhou => painel.falhou = true,
                    Busca::Buscando => {}
                }
            }
        }
        let fechar = match chefe {
            Some(codigo) => self.cabecalho_drops(ui, codigo),
            None => cabecalho_da_ficha(ui),
        };
        if fechar {
            self.fechar_drops();
            return;
        }
        ui.add_space(6.0);

        let Some(painel) = &self.painel else { return };
        if let Some(&(item, chance)) = painel.fichas.last() {
            let voltar = match (painel.fichas.len(), chefe) {
                (1, Some(_)) => Some("‹ Drops"),
                (1, None) => None,
                _ => Some("‹ Voltar"),
            };
            let altura = (limite - 90.0).max(120.0);
            let mut navegar = None;
            ScrollArea::vertical().id_salt(("ficha", item)).max_height(altura).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                navegar = self.ficha_do_item(ui, item, chance, voltar);
            });
            if let Some(navegar) = navegar {
                self.navegar(navegar);
            }
            return;
        }
        let Some(drops) = painel.drops.clone() else {
            if painel.falhou {
                nota(ui, "O questlog não respondeu. Feche e abra o painel para tentar de novo.", branco(0x99));
            } else {
                self.esqueleto_drops(ui);
            }
            return;
        };
        if drops.itens.is_empty() {
            nota(ui, "O questlog não lista drops deste chefe.", branco(0x99));
            return;
        }
        let secoes = separar(&drops.itens);
        ScrollArea::vertical().max_height((limite - 90.0).max(120.0)).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if let Some(conjunto) = &secoes.conjunto {
                self.fileira_do_conjunto(ui, conjunto);
            }
            if !secoes.sempre.is_empty() {
                titulo(ui, "Sempre", branco(0xCC));
                for item in &secoes.sempre {
                    self.linha_sempre(ui, item, &drops);
                }
            }
            if !secoes.tambem.is_empty() {
                titulo(ui, "Também cai", branco(0xCC));
                for item in &secoes.tambem {
                    self.linha_item(ui, item, 0.0, true);
                }
            }
            if !secoes.outros.is_empty() {
                let total: usize = secoes.outros.iter().map(|(_, l)| l.len()).sum();
                titulo(ui, &format!("Outros equipamentos ({total})"), branco(0xCC));
                for (grade, lista) in &secoes.outros {
                    self.raridade_dos_outros(ui, *grade, lista);
                }
            }
            ui.add_space(6.0);
            nota(ui, ORIGEM_DROPS, branco(0x66));
        });
    }

    /// Retrato, nome, nível e quando renasce; o ✕ fecha (devolve true no clique).
    fn cabecalho_drops(&mut self, ui: &mut Ui, codigo: u32) -> bool {
        let info = self.regioes.values().flat_map(|r| &r.chefes).find(|c| c.codigo == codigo).cloned();
        let visto = self.chefes_da_regiao().and_then(|v| v.chefes.into_iter().find(|c| c.codigo == Some(codigo)));
        let largura = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(largura, 40.0), Sense::hover());
        let quadrado = Rect::from_min_size(rect.min + vec2(0.0, 2.0), Vec2::splat(36.0));
        match info.as_ref().and_then(|i| i.retrato.as_deref()) {
            Some(retrato) => self.imagem_web(ui, retrato, eventos::ROSTO, quadrado),
            None => {
                ui.painter().rect_filled(quadrado, 4, branco(0x22));
            }
        }
        let x = quadrado.max.x + 8.0;
        let mut job = LayoutJob::default();
        let nome = info.as_ref().map_or_else(|| format!("NPC {codigo}"), |i| i.nome.clone());
        trecho(&mut job, &nome, 13.0, true, texto());
        job.wrap = uma_linha(rect.max.x - x - 30.0);
        let nome = montar(ui, job);
        let mut job = LayoutJob::default();
        if let Some(nivel) = info.as_ref().map(|i| i.nivel).filter(|n| *n > 0) {
            trecho(&mut job, &format!("nível {nivel}   "), 10.0, false, branco(0x88));
        }
        let agora = nucleo::agora();
        match &visto {
            Some(c) if !c.vivo => {
                let falta = super::chefes::falta_para_renascer(c, agora);
                let cor = if falta <= 600 { visual::DOURADO } else { branco(0xCC) };
                trecho(&mut job, "renasce em ", 10.0, false, branco(0x88));
                trecho(&mut job, &eventos::contagem(falta), 10.0, true, cor);
                let horario = eventos::horario_unix(c.hora_ms / 1000, agora);
                trecho(&mut job, &format!(" ({horario})"), 10.0, false, branco(0x88));
            }
            Some(c) if c.provavel => trecho(&mut job, "vivo, provável", 10.0, false, visual::AMARELO),
            Some(_) => trecho(&mut job, "vivo", 10.0, false, Color32::from_rgb(0x5B, 0xD1, 0x6B)),
            None => trecho(&mut job, "fora da lista desta região", 10.0, false, branco(0x88)),
        }
        let linha = montar(ui, job);
        let pintor = ui.painter();
        pintor.galley(pos2(x, rect.min.y + 3.0), nome, texto());
        pintor.galley(pos2(x, rect.min.y + 22.0), linha, texto());

        let fechar = Rect::from_min_size(pos2(rect.max.x - 24.0, rect.min.y), vec2(24.0, 22.0));
        let resposta = ui.interact(fechar, ui.id().with("fechar_drops"), Sense::click());
        if resposta.hovered() {
            ui.painter().rect_filled(fechar, 5, branco(0x1F));
        }
        let cor = if resposta.hovered() { texto() } else { branco(0xAA) };
        let x_ = ui.fonts_mut(|f| f.layout_no_wrap("✕".into(), super::fonte(12.0, false), cor));
        ui.painter().galley(fechar.center() - x_.size() / 2.0, x_, cor);
        resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Fechar os drops").clicked()
    }

    /// As peças do conjunto lado a lado, na cor da raridade, com a % embaixo de cada uma.
    fn fileira_do_conjunto(&mut self, ui: &mut Ui, conjunto: &Conjunto<'_>) {
        let (nome_raridade, cor) = raridade(conjunto.raridade);
        let nome = match nome_raridade {
            Some(r) => format!("Conjunto {r} {}", conjunto.nome),
            None => format!("Conjunto {}", conjunto.nome),
        };
        titulo(ui, &nome, cor);
        let largura = ui.available_width();
        let n = conjunto.pecas.len().max(1) as f32;
        let passo = (largura / n).min(40.0);
        let lado = (passo - 6.0).min(30.0);
        let (rect, _) = ui.allocate_exact_size(vec2(largura, lado + 18.0), Sense::hover());
        for (i, peca) in conjunto.pecas.iter().enumerate() {
            let centro_x = rect.min.x + passo * (i as f32 + 0.5);
            let quadrado = Rect::from_center_size(pos2(centro_x, rect.min.y + lado / 2.0 + 1.0), Vec2::splat(lado));
            match peca.icone.as_deref() {
                Some(icone) => self.imagem_web(ui, icone, ICONE_INTEIRO, quadrado),
                None => {
                    ui.painter().rect_filled(quadrado, 3, branco(0x22));
                }
            }
            let borda = Color32::from_rgba_unmultiplied(cor.r(), cor.g(), cor.b(), 0xAA);
            ui.painter().rect_stroke(quadrado, 3, Stroke::new(1.0_f32, borda), eframe::egui::StrokeKind::Outside);
            let chance = peca.chance.map_or_else(|| "?".to_string(), porcentagem);
            let mut job = LayoutJob::default();
            trecho(&mut job, &chance, 10.0, true, branco(0xDD));
            let galley = montar(ui, job);
            ui.painter().galley(pos2(centro_x - galley.size().x / 2.0, quadrado.max.y + 3.0), galley, texto());
            let celula = Rect::from_center_size(quadrado.center(), vec2(passo, lado + 4.0));
            let dica = format!("{}\n{chance} no questlog\n\nClique para ver a ficha.", peca.nome);
            let resposta = ui.interact(celula, ui.id().with(("peca", peca.codigo)), Sense::click());
            if resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
                self.abrir_ficha(peca);
            }
        }
        let n = conjunto.pecas.len();
        nota(ui, &format!("As {n} peças somam 100% no questlog: deve cair uma por morte."), branco(0x99));
        ui.add_space(4.0);
    }

    /// O que cai sempre; o baú abre e mostra o que vem dentro.
    fn linha_sempre(&mut self, ui: &mut Ui, item: &ItemDrop, drops: &DropsNpc) {
        let conteudo = drops.baus.get(&item.codigo).filter(|c| !c.is_empty());
        let Some(conteudo) = conteudo else {
            self.linha_item(ui, item, 0.0, true);
            return;
        };
        let aberto = self.painel.as_ref().is_some_and(|p| p.bau_aberto);
        let resposta = self.linha_item(ui, item, 14.0, false);
        let seta = if aberto { "▾" } else { "▸" };
        let mut job = LayoutJob::default();
        trecho(&mut job, seta, 11.0, false, branco(0x99));
        let seta = montar(ui, job);
        let canto = pos2(resposta.rect.min.x + 1.0, resposta.rect.center().y - seta.size().y / 2.0);
        ui.painter().galley(canto, seta, texto());
        let dica = if aberto { "Esconder o que vem no baú" } else { "Ver o que vem no baú" };
        if resposta.interact(Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked()
            && let Some(painel) = &mut self.painel
        {
            painel.bau_aberto = !aberto;
        }
        if aberto {
            let mut lista: Vec<&ItemDrop> = conteudo.iter().collect();
            lista.sort_by(maior_chance);
            for dentro in lista {
                self.linha_item(ui, dentro, 30.0, true);
            }
            ui.add_space(2.0);
        }
    }

    /// "Único   32 itens   0,036% a 0,16% cada" (sem o nome conferido, um quadrado na cor dela): o
    /// clique abre a lista daquela raridade.
    fn raridade_dos_outros(&mut self, ui: &mut Ui, grade: u8, lista: &[&ItemDrop]) {
        let aberta = self.painel.as_ref().is_some_and(|p| p.raridades_abertas.contains(&grade));
        let (nome, cor) = raridade(grade);
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 20.0), Sense::click());
        if resposta.hovered() {
            ui.painter().rect_filled(rect, 3, branco(0x0C));
        }
        let meio = rect.center().y;
        let mut job = LayoutJob::default();
        trecho(&mut job, if aberta { "▾ " } else { "▸ " }, 11.0, false, branco(0x99));
        trecho(&mut job, nome.unwrap_or("■"), 11.0, true, cor);
        let itens = if lista.len() == 1 { "1 item".to_string() } else { format!("{} itens", lista.len()) };
        trecho(&mut job, &format!("   {itens}"), 10.0, false, branco(0x88));
        let esquerda = montar(ui, job);
        let chances: Vec<f64> = lista.iter().filter_map(|i| i.chance).collect();
        let faixa = match (chances.iter().copied().reduce(f64::min), chances.iter().copied().reduce(f64::max)) {
            (Some(menor), Some(maior)) => {
                let (menor, maior) = (porcentagem(menor), porcentagem(maior));
                if menor == maior { format!("{maior} cada") } else { format!("{menor} a {maior} cada") }
            }
            _ => "sem %".to_string(),
        };
        let mut job = LayoutJob::default();
        trecho(&mut job, &faixa, 10.0, false, branco(0xAA));
        let direita = montar(ui, job);
        let pintor = ui.painter();
        pintor.galley(pos2(rect.min.x + 2.0, meio - esquerda.size().y / 2.0), esquerda, texto());
        pintor.galley(pos2(rect.max.x - 4.0 - direita.size().x, meio - direita.size().y / 2.0), direita, texto());
        let dica = if aberta { "Esconder a lista" } else { "Ver cada item, com a % de cada um" };
        if resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked()
            && let Some(painel) = &mut self.painel
        {
            if aberta {
                painel.raridades_abertas.remove(&grade);
            } else {
                painel.raridades_abertas.insert(grade);
            }
        }
        if aberta {
            for item in lista {
                self.linha_item(ui, item, 14.0, true);
            }
            ui.add_space(2.0);
        }
    }

    /// Abre a ficha do item no lugar da lista.
    fn abrir_ficha(&mut self, item: &ItemDrop) {
        self.navegar(Navegar::Item(item.codigo, item.chance));
    }

    fn navegar(&mut self, navegar: Navegar) {
        let Some(painel) = &mut self.painel else { return };
        painel.todos_os_npcs = false;
        painel.receitas_abertas.clear();
        match navegar {
            Navegar::Voltar => {
                painel.fichas.pop();
            }
            Navegar::Item(item, chance) => painel.fichas.push((item, chance)),
            Navegar::Chefe(codigo) if painel.codigo == Some(codigo) => painel.fichas.clear(),
            Navegar::Chefe(codigo) => *painel = PainelDrops::novo(Some(codigo), painel.lado, Vec::new()),
        }
    }

    /// Ícone, nome na cor da raridade e, à direita, a % (e a quantidade, quando passa de 1). Com
    /// `ficha`, o clique abre a ficha do item.
    fn linha_item(&mut self, ui: &mut Ui, item: &ItemDrop, recuo: f32, ficha: bool) -> eframe::egui::Response {
        let largura = ui.available_width();
        let sentido = if ficha { Sense::click() } else { Sense::hover() };
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 22.0), sentido);
        if ficha && resposta.hovered() {
            ui.painter().rect_filled(rect, 3, branco(0x0C));
        }
        let meio = rect.center().y;
        let quadrado = Rect::from_center_size(pos2(rect.min.x + recuo + 10.0, meio), Vec2::splat(18.0));
        match item.icone.as_deref() {
            Some(icone) => self.imagem_web(ui, icone, ICONE_INTEIRO, quadrado),
            None => {
                ui.painter().rect_filled(quadrado, 3, branco(0x22));
            }
        }
        let mut direita = LayoutJob::default();
        if let Some((minimo, maximo)) = item.quantidade.filter(|(_, maximo)| *maximo > 1) {
            let quantos = if minimo == maximo { format!("×{maximo}   ") } else { format!("×{minimo} a {maximo}   ") };
            trecho(&mut direita, &quantos, 10.0, false, branco(0x88));
        }
        let chance = item.chance.map_or_else(|| "?".to_string(), porcentagem);
        trecho(&mut direita, &chance, 11.0, true, branco(0xDD));
        let direita = montar(ui, direita);
        let x = quadrado.max.x + 6.0;
        let (_, cor) = raridade(item.raridade);
        let mut job = LayoutJob::default();
        trecho(&mut job, &item.nome, 11.0, false, cor);
        job.wrap = uma_linha((rect.max.x - 4.0 - direita.size().x - 8.0 - x).max(40.0));
        let nome = montar(ui, job);
        let pintor = ui.painter();
        pintor.galley(pos2(x, meio - nome.size().y / 2.0), nome, texto());
        pintor.galley(pos2(rect.max.x - 4.0 - direita.size().x, meio - direita.size().y / 2.0), direita, texto());
        let raridade = raridade(item.raridade).0.map_or_else(String::new, |r| format!("{r}, "));
        let sem_chance = if item.chance.is_none() { "\nO questlog não dá a chance deste." } else { "" };
        let clique = if ficha { "\n\nClique para ver a ficha." } else { "" };
        let dica = format!("{}\n{raridade}{chance} no questlog{sem_chance}{clique}", item.nome);
        let resposta = resposta.on_hover_text(dica);
        if ficha && resposta.clone().on_hover_cursor(CursorIcon::PointingHand).clicked() {
            self.abrir_ficha(item);
        }
        resposta
    }
}

impl Overlay {
    /// Enquanto os drops não chegam: a fileira do conjunto e algumas linhas, pulsando.
    fn esqueleto_drops(&mut self, ui: &mut Ui) {
        self.carregando = true;
        let cor = super::pulso(ui);
        let largura = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(largura, 70.0), Sense::hover());
        let pintor = ui.painter();
        pintor.rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(150.0, 10.0)), 3, cor);
        for i in 0..7 {
            let canto = rect.min + vec2(5.0 + 40.0 * i as f32, 26.0);
            pintor.rect_filled(Rect::from_min_size(canto, Vec2::splat(30.0)), 3, cor);
        }
        for i in 0..4 {
            let (rect, _) = ui.allocate_exact_size(vec2(largura, 22.0), Sense::hover());
            let pintor = ui.painter();
            let meio = rect.center().y;
            pintor.rect_filled(Rect::from_center_size(pos2(rect.min.x + 10.0, meio), Vec2::splat(18.0)), 3, cor);
            let barra = Rect::from_min_size(pos2(rect.min.x + 28.0, meio - 4.0), vec2(120.0 + 25.0 * i as f32, 8.0));
            pintor.rect_filled(barra, 3, cor);
        }
    }
}

/// Cabeçalho do painel aberto direto numa ficha (lista de desejos): o ✕ fecha (devolve true no clique).
fn cabecalho_da_ficha(ui: &mut Ui) -> bool {
    let largura = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(largura, 22.0), Sense::hover());
    let mut job = LayoutJob::default();
    trecho(&mut job, "★ Lista de desejos", 11.0, true, visual::DOURADO);
    let galley = montar(ui, job);
    ui.painter().galley(pos2(rect.min.x + 2.0, rect.center().y - galley.size().y / 2.0), galley, texto());
    let fechar = Rect::from_min_size(pos2(rect.max.x - 24.0, rect.min.y), vec2(24.0, 22.0));
    let resposta = ui.interact(fechar, ui.id().with("fechar_ficha"), Sense::click());
    if resposta.hovered() {
        ui.painter().rect_filled(fechar, 5, branco(0x1F));
    }
    let cor = if resposta.hovered() { texto() } else { branco(0xAA) };
    let x_ = ui.fonts_mut(|f| f.layout_no_wrap("✕".into(), super::fonte(12.0, false), cor));
    ui.painter().galley(fechar.center() - x_.size() / 2.0, x_, cor);
    resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Fechar a ficha").clicked()
}

/// Título de seção: o texto e um fio até a borda.
pub(super) fn titulo(ui: &mut Ui, texto_titulo: &str, cor: Color32) {
    ui.add_space(4.0);
    let largura = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(largura, 20.0), Sense::hover());
    let mut job = LayoutJob::default();
    trecho(&mut job, texto_titulo, 11.0, true, cor);
    let galley = montar(ui, job);
    let meio = rect.center().y;
    let fim_texto = rect.min.x + galley.size().x;
    ui.painter().galley(pos2(rect.min.x, meio - galley.size().y / 2.0), galley, texto());
    if fim_texto + 8.0 < rect.max.x {
        ui.painter().hline(fim_texto + 8.0..=rect.max.x, meio, Stroke::new(1.0_f32, branco(0x1A)));
    }
}

pub(super) fn nota(ui: &mut Ui, texto_nota: &str, cor: Color32) {
    let rotulo = eframe::egui::RichText::new(texto_nota).font(super::fonte(10.0, false)).color(cor);
    ui.add(eframe::egui::Label::new(rotulo).wrap());
}

#[cfg(test)]
mod testes {
    use nucleo::medicao::catalogo::ler_itens;
    use serde_json::Value;

    use super::*;

    /// Trechos reais do getNpc do questlog (2026-10-06): o conjunto inteiro, o que não é equipamento e
    /// dois equipamentos de cada raridade e categoria (o Gartua inteiro).
    fn drops(chefe: &str) -> Vec<ItemDrop> {
        let todos: Value = serde_json::from_str(include_str!("drops-teste.json")).unwrap();
        ler_itens(todos.get(chefe))
    }

    fn nomes<'a>(itens: &[&'a ItemDrop]) -> Vec<&'a str> {
        itens.iter().map(|i| i.nome.as_str()).collect()
    }

    #[test]
    fn conjunto_e_o_grupo_que_soma_100() {
        let itens = drops("newbold");
        let s = separar(&itens);
        let conjunto = s.conjunto.expect("Newbold tem conjunto");
        assert_eq!((conjunto.nome, conjunto.raridade, conjunto.pecas.len()), ("de Newbold", 31, 7));
        assert_eq!(conjunto.pecas[0].nome, "Luvas de Newbold");
        assert_eq!(conjunto.pecas[6].nome, "Peitoral de Newbold");
        assert_eq!(nomes(&s.sempre), ["Baú de Saque de Newbold (Vinculado)"]);
        assert_eq!(s.tambem.first().map(|i| i.nome.as_str()), Some("Pedra Espiritual"));
        assert_eq!(s.tambem.last().map(|i| i.nome.as_str()), Some("Pedra de Mana Inferior"));
        assert_eq!(s.outros.iter().map(|(r, _)| *r).collect::<Vec<_>>(), [41, 31, 21, 11]);
        assert!(s.outros.iter().flat_map(|(_, l)| l).all(|i| !i.nome.ends_with("de Newbold")));

        let itens = drops("danar");
        let conjunto = separar(&itens).conjunto.expect("Danar tem conjunto");
        assert_eq!((conjunto.nome, conjunto.raridade, conjunto.pecas.len()), ("de Danar", 21, 7));

        // Dourado (Único), com os acessórios.
        let itens = drops("gartua");
        let s = separar(&itens);
        let conjunto = s.conjunto.expect("Gartua tem conjunto");
        assert_eq!((conjunto.nome, conjunto.raridade, conjunto.pecas.len()), ("de Gartua", 41, 9));
        assert!(s.outros.is_empty());
        assert_eq!(nomes(&s.sempre), ["Baú de Saque de Gartua (Vinculado)"]);
    }

    #[test]
    fn sem_soma_de_100_nao_ha_conjunto() {
        let mut itens = drops("newbold");
        let luvas = itens.iter_mut().find(|i| i.nome == "Luvas de Newbold").unwrap();
        luvas.chance = luvas.chance.map(|c| c - 0.01);
        let s = separar(&itens);
        assert!(s.conjunto.is_none());
        let epicos = s.outros.iter().find(|(r, _)| *r == 31).map(|(_, l)| l.len());
        assert_eq!(epicos, Some(7 + 4), "as 7 peças voltam para os outros, junto dos 4 épicos genéricos");
    }

    #[test]
    fn sufixo_desde_a_primeira_preposicao() {
        assert_eq!(sufixo("Espada Longa do Sonho Ilusório"), Some("do Sonho Ilusório"));
        assert_eq!(sufixo("Guarda-braço da Sombra"), Some("da Sombra"));
        assert_eq!(sufixo("Peitoral do Brigadeiro Lagta"), Some("do Brigadeiro Lagta"));
        assert_eq!(sufixo("Pedra Espiritual"), None);
    }

    #[test]
    fn porcentagem_com_casas_conforme_o_tamanho() {
        assert_eq!(porcentagem(1.0), "100%");
        assert_eq!(porcentagem(0.25), "25%");
        assert_eq!(porcentagem(0.17341040), "17,3%");
        assert_eq!(porcentagem(0.035714285), "3,6%");
        assert_eq!(porcentagem(0.0045735), "0,46%");
        assert_eq!(porcentagem(0.00042), "0,042%");
    }
}
