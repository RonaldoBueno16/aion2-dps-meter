//! Ficha do item no painel de drops, como a dica do jogo: nível, atributos, os da alma, encaixes,
//! a ajuda e o nível mínimo. Pelo questlog (getItem), com o nome de cada atributo em português que o
//! statFormat dele traz; tudo só na memória.

use std::collections::HashMap;

use eframe::egui::text::LayoutJob;
use eframe::egui::{CursorIcon, Rect, Sense, Ui, Vec2, pos2, vec2};
use nucleo::formato::n;
use nucleo::medicao::catalogo::{Atributo, Busca, DetalheItem};
use nucleo::medicao::dados_jogo;

/// Nomes e formatos dos atributos pelo id.
type Atributos = HashMap<String, Atributo>;

use super::drops::{porcentagem, raridade};
use super::{Overlay, branco, montar, pulso, texto, trecho, uma_linha};

const ICONE_INTEIRO: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// "92", "1.234", "8,8": até duas casas, sem zero sobrando.
fn numero(valor: f64) -> String {
    let texto = n(valor, 2);
    match texto.split_once(',') {
        Some((inteira, casas)) if casas.trim_end_matches('0').is_empty() => inteira.to_string(),
        Some((inteira, casas)) => format!("{inteira},{}", casas.trim_end_matches('0')),
        None => texto,
    }
}

/// Valor como o questlog mostra: em % (o cru vem em centésimos) quando o atributo é de porcentagem;
/// Energia de Voo e Vigor vêm em centésimos de ponto.
pub(super) fn valor_do_atributo(id: &str, valor: f64, atributo: Option<&Atributo>) -> String {
    let valor = if matches!(id, "fpmax" | "spmax") { valor * 0.01 } else { valor };
    match atributo {
        Some(a) if a.porcentagem => format!("{}%", numero(valor / 100.0)),
        _ => numero(valor),
    }
}

/// Uma linha da ficha: nome, valor e a ajuda do atributo.
pub(super) struct LinhaAtributo {
    pub nome: String,
    pub valor: String,
    pub ajuda: String,
}

fn linha(id: &str, valor: String, atributos: Option<&Atributos>) -> LinhaAtributo {
    let atributo = atributos.and_then(|a| a.get(id));
    LinhaAtributo {
        nome: atributo.map_or_else(|| id.to_string(), |a| a.nome.clone()),
        valor,
        ajuda: atributo.map(|a| a.descricao.clone()).unwrap_or_default(),
    }
}

/// Atributos principais na ordem do jogo (sortOrder).
pub(super) fn linhas_principais(ficha: &DetalheItem, atributos: Option<&Atributos>) -> Vec<LinhaAtributo> {
    let ordem = |id: &str| atributos.and_then(|a| a.get(id)).map_or(i64::MAX, |a| a.ordem);
    let mut principais: Vec<&(String, f64)> = ficha.principais.iter().collect();
    principais.sort_by(|a, b| ordem(&a.0).cmp(&ordem(&b.0)).then_with(|| a.0.cmp(&b.0)));
    principais
        .into_iter()
        .map(|(id, valor)| linha(id, valor_do_atributo(id, *valor, atributos.and_then(|a| a.get(id))), atributos))
        .collect()
}

/// Atributos da alma na ordem do questlog (a mesma da dica do jogo); faixa quando o valor é sorteado.
pub(super) fn linhas_da_alma(ficha: &DetalheItem, atributos: Option<&Atributos>) -> Vec<LinhaAtributo> {
    ficha
        .alma
        .iter()
        .map(|(id, minimo, maximo)| {
            let formato = atributos.and_then(|a| a.get(id));
            let (menor, maior) = (valor_do_atributo(id, *minimo, formato), valor_do_atributo(id, *maximo, formato));
            linha(id, if menor == maior { maior } else { format!("{menor} a {maior}") }, atributos)
        })
        .collect()
}

impl Overlay {
    /// A ficha no lugar da lista de drops. Devolve true no clique de "‹ Drops".
    pub(super) fn ficha_do_item(&mut self, ui: &mut Ui, codigo: u32, chance: Option<f64>) -> bool {
        let voltar = botao_voltar(ui);
        ui.add_space(4.0);
        let ficha = match dados_jogo::item(codigo) {
            Busca::Pronto(ficha) => ficha,
            Busca::Buscando => {
                self.esqueleto_da_ficha(ui);
                return voltar;
            }
            Busca::Falhou => {
                let aviso = "O questlog não respondeu. Feche e abra o painel para tentar de novo.";
                super::drops::nota(ui, aviso, branco(0x99));
                return voltar;
            }
        };
        let atributos = match dados_jogo::atributos() {
            Busca::Pronto(a) => Some(a),
            Busca::Buscando => {
                self.carregando = true;
                None
            }
            Busca::Falhou => None,
        };
        let atributos = atributos.as_deref();

        // Ícone, nome na cor da raridade e, embaixo, a raridade (onde conferida) e o nível do item.
        let (nome_raridade, cor) = raridade(ficha.raridade);
        let largura = ui.available_width();
        let mut job = LayoutJob::default();
        trecho(&mut job, &ficha.nome, 13.0, true, cor);
        job.wrap.max_width = largura - 52.0;
        let nome = montar(ui, job);
        let mut job = LayoutJob::default();
        if let Some(r) = nome_raridade {
            trecho(&mut job, &format!("{r}   "), 10.0, true, cor);
        }
        if let Some(nivel) = ficha.nivel_item {
            trecho(&mut job, &format!("Nível do item {nivel}"), 10.0, false, branco(0xCC));
        }
        let detalhe = montar(ui, job);
        let altura = (nome.size().y + detalhe.size().y + 4.0).max(44.0);
        let (rect, _) = ui.allocate_exact_size(vec2(largura, altura), Sense::hover());
        let quadrado = Rect::from_min_size(rect.min + vec2(0.0, 2.0), Vec2::splat(40.0));
        match ficha.icone.as_deref() {
            Some(icone) => self.imagem_web(ui, icone, ICONE_INTEIRO, quadrado),
            None => {
                ui.painter().rect_filled(quadrado, 3, branco(0x22));
            }
        }
        let x = quadrado.max.x + 10.0;
        let altura_nome = nome.size().y;
        ui.painter().galley(pos2(x, rect.min.y + 2.0), nome, texto());
        ui.painter().galley(pos2(x, rect.min.y + 4.0 + altura_nome), detalhe, texto());

        let principais = linhas_principais(&ficha, atributos);
        if !principais.is_empty() {
            super::drops::titulo(ui, "Atributos", branco(0xCC));
            for l in &principais {
                linha_da_ficha(ui, l);
            }
        }
        if !ficha.alma.is_empty() {
            let titulo = match ficha.alma_sorteia {
                Some(1) => "Ao vincular a alma: 1 sorteado destes".to_string(),
                Some(k) => format!("Ao vincular a alma: {k} sorteados destes"),
                None => "Ao vincular a alma".to_string(),
            };
            super::drops::titulo(ui, &titulo, branco(0xCC));
            for l in &linhas_da_alma(&ficha, atributos) {
                linha_da_ficha(ui, l);
            }
        }
        if ficha.pedras_de_mana > 0 || ficha.pedras_divinas > 0 {
            super::drops::titulo(ui, "Encaixes", branco(0xCC));
            let encaixes = |n: u32| if n == 1 { "1 encaixe".to_string() } else { format!("{n} encaixes") };
            if ficha.pedras_de_mana > 0 {
                let valor = encaixes(ficha.pedras_de_mana);
                linha_da_ficha(ui, &LinhaAtributo { nome: "Pedra de Mana".into(), valor, ajuda: String::new() });
            }
            if ficha.pedras_divinas > 0 {
                let valor = encaixes(ficha.pedras_divinas);
                linha_da_ficha(ui, &LinhaAtributo { nome: "Pedra Divina".into(), valor, ajuda: String::new() });
            }
        }
        if let Some(descricao) = &ficha.descricao {
            ui.add_space(6.0);
            super::drops::nota(ui, descricao, branco(0xCC));
        }
        ui.add_space(6.0);
        let mut rodape = Vec::new();
        if let Some(nivel) = ficha.nivel_minimo.filter(|n| *n > 1) {
            rodape.push(format!("Nível mínimo de uso {nivel}"));
        }
        match ficha.negociavel {
            Some(true) => rodape.push("Negociável".into()),
            Some(false) => rodape.push("Não negociável".into()),
            None => {}
        }
        if let Some(chance) = chance {
            rodape.push(format!("Chance aqui: {} no questlog", porcentagem(chance)));
        }
        for texto_rodape in rodape {
            super::drops::nota(ui, &texto_rodape, branco(0x99));
        }
        voltar
    }

    fn esqueleto_da_ficha(&mut self, ui: &mut Ui) {
        self.carregando = true;
        let cor = pulso(ui);
        let largura = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(largura, 48.0), Sense::hover());
        let pintor = ui.painter();
        pintor.rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 2.0), Vec2::splat(40.0)), 3, cor);
        pintor.rect_filled(Rect::from_min_size(rect.min + vec2(50.0, 6.0), vec2(180.0, 12.0)), 3, cor);
        pintor.rect_filled(Rect::from_min_size(rect.min + vec2(50.0, 26.0), vec2(110.0, 9.0)), 3, cor);
        for i in 0..5 {
            let (linha, _) = ui.allocate_exact_size(vec2(largura, 20.0), Sense::hover());
            let meio = linha.center().y;
            let pintor = ui.painter();
            let nome = Rect::from_min_size(pos2(linha.min.x, meio - 4.0), vec2(90.0 + 15.0 * i as f32, 8.0));
            pintor.rect_filled(nome, 3, cor);
            pintor.rect_filled(Rect::from_min_size(pos2(linha.max.x - 40.0, meio - 4.0), vec2(36.0, 8.0)), 3, cor);
        }
    }
}

fn botao_voltar(ui: &mut Ui) -> bool {
    let largura = ui.available_width();
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 20.0), Sense::click());
    if resposta.hovered() {
        ui.painter().rect_filled(rect, 3, branco(0x0C));
    }
    let mut job = LayoutJob::default();
    trecho(&mut job, "‹ Drops", 11.0, true, if resposta.hovered() { texto() } else { branco(0xAA) });
    let galley = montar(ui, job);
    ui.painter().galley(pos2(rect.min.x + 2.0, rect.center().y - galley.size().y / 2.0), galley, texto());
    resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Volta à lista de drops").clicked()
}

/// Nome à esquerda, valor à direita; a ajuda do atributo no mouse.
fn linha_da_ficha(ui: &mut Ui, l: &LinhaAtributo) {
    let largura = ui.available_width();
    let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 20.0), Sense::hover());
    let meio = rect.center().y;
    let mut job = LayoutJob::default();
    trecho(&mut job, &l.valor, 11.0, true, texto());
    let valor = montar(ui, job);
    let mut job = LayoutJob::default();
    trecho(&mut job, &l.nome, 11.0, false, branco(0xBB));
    job.wrap = uma_linha((largura - valor.size().x - 16.0).max(40.0));
    let nome = montar(ui, job);
    let pintor = ui.painter();
    pintor.galley(pos2(rect.min.x + 2.0, meio - nome.size().y / 2.0), nome, texto());
    pintor.galley(pos2(rect.max.x - 4.0 - valor.size().x, meio - valor.size().y / 2.0), valor, texto());
    if !l.ajuda.is_empty() {
        resposta.on_hover_text(&l.ajuda);
    }
}

#[cfg(test)]
mod testes {
    use nucleo::medicao::catalogo::{ler_atributos, ler_detalhe};
    use serde_json::Value;

    use super::*;

    /// Trechos reais do questlog (2026-10-06): o getItem do Guarda-braço da Alma Forjada (o do print
    /// do jogo) e do Montante da Fantasia, e o statFormat em português dos atributos deles.
    fn dados() -> Value {
        serde_json::from_str(include_str!("item-teste.json")).unwrap()
    }

    fn pares(linhas: &[LinhaAtributo]) -> Vec<(&str, &str)> {
        linhas.iter().map(|l| (l.nome.as_str(), l.valor.as_str())).collect()
    }

    #[test]
    fn ficha_do_guarda_braco_igual_a_dica_do_jogo() {
        let dados = dados();
        let atributos = ler_atributos(&dados["atributos"]);
        let ficha = ler_detalhe(&dados["guarda"]).unwrap();
        assert_eq!(ficha.nome, "Guarda-braço da Alma Forjada");
        assert_eq!(
            pares(&linhas_principais(&ficha, Some(&atributos))),
            [("Ataque", "92"), ("Precisão", "50"), ("Acerto Crítico", "50")]
        );
        assert_eq!(
            pares(&linhas_da_alma(&ficha, Some(&atributos))),
            [("Poder", "10"), ("Precisão", "30"), ("Bloqueio", "15"), ("PV", "120")]
        );
        assert_eq!((ficha.nivel_item, ficha.nivel_minimo), (Some(45), Some(45)));
        assert_eq!((ficha.pedras_de_mana, ficha.pedras_divinas, ficha.alma_sorteia), (4, 1, None));
        assert_eq!(ficha.negociavel, Some(false));
    }

    #[test]
    fn atributo_de_porcentagem_vem_em_centesimos() {
        let dados = dados();
        let atributos = ler_atributos(&dados["atributos"]);
        let ficha = ler_detalhe(&dados["montante"]).unwrap();
        let alma = linhas_da_alma(&ficha, Some(&atributos));
        let velocidade = alma.iter().find(|l| l.nome == "Velocidade de Combate").unwrap();
        assert_eq!(velocidade.valor, "7,59% a 8,8%");
        assert_eq!(ficha.alma_sorteia, Some(4));
        let principais = linhas_principais(&ficha, Some(&atributos));
        let reducao = principais.iter().find(|l| l.nome == "Redução de Dano de Bloqueio com Arma").unwrap();
        assert_eq!(reducao.valor, "100.000");
    }

    #[test]
    fn ajuda_do_atributo_sem_as_tags() {
        let atributos = ler_atributos(&dados()["atributos"]);
        let critico = &atributos["critical"].descricao;
        assert!(critico.starts_with("Aumenta a Chance de Acerto Crítico."), "{critico}");
        assert!(!critico.contains('<') && !critico.contains('>'), "{critico}");
    }
}
