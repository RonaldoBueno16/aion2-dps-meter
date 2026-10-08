//! Lista de desejos: os itens que você procura, de onde cada um vem e quais chefes de campo da região
//! derrubam algum deles (direto ou pelo baú de saque), acima da chance mínima. Só os códigos e as
//! suas escolhas vão para o config.json; nomes, fontes e chances vêm do questlog, só na memória.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Align, Color32, CursorIcon, Layout, Pos2, Rect, Response, RichText, ScrollArea, Sense, Ui, Vec2, pos2, vec2,
};
use nucleo::medicao::catalogo::{Busca, DetalheItem, FonteNpc, Fontes, InfoRegiao, TipoFonte};
use nucleo::medicao::dados_jogo;

use super::chefes::{ChefeVisto, falta_para_renascer};
use super::drops::{Navegar, nota, porcentagem, raridade, titulo};
use super::{Overlay, Tela, botao, branco, fonte, montar, pulso, texto, trecho, uma_linha, visual};
use crate::config::{Config, DESEJOS_MAX, Desejo, PRIORIDADE_PADRAO};
use crate::eventos;

/// Monstros que a ficha mostra antes do "e mais N"; só deles a região é buscada (um getNpc de ~40 KB
/// cada).
const NPCS_NA_FICHA: usize = 5;
/// Com o "e mais" aberto, até quantos (equipamento genérico chega a 1.357).
const NPCS_ABERTOS_MAX: usize = 100;
/// Missões, dungeons... de cada tipo antes do "e mais N".
const OUTRAS_POR_TIPO: usize = 5;
/// Largura da coluna "Cai de", "No baú", "Craft".
const COLUNA: f32 = 58.0;
const ICONE_INTEIRO: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
/// Quanto tempo o "Tirar?" espera o segundo clique.
const CONFIRMAR_EM: Duration = Duration::from_secs(5);
/// Altura máxima da lista na tela de desejos; passando disso, rola.
const LISTA_MAX: f32 = 380.0;
const ORIGEM_FONTES: &str = "De onde vem e as chances: questlog (base comunitária), não conferido no jogo.";
const TIPOS: [TipoFonte; 7] = [
    TipoFonte::Missao,
    TipoFonte::Dungeon,
    TipoFonte::Conquista,
    TipoFonte::Vendedor,
    TipoFonte::Coleta,
    TipoFonte::Suprimento,
    TipoFonte::Passe,
];

/// A chance mínima da config (em %) na escala do questlog (de 0 a 1).
pub(super) fn chance_minima(config: &Config) -> f64 {
    f64::from(config.desejos_chance_minima) / 100.0
}

/// A chance mínima da config como texto: "0,5%", "0,25%", "2%".
pub(super) fn chance_da_config(pct: f32) -> String {
    let texto = nucleo::formato::n(f64::from(pct), 2);
    let texto = match texto.split_once(',') {
        Some((inteira, casas)) if casas.trim_end_matches('0').is_empty() => inteira.to_string(),
        Some((inteira, casas)) => format!("{inteira},{}", casas.trim_end_matches('0')),
        None => texto,
    };
    format!("{texto}%")
}

/// Para cada chefe da região (código do NPC): os desejos que ele derruba e a chance, direto ou pelo
/// baú (chance do baú no chefe × chance do item no baú, supondo independentes), só com chance >=
/// `minima`. Chance que o questlog não dá só entra com a mínima 0.
pub(super) fn chefes_com_desejo(
    regiao: &InfoRegiao,
    desejos: &[(u32, Arc<Fontes>)],
    baus: &HashMap<u32, Arc<Fontes>>,
    minima: f64,
) -> HashMap<u32, Vec<(u32, f64)>> {
    let passa = |chance: Option<f64>| match chance {
        Some(c) => c >= minima,
        None => minima <= 0.0,
    };
    let mut saida: HashMap<u32, Vec<(u32, f64)>> = HashMap::new();
    let mut anotar = |npc: u32, item: u32, chance: f64| {
        let lista = saida.entry(npc).or_default();
        match lista.iter_mut().find(|(i, _)| *i == item) {
            Some((_, maior)) => *maior = maior.max(chance),
            None => lista.push((item, chance)),
        }
    };
    let chefe = |npc: u32| regiao.chefes.iter().any(|c| c.codigo == npc);
    for (item, fontes) in desejos {
        for npc in fontes.npcs.iter().filter(|n| chefe(n.codigo) && passa(n.chance)) {
            anotar(npc.codigo, *item, npc.chance.unwrap_or(0.0));
        }
        for bau in &fontes.baus {
            let Some(do_bau) = baus.get(&bau.codigo) else { continue };
            for npc in do_bau.npcs.iter().filter(|n| chefe(n.codigo)) {
                let chance = npc.chance.zip(bau.chance).map(|(no_chefe, no_bau)| no_chefe * no_bau);
                if passa(chance) {
                    anotar(npc.codigo, *item, chance.unwrap_or(0.0));
                }
            }
        }
    }
    saida
}

/// As fichas dos desejos e dos baús em que eles vêm, como o catálogo tem agora: nunca espera, só pede.
#[derive(Default)]
pub(super) struct FontesDosDesejos {
    pub fichas: HashMap<u32, DetalheItem>,
    pub baus: HashMap<u32, Arc<Fontes>>,
    /// Fichas (de desejo ou de baú) que ainda não chegaram, e as que o questlog não mandou.
    pub faltando: usize,
    pub falharam: usize,
}

impl FontesDosDesejos {
    pub(super) fn itens(&self) -> Vec<(u32, Arc<Fontes>)> {
        self.fichas.iter().map(|(codigo, ficha)| (*codigo, ficha.fontes.clone())).collect()
    }
}

/// `buscar`: a ficha do catálogo, pela fila urgente (tela aberta) ou pela normal (alerta).
pub(super) fn fontes_dos_desejos(
    codigos: impl IntoIterator<Item = u32>,
    buscar: fn(u32) -> Busca<DetalheItem>,
) -> FontesDosDesejos {
    let mut saida = FontesDosDesejos::default();
    let mut baus = HashSet::new();
    for codigo in codigos {
        match buscar(codigo) {
            Busca::Pronto(ficha) => {
                baus.extend(ficha.fontes.baus.iter().map(|b| b.codigo));
                saida.fichas.insert(codigo, ficha);
            }
            Busca::Buscando => saida.faltando += 1,
            Busca::Falhou => saida.falharam += 1,
        }
    }
    for bau in baus {
        match buscar(bau) {
            Busca::Pronto(ficha) => {
                saida.baus.insert(bau, ficha.fontes);
            }
            Busca::Buscando => saida.faltando += 1,
            Busca::Falhou => saida.falharam += 1,
        }
    }
    saida
}

/// Os chefes da região que derrubam um desejo: código do NPC → (nome do item, chance), da maior
/// chance à menor.
pub(super) struct DesejosNaRegiao {
    pub chefes: HashMap<u32, Vec<(String, f64)>>,
    pub faltando: usize,
    pub falharam: usize,
}

pub(super) fn desejos_na_regiao(
    regiao: &InfoRegiao,
    codigos: impl IntoIterator<Item = u32>,
    minima: f64,
    buscar: fn(u32) -> Busca<DetalheItem>,
) -> DesejosNaRegiao {
    let fontes = fontes_dos_desejos(codigos, buscar);
    let por_chefe = chefes_com_desejo(regiao, &fontes.itens(), &fontes.baus, minima);
    let nome = |item: u32| fontes.fichas.get(&item).map_or_else(|| format!("Item {item}"), |f| f.nome.clone());
    let chefes = por_chefe
        .into_iter()
        .map(|(npc, mut lista)| {
            lista.sort_by(|a, b| b.1.total_cmp(&a.1));
            (npc, lista.into_iter().map(|(item, chance)| (nome(item), chance)).collect())
        })
        .collect();
    DesejosNaRegiao { chefes, faltando: fontes.faltando, falharam: fontes.falharam }
}

fn nome_da_prioridade(prioridade: u8) -> &'static str {
    match prioridade {
        1 => "Alta",
        3 => "Baixa",
        _ => "Média",
    }
}

fn nome_do_tipo(tipo: TipoFonte) -> &'static str {
    match tipo {
        TipoFonte::Missao => "Missão",
        TipoFonte::Dungeon => "Dungeon",
        TipoFonte::Conquista => "Conquista",
        TipoFonte::Vendedor => "Vendedor",
        TipoFonte::Coleta => "Coleta",
        TipoFonte::Suprimento => "Suprimento",
        TipoFonte::Passe => "Passe",
    }
}

/// "alchemy" → "Alchemy": a profissão como o questlog dá (o nome em português não foi conferido no
/// jogo, e os de classe também ficam em inglês).
fn profissao(categoria: &str) -> String {
    let mut letras = categoria.chars();
    letras.next().map_or_else(String::new, |primeira| primeira.to_uppercase().chain(letras).collect())
}

/// "raça light": como o questlog dá, como a profissão (o nome no jogo não foi conferido).
fn raca(raca: &str) -> String {
    match raca {
        "all" | "" => String::new(),
        outra => format!("raça {outra}"),
    }
}

/// " (0,0016% a 0,081%)" dos monstros dados; vazio sem chance.
fn faixa_de_chance(npcs: &[FonteNpc]) -> String {
    let chances = || npcs.iter().filter_map(|n| n.chance);
    match (chances().reduce(f64::min), chances().reduce(f64::max)) {
        (Some(menor), Some(maior)) if porcentagem(menor) == porcentagem(maior) => format!(" ({} cada)", porcentagem(maior)),
        (Some(menor), Some(maior)) => format!(" ({} a {})", porcentagem(menor), porcentagem(maior)),
        _ => String::new(),
    }
}

/// A melhor fonte do desejo numa linha, para a lista: o chefe da região que derruba (com a chance e
/// quando renasce); senão o monstro de maior chance, o baú, a receita ou a primeira das outras.
fn resumo_da_fonte(
    fontes: &Fontes,
    baus: &HashMap<u32, Arc<Fontes>>,
    na_regiao: Option<(&ChefeVisto, f64)>,
    agora: nucleo::Hora,
) -> (String, bool) {
    if let Some((chefe, chance)) = na_regiao {
        let estado = if chefe.vivo {
            "vivo".to_string()
        } else {
            format!("renasce em {}", eventos::contagem(falta_para_renascer(chefe, agora)))
        };
        return (format!("♛ {} {}   {estado}", chefe.nome, porcentagem(chance)), true);
    }
    if let Some(npc) = fontes.npcs.first() {
        let chance = npc.chance.map_or_else(String::new, |c| format!(" {}", porcentagem(c)));
        let mais = match fontes.npcs.len() - 1 {
            0 => String::new(),
            1 => " e mais 1".into(),
            n => format!(" e mais {n}"),
        };
        return (format!("Cai de {}{chance}{mais}", npc.nome), false);
    }
    if let Some(bau) = fontes.baus.first() {
        let quem = baus.get(&bau.codigo).and_then(|b| b.npcs.first()).map_or_else(String::new, |n| format!(", de {}", n.nome));
        let chance = bau.chance.map_or_else(String::new, |c| format!(" {}", porcentagem(c)));
        return (format!("No baú {}{chance}{quem}", bau.nome), false);
    }
    if let Some(receita) = fontes.receitas.first() {
        return (format!("Craft: {}", profissao(&receita.profissao)), false);
    }
    if let Some(outra) = fontes.outras.first() {
        return (format!("{}: {}", nome_do_tipo(outra.tipo), outra.nome), false);
    }
    ("O questlog não diz de onde vem.".into(), false)
}

/// O que um clique na lista pede; aplicado depois de desenhar.
enum Acao {
    Prioridade(u32),
    Alertar(u32),
    Tirar(u32),
    Ficha(u32),
}

impl Overlay {
    pub(super) fn eh_desejo(&self, codigo: u32) -> bool {
        self.config.desejos.iter().any(|d| d.codigo == codigo)
    }

    /// Marca (prioridade média, alerta ligado) ou tira o item da lista.
    pub(super) fn alternar_desejo(&mut self, codigo: u32) {
        if self.eh_desejo(codigo) {
            self.config.desejos.retain(|d| d.codigo != codigo);
        } else if self.config.desejos.len() < DESEJOS_MAX {
            self.config.desejos.push(Desejo { codigo, prioridade: PRIORIDADE_PADRAO, alertar: true });
        }
        self.aplicar_config();
    }

    /// A ☆ da ficha: dourada cheia com o item na lista.
    pub(super) fn estrela_do_item(&mut self, ui: &Ui, centro: Pos2, codigo: u32) {
        let marcado = self.eh_desejo(codigo);
        let rect = Rect::from_center_size(centro, Vec2::splat(22.0));
        let resposta =
            ui.interact(rect, ui.id().with(("desejo", codigo)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if resposta.hovered() {
            ui.painter().rect_filled(rect, 4, branco(0x1A));
        }
        let (simbolo, cor) = match (marcado, resposta.hovered()) {
            (true, _) => ("★", visual::DOURADO),
            (false, true) => ("☆", texto()),
            (false, false) => ("☆", branco(0x88)),
        };
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(simbolo.into(), fonte(15.0, false), cor));
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, cor);
        let dica = if marcado {
            "Na sua lista de desejos (★ no cabeçalho). Clique para tirar."
        } else if self.config.desejos.len() >= DESEJOS_MAX {
            "A lista de desejos está cheia (200 itens)."
        } else {
            "Marcar na lista de desejos: a tela Bosses destaca o chefe que derruba o item, e o 🔔 da lista avisa \
             quando ele renasce."
        };
        if resposta.on_hover_text(dica).clicked() {
            self.alternar_desejo(codigo);
        }
    }

    /// "Onde conseguir" na ficha: quem derruba (os 5 de maior chance, com a região), em que baú vem e de
    /// quem o baú cai, a receita com os ingredientes e o resto (missão, dungeon...). Devolve para onde o
    /// clique leva.
    pub(super) fn onde_conseguir(&mut self, ui: &mut Ui, ficha: &DetalheItem) -> Option<Navegar> {
        let fontes = ficha.fontes.clone();
        titulo(ui, "Onde conseguir", branco(0xCC));
        if *fontes == Fontes::default() {
            nota(ui, "O questlog não diz de onde vem este item.", branco(0x99));
            return None;
        }
        let mut navegar = None;
        let (todos, abertas) =
            self.painel.as_ref().map_or((false, HashSet::new()), |p| (p.todos_os_npcs, p.receitas_abertas.clone()));

        let quantos = fontes.npcs.len().min(if todos { NPCS_ABERTOS_MAX } else { NPCS_NA_FICHA });
        for (i, npc) in fontes.npcs[..quantos].iter().enumerate() {
            if let Some(destino) = self.linha_npc(ui, if i == 0 { "Cai de" } else { "" }, npc, i < NPCS_NA_FICHA) {
                navegar = Some(destino);
            }
        }
        if fontes.npcs.len() > NPCS_NA_FICHA {
            let escondidos = &fontes.npcs[NPCS_NA_FICHA..];
            let rotulo = if todos {
                "▾ Esconder os outros monstros".to_string()
            } else {
                format!("▸ e mais {} monstros{}", escondidos.len(), faixa_de_chance(escondidos))
            };
            if self.linha_fonte(ui, "", None, &rotulo, branco(0xAA), "", "", true).clicked()
                && let Some(painel) = &mut self.painel
            {
                painel.todos_os_npcs = !todos;
            }
            if todos && fontes.npcs.len() > NPCS_ABERTOS_MAX {
                let resto = fontes.npcs.len() - NPCS_ABERTOS_MAX;
                nota(ui, &format!("e mais {resto}, com chance menor."), branco(0x88));
            }
        }

        for (i, bau) in fontes.baus.iter().enumerate() {
            let (detalhe, total) = match dados_jogo::item(bau.codigo) {
                Busca::Pronto(do_bau) => match do_bau.fontes.npcs.first() {
                    Some(npc) => {
                        let chance = npc.chance.map_or_else(|| "?".to_string(), porcentagem);
                        let mais = match do_bau.fontes.npcs.len() - 1 {
                            0 => String::new(),
                            n => format!(" e mais {n}"),
                        };
                        (format!("cai de {} ({chance}){mais}", npc.nome), npc.chance.zip(bau.chance).map(|(a, b)| a * b))
                    }
                    None => ("o questlog não diz quem derruba o baú".to_string(), None),
                },
                Busca::Buscando => {
                    self.carregando = true;
                    (String::new(), None)
                }
                Busca::Falhou => (String::new(), None),
            };
            let direita = match (total, bau.chance) {
                (Some(total), _) => porcentagem(total),
                (None, Some(no_bau)) => format!("{} no baú", porcentagem(no_bau)),
                (None, None) => "?".into(),
            };
            let icone = bau.icone.as_deref().map(|i| (i, ICONE_INTEIRO));
            let rotulo = if i == 0 { "No baú" } else { "" };
            let resposta = self.linha_fonte(ui, rotulo, icone, &bau.nome, raridade(bau.raridade).1, &detalhe, &direita, true);
            let no_bau = bau.chance.map_or_else(|| "?".to_string(), porcentagem);
            let conta = if total.is_some() {
                format!("\n{direita} por morte: a chance do baú no chefe × {no_bau} dentro dele (supondo independentes).")
            } else {
                String::new()
            };
            let dica = format!("{}\n{no_bau} dentro do baú.{conta}\n\nClique para ver a ficha do baú.", bau.nome);
            if resposta.on_hover_text(dica).clicked() {
                navegar = Some(Navegar::Item(bau.codigo, bau.chance));
            }
        }

        for (i, receita) in fontes.receitas.iter().enumerate() {
            let aberta = abertas.contains(&receita.codigo);
            let ingredientes =
                receita.entradas.iter().map(|(item, q)| format!("{q}× {}", item.nome)).collect::<Vec<_>>().join(", ");
            let nome = format!("{} {}", if aberta { "▾" } else { "▸" }, profissao(&receita.profissao));
            let rotulo = if i == 0 { "Craft" } else { "" };
            let resposta = self.linha_fonte(ui, rotulo, None, &nome, texto(), &ingredientes, "", true);
            let dica = format!(
                "Profissão como o questlog dá (em inglês).\n{ingredientes}\n\nClique para {} os ingredientes e a maestria.",
                if aberta { "esconder" } else { "ver" }
            );
            if resposta.on_hover_text(dica).clicked()
                && let Some(painel) = &mut self.painel
                && !painel.receitas_abertas.remove(&receita.codigo)
            {
                painel.receitas_abertas.insert(receita.codigo);
            }
            if !aberta {
                continue;
            }
            match dados_jogo::receita(receita.codigo) {
                Busca::Pronto(r) => {
                    let mut partes = Vec::new();
                    if let Some(nivel) = r.nivel_maestria {
                        let grau = r.maestria.map_or_else(String::new, |m| format!(" ({m})"));
                        partes.push(format!("maestria {nivel}{grau}"));
                    }
                    partes.extend(r.raca.as_deref().map(raca).filter(|r| !r.is_empty()));
                    if !partes.is_empty() {
                        self.linha_fonte(ui, "", None, &partes.join("   "), branco(0xAA), "", "", false);
                    }
                }
                Busca::Buscando => self.carregando = true,
                Busca::Falhou => nota(ui, "O questlog não mandou a maestria desta receita.", branco(0x88)),
            }
            for (item, quantidade) in &receita.entradas {
                let icone = item.icone.as_deref().map(|i| (i, ICONE_INTEIRO));
                let nome = format!("{quantidade}× {}", item.nome);
                let resposta = self.linha_fonte(ui, "", icone, &nome, raridade(item.raridade).1, "", "", true);
                if resposta.on_hover_text("Clique para ver a ficha do ingrediente.").clicked() {
                    navegar = Some(Navegar::Item(item.codigo, None));
                }
            }
        }

        for tipo in TIPOS {
            let lista: Vec<_> = fontes.outras.iter().filter(|o| o.tipo == tipo).collect();
            for (i, outra) in lista.iter().take(OUTRAS_POR_TIPO).enumerate() {
                let rotulo = if i == 0 { nome_do_tipo(tipo) } else { "" };
                let direita = outra.chance.map(porcentagem).unwrap_or_default();
                self.linha_fonte(ui, rotulo, None, &outra.nome, texto(), "", &direita, false);
            }
            if lista.len() > OUTRAS_POR_TIPO {
                nota(ui, &format!("e mais {}", lista.len() - OUTRAS_POR_TIPO), branco(0x88));
            }
        }
        ui.add_space(4.0);
        nota(ui, ORIGEM_FONTES, branco(0x66));
        navegar
    }

    /// Um monstro que derruba: nível, região (só nos 5 primeiros) e se é chefe de campo de uma região
    /// já vista; o chefe abre os drops dele.
    fn linha_npc(&mut self, ui: &mut Ui, rotulo: &str, npc: &FonteNpc, com_regiao: bool) -> Option<Navegar> {
        let chefe = self.regioes.values().flat_map(|r| &r.chefes).any(|c| c.codigo == npc.codigo);
        let mut detalhe = Vec::new();
        if npc.nivel > 0 {
            detalhe.push(format!("nível {}", npc.nivel));
        }
        if com_regiao {
            match dados_jogo::regioes_do_npc(npc.codigo) {
                Busca::Pronto(regioes) if !regioes.is_empty() => detalhe.push(regioes.join(", ")),
                Busca::Buscando => self.carregando = true,
                _ => {}
            }
        }
        if chefe {
            detalhe.push("chefe de campo".into());
        }
        let mut direita = npc.chance.map_or_else(|| "?".to_string(), porcentagem);
        if let Some((minimo, maximo)) = npc.quantidade.filter(|(_, maximo)| *maximo > 1) {
            direita = if minimo == maximo { format!("×{maximo}  {direita}") } else { format!("×{minimo} a {maximo}  {direita}") };
        }
        let icone = npc.retrato.as_deref().map(|r| (r, eventos::ROSTO));
        let resposta = self.linha_fonte(ui, rotulo, icone, &npc.nome, texto(), &detalhe.join("   "), &direita, chefe);
        let clique = if chefe { "\n\nClique para ver os drops dele." } else { "" };
        let resposta = resposta.on_hover_text(format!("{}: {direita} por morte no questlog.{clique}", npc.nome));
        (chefe && resposta.clicked()).then_some(Navegar::Chefe(npc.codigo))
    }

    /// Linha da "Onde conseguir": o rótulo na coluna da esquerda (só na primeira de cada tipo), o ícone,
    /// o nome e embaixo o detalhe, e à direita a chance.
    #[allow(clippy::too_many_arguments)]
    fn linha_fonte(
        &mut self,
        ui: &mut Ui,
        rotulo: &str,
        icone: Option<(&str, [f32; 4])>,
        nome: &str,
        cor: Color32,
        detalhe: &str,
        direita: &str,
        clicavel: bool,
    ) -> Response {
        let largura = ui.available_width();
        let altura = if detalhe.is_empty() { 22.0 } else { 34.0 };
        let (rect, resposta) =
            ui.allocate_exact_size(vec2(largura, altura), if clicavel { Sense::click() } else { Sense::hover() });
        if clicavel && resposta.hovered() {
            ui.painter().rect_filled(rect, 3, branco(0x0C));
        }
        let meio = rect.min.y + 11.0;
        if !rotulo.is_empty() {
            let mut job = LayoutJob::default();
            trecho(&mut job, rotulo, 10.0, false, branco(0x88));
            let galley = montar(ui, job);
            ui.painter().galley(pos2(rect.min.x + 2.0, meio - galley.size().y / 2.0), galley, texto());
        }
        let mut x = rect.min.x + COLUNA;
        if let Some((imagem, recorte)) = icone {
            let quadrado = Rect::from_center_size(pos2(x + 9.0, meio), Vec2::splat(18.0));
            self.imagem_web(ui, imagem, recorte, quadrado);
            x += 24.0;
        }
        let mut job = LayoutJob::default();
        trecho(&mut job, direita, 11.0, true, branco(0xDD));
        let galley_direita = montar(ui, job);
        let mut job = LayoutJob::default();
        trecho(&mut job, nome, 11.0, false, cor);
        job.wrap = uma_linha((rect.max.x - 4.0 - galley_direita.size().x - 8.0 - x).max(40.0));
        let galley_nome = montar(ui, job);
        let pintor = ui.painter();
        pintor.galley(pos2(x, meio - galley_nome.size().y / 2.0), galley_nome, texto());
        let fim = rect.max.x - 4.0 - galley_direita.size().x;
        pintor.galley(pos2(fim, meio - galley_direita.size().y / 2.0), galley_direita, texto());
        if !detalhe.is_empty() {
            let mut job = LayoutJob::default();
            trecho(&mut job, detalhe, 10.0, false, branco(0x88));
            job.wrap = uma_linha((rect.max.x - 4.0 - x).max(40.0));
            let galley = montar(ui, job);
            ui.painter().galley(pos2(x, meio + 7.0), galley, texto());
        }
        if clicavel { resposta.on_hover_cursor(CursorIcon::PointingHand) } else { resposta }
    }

    /// Abre a lista de desejos; uma falha antes (sem internet) não impede de tentar de novo.
    pub(super) fn abrir_desejos(&mut self) {
        self.tela = Tela::Desejos;
        dados_jogo::repetir_falhas();
    }

    /// Os chefes da região que derrubam um desejo; None com o destaque desligado, sem desejo ou antes
    /// da lista da região.
    pub(super) fn desejos_da_regiao(&self) -> Option<DesejosNaRegiao> {
        if !self.config.desejos_destacar || self.config.desejos.is_empty() {
            return None;
        }
        let regiao = self.chefes.as_ref().and_then(|(lista, _)| self.regioes.get(&lista.regiao))?;
        let codigos = self.config.desejos.iter().map(|d| d.codigo);
        Some(desejos_na_regiao(regiao, codigos, chance_minima(&self.config), dados_jogo::item))
    }

    pub(super) fn tela_desejos(&mut self, ui: &mut Ui) {
        let titulo_tela = format!("Lista de desejos ({})", self.config.desejos.len());
        ui.horizontal(|ui| {
            ui.label(RichText::new(titulo_tela).font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "Voltar", true).on_hover_text("Volta ao medidor").clicked() {
                    self.tela = Tela::Medidor;
                }
            });
        });
        ui.add_space(4.0);
        let explicacao = "Os itens que você procura e de onde cada um vem, pelo questlog. Marque na ☆ da ficha de um \
                          item (Bosses ♛ › chefe › item). A tela Bosses destaca o chefe que derruba um desejo, e o 🔔 \
                          avisa quando ele renasce. Clique num item para ver de onde ele vem.";
        ui.add(eframe::egui::Label::new(RichText::new(explicacao).font(fonte(10.0, false)).color(branco(0x88))).wrap());
        ui.add_space(6.0);
        if self.config.desejos.is_empty() {
            let vazio = "Nenhum item na lista ainda.";
            ui.label(RichText::new(vazio).font(fonte(12.0, false)).color(texto().gamma_multiply(0.6)));
            return;
        }

        let fontes = fontes_dos_desejos(self.config.desejos.iter().map(|d| d.codigo), dados_jogo::item);
        let vistos = self.chefes_da_regiao();
        let regiao = self.chefes.as_ref().and_then(|(lista, _)| self.regioes.get(&lista.regiao)).cloned();
        let minima = chance_minima(&self.config);
        let agora = nucleo::agora();
        let mut desejos = self.config.desejos.clone();
        let nome = |d: &Desejo| fontes.fichas.get(&d.codigo).map(|f| f.nome.clone()).unwrap_or_default();
        desejos.sort_by(|a, b| a.prioridade.cmp(&b.prioridade).then_with(|| nome(a).cmp(&nome(b))));

        let mut acao = None;
        ScrollArea::vertical().id_salt("desejos").max_height(LISTA_MAX).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for desejo in &desejos {
                let ficha = fontes.fichas.get(&desejo.codigo);
                // O chefe da região com a maior chance de derrubar este, acima da mínima.
                let na_regiao = ficha.zip(regiao.as_ref()).and_then(|(ficha, regiao)| {
                    let itens = [(desejo.codigo, ficha.fontes.clone())];
                    let (npc, chance) = chefes_com_desejo(regiao, &itens, &fontes.baus, minima)
                        .into_iter()
                        .filter_map(|(npc, lista)| Some((npc, lista.first()?.1)))
                        .max_by(|a, b| a.1.total_cmp(&b.1))?;
                    let chefe = vistos.as_ref()?.chefes.iter().find(|c| c.codigo == Some(npc))?;
                    Some((chefe, chance))
                });
                let resumo = ficha.map(|f| resumo_da_fonte(&f.fontes, &fontes.baus, na_regiao, agora));
                if let Some(a) = self.linha_desejo(ui, desejo, ficha, resumo) {
                    acao = Some(a);
                }
            }
        });
        if fontes.falharam > 0 {
            ui.add_space(4.0);
            let aviso = "O questlog não respondeu para alguns itens. Feche e abra a lista para tentar de novo.";
            nota(ui, aviso, branco(0x99));
        }
        match acao {
            Some(Acao::Prioridade(codigo)) => {
                if let Some(d) = self.config.desejos.iter_mut().find(|d| d.codigo == codigo) {
                    d.prioridade = d.prioridade % 3 + 1;
                }
                self.aplicar_config();
            }
            Some(Acao::Alertar(codigo)) => {
                if let Some(d) = self.config.desejos.iter_mut().find(|d| d.codigo == codigo) {
                    d.alertar = !d.alertar;
                }
                self.aplicar_config();
            }
            Some(Acao::Tirar(codigo)) => {
                if self.desejo_tirando.is_some_and(|(qual, desde)| qual == codigo && desde.elapsed() < CONFIRMAR_EM) {
                    self.desejo_tirando = None;
                    self.config.desejos.retain(|d| d.codigo != codigo);
                    self.aplicar_config();
                } else {
                    self.desejo_tirando = Some((codigo, Instant::now()));
                }
            }
            Some(Acao::Ficha(codigo)) => self.alternar_ficha(codigo, ui.ctx().pixels_per_point()),
            None => {}
        }
    }

    /// Ícone, nome e embaixo a melhor fonte; à direita a prioridade (o clique troca), o 🔔 e o ✕.
    fn linha_desejo(
        &mut self,
        ui: &mut Ui,
        desejo: &Desejo,
        ficha: Option<&DetalheItem>,
        resumo: Option<(String, bool)>,
    ) -> Option<Acao> {
        let codigo = desejo.codigo;
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 38.0), Sense::click());
        let aberta = self.painel.as_ref().is_some_and(|p| p.codigo.is_none() && p.ficha() == Some(codigo));
        if aberta || resposta.hovered() {
            ui.painter().rect_filled(rect, 3, branco(if aberta { 0x22 } else { 0x0C }));
        }
        let meio = rect.center().y;
        let quadrado = Rect::from_center_size(pos2(rect.min.x + 17.0, meio), Vec2::splat(30.0));
        match ficha.and_then(|f| f.icone.as_deref()) {
            Some(icone) => self.imagem_web(ui, icone, ICONE_INTEIRO, quadrado),
            None if ficha.is_none() && resumo.is_none() => {
                self.carregando = true;
                ui.painter().rect_filled(quadrado, 3, pulso(ui));
            }
            None => {
                ui.painter().rect_filled(quadrado, 3, branco(0x22));
            }
        }

        // Da direita para a esquerda: ✕, 🔔 e a prioridade.
        let mut acao = None;
        let tirando = self.desejo_tirando.is_some_and(|(qual, desde)| qual == codigo && desde.elapsed() < CONFIRMAR_EM);
        let (simbolo, tamanho) = if tirando { ("Tirar?", 10.0) } else { ("✕", 11.0) };
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(simbolo.into(), fonte(tamanho, tirando), branco(0xAA)));
        let caixa = Rect::from_center_size(
            pos2(rect.max.x - 4.0 - (galley.size().x + 10.0) / 2.0, meio),
            vec2(galley.size().x + 10.0, 20.0),
        );
        let x_ = ui.interact(caixa, ui.id().with(("tirar_desejo", codigo)), Sense::click());
        let cor = if tirando { visual::VERMELHO_CLARO } else if x_.hovered() { texto() } else { branco(0x66) };
        if x_.hovered() {
            ui.painter().rect_filled(caixa, 4, branco(0x1A));
        }
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(simbolo.into(), fonte(tamanho, tirando), cor));
        ui.painter().galley(caixa.center() - galley.size() / 2.0, galley, cor);
        let dica = if tirando { "Clique de novo para tirar da lista" } else { "Tirar da lista" };
        if x_.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            acao = Some(Acao::Tirar(codigo));
        }

        let sino = visual::sino(ui, pos2(caixa.min.x - 14.0, meio), desejo.alertar, ui.id().with(("sino_desejo", codigo)));
        let sobre_o_sino = if desejo.alertar {
            format!(
                "Alerta ligado: {} min antes de renascer um chefe que derruba este item com {} ou mais, e na hora. \
                 Clique para desligar.",
                self.config.alertas.chefes_antes_min,
                chance_da_config(self.config.desejos_chance_minima)
            )
        } else {
            "Ligar o alerta: avisa quando renasce um chefe que derruba este item.".to_string()
        };
        if sino.on_hover_text(sobre_o_sino).clicked() {
            acao = Some(Acao::Alertar(codigo));
        }

        let cor_prioridade = match desejo.prioridade {
            1 => visual::DOURADO,
            3 => branco(0x77),
            _ => branco(0xBB),
        };
        let galley =
            ui.fonts_mut(|f| f.layout_no_wrap(nome_da_prioridade(desejo.prioridade).into(), fonte(10.0, true), cor_prioridade));
        let chip = Rect::from_center_size(pos2(caixa.min.x - 28.0 - 24.0, meio), vec2(44.0, 18.0));
        let clique = ui.interact(chip, ui.id().with(("prioridade", codigo)), Sense::click());
        ui.painter().rect_filled(chip, 9, branco(if clique.hovered() { 0x2A } else { 0x16 }));
        ui.painter().galley(chip.center() - galley.size() / 2.0, galley, cor_prioridade);
        let dica = format!("Prioridade {}: a lista começa pelas altas. Clique para trocar.", nome_da_prioridade(desejo.prioridade));
        if clique.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            acao = Some(Acao::Prioridade(codigo));
        }

        let x = quadrado.max.x + 8.0;
        let limite = (chip.min.x - 8.0 - x).max(40.0);
        let (nome, cor) = match ficha {
            Some(f) => (f.nome.clone(), raridade(f.raridade).1),
            None => (format!("Item {codigo}"), branco(0x99)),
        };
        let mut job = LayoutJob::default();
        trecho(&mut job, &nome, 12.0, false, cor);
        job.wrap = uma_linha(limite);
        let galley_nome = montar(ui, job);
        let (linha, destaque) = match (&resumo, ficha) {
            (Some((texto_resumo, destaque)), _) => (texto_resumo.clone(), *destaque),
            (None, None) => ("Buscando no questlog...".into(), false),
            (None, Some(_)) => (String::new(), false),
        };
        let mut job = LayoutJob::default();
        trecho(&mut job, &linha, 10.0, destaque, if destaque { visual::DOURADO } else { branco(0x88) });
        job.wrap = uma_linha(limite);
        let galley_linha = montar(ui, job);
        let pintor = ui.painter();
        pintor.galley(pos2(x, meio - 1.0 - galley_nome.size().y), galley_nome, texto());
        pintor.galley(pos2(x, meio + 1.0), galley_linha, texto());

        let dica = format!("{nome}\n{linha}\n\nClique para ver de onde vem.");
        if acao.is_none() && resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            acao = Some(Acao::Ficha(codigo));
        }
        acao
    }
}

#[cfg(test)]
mod testes {
    use nucleo::medicao::catalogo::{ler_detalhe, ler_receita, ler_regiao, ler_regioes_npc};
    use serde_json::Value;

    use super::*;

    /// Respostas reais do questlog (2026-10-08; o Peitoral da Fantasia de 2026-10-06), cortadas pelo
    /// docs/0.14.0/F7-gerar_fixture.py: só os campos que estes testes leem.
    fn dados() -> Value {
        serde_json::from_str(include_str!("desejos-teste.json")).unwrap()
    }

    fn fontes(dados: &Value, item: &str) -> Arc<Fontes> {
        ler_detalhe(&dados[item]).unwrap().fontes
    }

    fn altgard(dados: &Value) -> InfoRegiao {
        ler_regiao(&dados["regiao_altgard"]).unwrap().1
    }

    #[test]
    fn onde_conseguir_direto() {
        let f = fontes(&dados(), "luvas_newbold_do_chefe");
        assert_eq!(f.npcs.len(), 1);
        let npc = &f.npcs[0];
        assert_eq!((npc.codigo, npc.nome.as_str(), npc.nivel), (2400424, "Profanador Newbold", 45));
        assert_eq!(npc.chance, Some(0.1734104));
        assert!(f.baus.is_empty() && f.receitas.is_empty() && f.outras.is_empty());
    }

    #[test]
    fn onde_conseguir_pelo_bau() {
        let dados = dados();
        let item = fontes(&dados, "luvas_newbold_do_bau");
        assert!(item.npcs.is_empty(), "a 210540129 não cai direto do chefe");
        assert_eq!(item.baus.iter().map(|b| (b.codigo, b.chance)).collect::<Vec<_>>(), [(533700387, Some(0.035714285))]);
        let baus = HashMap::from([(533700387, fontes(&dados, "bau_newbold"))]);
        // Na chance mínima padrão da config (0,5%): os 3,6% do baú passam.
        let minima = chance_minima(&Config::default());
        let chefes = chefes_com_desejo(&altgard(&dados), &[(210540129, item)], &baus, minima);
        assert_eq!(chefes.len(), 1);
        let desejos = &chefes[&2400424];
        assert_eq!(desejos.len(), 1);
        assert_eq!(desejos[0].0, 210540129);
        assert!((desejos[0].1 - 0.035714285).abs() < 1e-9, "{}", desejos[0].1);
    }

    #[test]
    fn craft_com_ingredientes_e_maestria() {
        let dados = dados();
        let f = fontes(&dados, "pedra_de_mana_craft");
        assert_eq!(f.receitas.len(), 4);
        assert!(f.receitas.iter().all(|r| r.profissao == "alchemy"));
        let entradas: Vec<(&str, u32)> = f.receitas[0].entradas.iter().map(|(i, q)| (i.nome.as_str(), *q)).collect();
        assert_eq!(entradas, [("Pedra de Mana Inferior", 5), ("Pó de Pedra Espiritual", 3), ("Tinta Avançada (Vinculado)", 2)]);
        assert_eq!(f.receitas[0].codigo, 314046001);
        let receita = ler_receita(&dados["receita_pedra_de_mana"]).unwrap();
        assert_eq!(receita.maestria.as_deref(), Some("beginner"));
        assert_eq!(receita.nivel_maestria, Some(20));
        assert_eq!(receita.raca.as_deref(), Some("light"));
    }

    #[test]
    fn regiao_do_monstro() {
        assert_eq!(ler_regioes_npc(&dados()["npc_fada"]), ["Altgard"]);
    }

    #[test]
    fn chance_minima_tira_o_ruido_do_item_generico() {
        let dados = dados();
        let peitoral = fontes(&dados, "peitoral_fantasia");
        let desejos = [(210130005, peitoral)];
        let regiao = altgard(&dados);
        let todos = chefes_com_desejo(&regiao, &desejos, &HashMap::new(), 0.0);
        // 23 NPCs na fixture: os 20 chefes de Altgard e 3 mobs comuns, que não entram.
        assert_eq!(todos.len(), 20);
        let padrao = chefes_com_desejo(&regiao, &desejos, &HashMap::new(), chance_minima(&Config::default()));
        assert!(padrao.is_empty(), "{padrao:?}");
    }

    #[test]
    fn so_relacoes_que_dao_o_item_contam_como_fonte() {
        // Do getItem da Pedra de Refino (2026-10-06): o pedido de suprimento a pede (uso, não fonte).
        let item: Value = serde_json::from_str(
            r#"{"id":"610530001","name":"Pedra de Refino","grade":11,
                "itemIsRequiredBySupplyRequests":[{"id":"1011152","name":"Pedra de Refino"}],
                "itemIsRewardOfDungeons":[{"id":"600001","name":"Caverna de Krao","chance":1}]}"#,
        )
        .unwrap();
        let f = ler_detalhe(&item).unwrap().fontes;
        let outras: Vec<_> = f.outras.iter().map(|o| (o.tipo, o.codigo, o.nome.as_str(), o.chance)).collect();
        assert_eq!(outras, [(nucleo::medicao::catalogo::TipoFonte::Dungeon, 600001, "Caverna de Krao", Some(1.0))]);
    }
}
