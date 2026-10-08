//! Bosses: os chefes de campo da região em que você está, como o servidor manda no 0x9101 a cada
//! poucos segundos, com o mapa aberto ou não (vivo, ou a hora de renascer). Nome, nível e retrato vêm
//! do questlog, pela ordem dos NPCs nomeados da região (conferida em 3 dos 24 de Altgard).

use std::collections::{BTreeSet, HashMap};

use eframe::egui::{Align, CursorIcon, Layout, Rect, RichText, Sense, Ui, Vec2, pos2, vec2};
use eframe::egui::text::LayoutJob;
use nucleo::medicao::catalogo::InfoRegiao;
use nucleo::medicao::dados_jogo;
use nucleo::protocolo::combate::ChefesDeCampo;
use nucleo::{Hora, TICKS_POR_SEGUNDO};

use super::desejos::desejos_na_regiao;
use super::drops::porcentagem;
use super::{
    IconeDaLinha, LinhaEvento, Overlay, Tela, botao, branco, fonte, hora_local, montar, texto, trecho, visual,
};
use crate::alertas::ChefeMarcado;
use crate::eventos;

/// Sem 0x9101 por mais que isso (fora da região, sem conexão), a lista é de antes: quem passou da
/// hora de renascer conta como vivo provável.
const LISTA_ANTIGA: i64 = 30 * TICKS_POR_SEGUNDO;
const TICKS_POR_MS: i64 = TICKS_POR_SEGUNDO / 1000;

/// Explicação comum, no mouse.
pub(super) const ORIGEM_CHEFES: &str = "Hora que o servidor manda na lista dos chefes de campo da região \
     (a cada poucos segundos, com o mapa aberto ou não). Em 2026-10-05 um chefe nasceu 2 min antes da \
     hora: pode aparecer um pouco antes. O nome vem do questlog, pela ordem dos chefes da região \
     (conferida em 3 dos 24 de Altgard).";

/// Um chefe como a tela mostra.
pub(super) struct ChefeVisto {
    /// O do 0x9101 (região × 100 + número), que marca o alerta.
    pub id: u32,
    /// Código do NPC no questlog, só com o nome confiável (a mesma contagem da região): abre os drops.
    pub codigo: Option<u32>,
    pub nome: String,
    pub nivel: i32,
    pub retrato: Option<String>,
    pub vivo: bool,
    /// Vivo só porque a lista é antiga e já passou da hora de renascer.
    pub provavel: bool,
    /// Unix em ms: a hora de renascer (morto) ou a hora marcada em que nasceu (vivo; 0 sem ela).
    pub hora_ms: i64,
}

pub(super) struct ChefesDaRegiao {
    pub regiao: String,
    pub chefes: Vec<ChefeVisto>,
    /// Quando a lista chegou, se já é antiga.
    pub antiga_desde: Option<Hora>,
}

/// Junta a lista do servidor com a região do questlog. Sem a região, ou com outra contagem de NPCs
/// nomeados (região com NPC nomeado que não é chefe de campo), os nomes ficam "Chefe 21".
pub(super) fn chefes_vistos(
    lista: &ChefesDeCampo,
    recebida: Hora,
    regiao: Option<&InfoRegiao>,
    agora: Hora,
) -> ChefesDaRegiao {
    let antiga = agora - recebida > LISTA_ANTIGA;
    let questlog = regiao.filter(|r| r.chefes.len() == lista.chefes.len());
    let agora_ms = agora / TICKS_POR_MS;
    let chefes = lista
        .chefes
        .iter()
        .map(|c| {
            let numero = (c.id / 100 == lista.regiao).then_some(c.id % 100);
            let info = numero.and_then(|n| questlog?.chefes.get((n as usize).checked_sub(1)?));
            let renasceu = !c.vivo && c.hora_ms <= agora_ms;
            ChefeVisto {
                id: c.id,
                codigo: info.map(|i| i.codigo),
                nome: info.map_or_else(|| format!("Chefe {}", numero.unwrap_or(c.id)), |i| i.nome.clone()),
                nivel: info.map_or(0, |i| i.nivel),
                retrato: info.and_then(|i| i.retrato.clone()),
                vivo: c.vivo || (renasceu && antiga),
                provavel: renasceu && antiga,
                hora_ms: c.hora_ms,
            }
        })
        .collect();
    let regiao = regiao.map_or_else(|| format!("região {}", lista.regiao), |r| r.nome.clone());
    ChefesDaRegiao { regiao, chefes, antiga_desde: antiga.then_some(recebida) }
}

/// Os chefes marcados para alerta, com o nome e o retrato da tela Bosses, da última lista de cada
/// região por onde você passou, mais os que derrubam um dos `desejos` (códigos de item) com chance >=
/// `minima` (A13). Roda no fio da bandeja: o questlog só é pedido (fila normal), nunca esperado.
pub(crate) fn chefes_marcados(
    por_regiao: &HashMap<u32, (ChefesDeCampo, Hora)>,
    marcados: &BTreeSet<u32>,
    desejos: &[u32],
    minima: f64,
    agora: Hora,
) -> Vec<ChefeMarcado> {
    let mut saida = Vec::new();
    for (codigo, (lista, recebida)) in por_regiao {
        if desejos.is_empty() && !lista.chefes.iter().any(|c| marcados.contains(&c.id)) {
            continue;
        }
        let regiao = dados_jogo::regiao(*codigo);
        let com_desejo = match &regiao {
            Some(r) if !desejos.is_empty() => {
                desejos_na_regiao(r, desejos.iter().copied(), minima, dados_jogo::item_de_fundo).chefes
            }
            _ => HashMap::new(),
        };
        let vistos = chefes_vistos(lista, *recebida, regiao.as_ref(), agora);
        for (chefe, visto) in lista.chefes.iter().zip(vistos.chefes) {
            let desejos_dele = visto.codigo.and_then(|c| com_desejo.get(&c));
            if marcados.contains(&chefe.id) || desejos_dele.is_some() {
                saida.push(ChefeMarcado {
                    id: chefe.id,
                    nome: visto.nome,
                    retrato: visto.retrato,
                    vivo: chefe.vivo,
                    hora_ms: chefe.hora_ms,
                    lista_em: *recebida,
                    desejos: desejos_dele.map(|l| l.iter().map(|(nome, _)| nome.clone()).collect()).unwrap_or_default(),
                });
            }
        }
    }
    saida
}

/// Segundos até renascer, sem passar de zero. A hora vem do pacote: saturando, um valor absurdo não
/// estoura.
pub(super) fn falta_para_renascer(chefe: &ChefeVisto, agora: Hora) -> i64 {
    chefe.hora_ms.saturating_mul(TICKS_POR_MS).saturating_sub(agora).max(0) / TICKS_POR_SEGUNDO
}

impl Overlay {
    /// A lista do último 0x9101 com os nomes do questlog; None antes de chegar a primeira.
    pub(super) fn chefes_da_regiao(&mut self) -> Option<ChefesDaRegiao> {
        let codigo = self.chefes.as_ref()?.0.regiao;
        if !self.regioes.contains_key(&codigo)
            && let Some(info) = dados_jogo::regiao(codigo)
        {
            self.regioes.insert(codigo, info);
        }
        let (lista, recebida) = self.chefes.as_ref()?;
        Some(chefes_vistos(lista, *recebida, self.regioes.get(&codigo), nucleo::agora()))
    }

    pub(super) fn tela_chefes(&mut self, ui: &mut Ui) {
        let vistos = self.chefes_da_regiao();
        let titulo = vistos.as_ref().map_or_else(|| "Bosses".to_string(), |v| format!("Bosses de {}", v.regiao));
        ui.horizontal(|ui| {
            ui.label(RichText::new(titulo).font(fonte(12.0, true)).color(texto()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "Voltar", true).on_hover_text("Volta ao medidor").clicked() {
                    self.tela = Tela::Medidor;
                }
            });
        });
        ui.add_space(4.0);
        let explicacao = "Os chefes de campo da região em que você está, como o servidor manda a cada poucos \
                          segundos, com o mapa aberto ou não. O nome vem do questlog. Clique num chefe para ver \
                          os drops dele ao lado. ★: derruba um item da sua lista de desejos.";
        ui.add(eframe::egui::Label::new(RichText::new(explicacao).font(fonte(10.0, false)).color(branco(0x88))).wrap());
        let Some(vistos) = vistos else {
            ui.add_space(6.0);
            let vazio = "Aguardando a lista do servidor: ela chega numa região com chefes de campo.";
            ui.label(RichText::new(vazio).font(fonte(12.0, false)).color(texto().gamma_multiply(0.6)));
            return;
        };
        if let Some(desde) = vistos.antiga_desde {
            ui.add_space(4.0);
            let aviso = format!(
                "Lista das {}: fora da região o servidor não manda. Quem passou da hora de renascer está em \
                 Vivos como provável.",
                hora_local(desde)
            );
            ui.add(eframe::egui::Label::new(RichText::new(aviso).font(fonte(10.0, false)).color(visual::AMARELO)).wrap());
        }
        ui.add_space(6.0);

        let agora = nucleo::agora();
        let desejos = self.desejos_da_regiao();
        let do_chefe = |c: &ChefeVisto| desejos.as_ref().and_then(|d| d.chefes.get(&c.codigo?));
        let so_com_desejo = desejos.is_some() && self.config.desejos_so_com_desejo;
        let (mut vivos, mut mortos): (Vec<_>, Vec<_>) =
            vistos.chefes.iter().filter(|c| !so_com_desejo || do_chefe(c).is_some()).partition(|c| c.vivo);
        vivos.sort_by(|a, b| b.nivel.cmp(&a.nivel).then_with(|| a.nome.cmp(&b.nome)));
        mortos.sort_by_key(|c| c.hora_ms);
        ui.horizontal(|ui| {
            if visual::aba(ui, &format!("Vivos ({})", vivos.len()), !self.chefes_mortos).clicked() {
                self.chefes_mortos = false;
            }
            if visual::aba(ui, &format!("Mortos ({})", mortos.len()), self.chefes_mortos).clicked() {
                self.chefes_mortos = true;
            }
            if desejos.is_some() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let dica = if so_com_desejo {
                        "Mostrando só os chefes que derrubam um item da sua lista. Clique para ver todos."
                    } else {
                        "Mostrar só os chefes que derrubam um item da sua lista de desejos (com a chance mínima)."
                    };
                    if visual::aba(ui, "★ Só com desejo", so_com_desejo).on_hover_text(dica).clicked() {
                        self.config.desejos_so_com_desejo = !self.config.desejos_so_com_desejo;
                        self.aplicar_config();
                    }
                });
            }
        });
        if let Some(d) = &desejos {
            let aviso = if d.falharam > 0 {
                Some("O questlog não mandou de onde vêm alguns desejos: sem ★ para eles. Reabra a tela para tentar de novo.")
            } else if d.faltando > 0 {
                self.carregando = true;
                Some("Buscando no questlog de onde vêm os seus desejos...")
            } else {
                None
            };
            if let Some(aviso) = aviso {
                ui.add_space(2.0);
                ui.add(eframe::egui::Label::new(RichText::new(aviso).font(fonte(10.0, false)).color(branco(0x88))).wrap());
            }
        }
        ui.add_space(4.0);
        let lista = if self.chefes_mortos { &mortos } else { &vivos };
        if lista.is_empty() {
            let vazio = match (self.chefes_mortos, so_com_desejo) {
                (true, false) => "Nenhum chefe morto agora.",
                (false, false) => "Nenhum chefe vivo agora.",
                (true, true) => "Nenhum chefe morto com desejo agora.",
                (false, true) => "Nenhum chefe vivo com desejo agora.",
            };
            ui.label(RichText::new(vazio).font(fonte(12.0, false)).color(texto().gamma_multiply(0.6)));
            return;
        }
        for chefe in lista {
            self.linha_chefe(ui, chefe, agora, do_chefe(chefe).map(Vec::as_slice));
        }
    }

    /// Retrato, nome e nível (e a ★ quando derruba um desejo) e, à direita: morto, a hora e a contagem
    /// para renascer; vivo, desde quando.
    fn linha_chefe(&mut self, ui: &mut Ui, chefe: &ChefeVisto, agora: Hora, desejos: Option<&[(String, f64)]>) {
        let largura = ui.available_width();
        let sentido = if chefe.codigo.is_some() { Sense::click() } else { Sense::hover() };
        let (linha, resposta) = ui.allocate_exact_size(vec2(largura, 22.0), sentido);
        let aberto = chefe.codigo.is_some() && self.painel.as_ref().and_then(|p| p.codigo) == chefe.codigo;
        if aberto || (resposta.hovered() && chefe.codigo.is_some()) {
            ui.painter().rect_filled(linha, 3, branco(if aberto { 0x22 } else { 0x14 }));
        }
        let meio = linha.center().y;
        let quadrado = Rect::from_center_size(pos2(linha.min.x + 12.0, meio), Vec2::splat(18.0));
        match &chefe.retrato {
            Some(retrato) => self.imagem_web(ui, retrato, eventos::ROSTO, quadrado),
            None => {
                ui.painter().rect_filled(quadrado, 3, branco(0x22));
            }
        }
        let mut job = LayoutJob::default();
        trecho(&mut job, &chefe.nome, 12.0, false, texto());
        if chefe.nivel > 0 {
            trecho(&mut job, &format!("  {}", chefe.nivel), 10.0, false, branco(0x88));
        }
        if desejos.is_some() {
            trecho(&mut job, "  ★", 11.0, false, visual::DOURADO);
        }
        let nome = montar(ui, job);
        let pintor = ui.painter();
        pintor.galley(pos2(linha.min.x + 28.0, meio - nome.size().y / 2.0), nome, texto());

        let fim = linha.max.x - 6.0;
        let (quem, exata) = (&chefe.nome, hora_com_segundos(chefe.hora_ms));
        let horario = eventos::horario_unix(chefe.hora_ms / 1000, agora);
        let mut direita = LayoutJob::default();
        let dica = if !chefe.vivo {
            let falta = falta_para_renascer(chefe, agora);
            let cor = if falta <= 600 { visual::DOURADO } else { branco(0xCC) };
            trecho(&mut direita, &format!("{horario}   "), 10.0, false, branco(0x77));
            trecho(&mut direita, &eventos::contagem(falta), 11.0, true, cor);
            format!("{quem}: renasce às {exata}.")
        } else if chefe.provavel {
            trecho(&mut direita, &format!("renasceu às {horario} (provável)"), 10.0, false, visual::AMARELO);
            format!("{quem}: a lista antiga dizia que renascia às {exata}.")
        } else if chefe.hora_ms.saturating_mul(TICKS_POR_MS) > agora {
            // Nasceu antes da hora marcada (visto uma vez, 136 s antes): a lista continua com ela.
            trecho(&mut direita, "vivo, antes da hora", 10.0, false, branco(0x99));
            format!("{quem}: nasceu antes da hora marcada ({exata}).")
        } else if chefe.hora_ms > 0 {
            trecho(&mut direita, &format!("desde {horario}"), 10.0, false, branco(0x99));
            format!("{quem}: vivo desde {exata}.")
        } else {
            trecho(&mut direita, "vivo", 10.0, false, branco(0x99));
            format!("{quem}: vivo, sem hora na lista (provavelmente não morreu desde que o servidor reiniciou).")
        };
        let direita = montar(ui, direita);
        let x_direita = fim - direita.size().x;
        pintor.galley(pos2(x_direita, meio - direita.size().y / 2.0), direita, texto());
        let marcado = self.config.alertas.chefes.contains(&chefe.id);
        let sobre_o_sino = if marcado {
            format!(
                "Alerta ligado: {} min antes de renascer e na hora. Clique para desligar.",
                self.config.alertas.chefes_antes_min
            )
        } else {
            "Ligar o alerta deste chefe: avisa antes de ele renascer (a antecedência muda em Configurações › \
             Alertas), mesmo fora da região, pela última lista que o jogo mandou."
                .to_string()
        };
        let sino = visual::sino(ui, pos2(x_direita - 14.0, meio), marcado, ui.id().with(("sino_chefe", chefe.id)));
        if sino.on_hover_text(sobre_o_sino).clicked() {
            if marcado {
                self.config.alertas.chefes.remove(&chefe.id);
            } else if self.config.alertas.chefes.len() < crate::config::CHEFES_MARCADOS_MAX {
                self.config.alertas.chefes.insert(chefe.id);
            }
            self.aplicar_config();
        }
        let clique = if chefe.codigo.is_some() { "\n\nClique para ver os drops ao lado." } else { "" };
        let desejo = desejos.map_or_else(String::new, |lista| {
            let itens: Vec<String> = lista.iter().map(|(nome, chance)| format!("{nome} {}", porcentagem(*chance))).collect();
            format!("\n\n★ Da sua lista de desejos: {}.", itens.join(", "))
        });
        let resposta = resposta.on_hover_text(format!("{dica}{desejo}\n\n{ORIGEM_CHEFES}{clique}"));
        if let Some(codigo) = chefe.codigo
            && resposta.on_hover_cursor(CursorIcon::PointingHand).clicked()
        {
            self.alternar_drops(codigo, ui.ctx().pixels_per_point());
        }
    }
}

/// "22:20:40" no horário de Brasília.
pub(super) fn hora_com_segundos(ms: i64) -> String {
    let s = (ms / 1000 - 3 * 3600).rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

/// Chefe morto como linha da área de eventos: retrato, contagem até renascer e a hora.
pub(super) fn linha_do_chefe(chefe: &ChefeVisto, agora: Hora) -> LinhaEvento {
    LinhaEvento {
        id: None,
        nome: chefe.nome.clone(),
        icone: chefe.retrato.clone().map_or(IconeDaLinha::Moldura, |r| IconeDaLinha::Web(r, eventos::ROSTO)),
        estado: eventos::Estado::Fechado(falta_para_renascer(chefe, agora)),
        horario: eventos::horario_unix(chefe.hora_ms / 1000, agora),
        dica: format!("{}: renasce às {}.\n\n{ORIGEM_CHEFES}", chefe.nome, hora_com_segundos(chefe.hora_ms)),
    }
}

#[cfg(test)]
mod testes {
    use nucleo::medicao::catalogo::ChefeRegiao;
    use nucleo::protocolo::combate::ChefeDeCampo;

    use super::*;

    const S: i64 = TICKS_POR_SEGUNDO;

    fn regiao(nomes: &[&str]) -> InfoRegiao {
        let chefes = nomes
            .iter()
            .enumerate()
            .map(|(i, nome)| ChefeRegiao { codigo: 2_400_000 + i as u32, nome: nome.to_string(), nivel: 45, retrato: None })
            .collect();
        InfoRegiao { nome: "Altgard".into(), chefes }
    }

    fn lista(chefes: &[(u32, bool, i64)]) -> ChefesDeCampo {
        let chefes = chefes.iter().map(|&(id, vivo, hora_ms)| ChefeDeCampo { id, vivo, posicao: None, hora_ms }).collect();
        ChefesDeCampo { regiao: 1110, chefes }
    }

    #[test]
    fn nome_pela_ordem_da_regiao_e_numero_sem_ela() {
        let agora = 1_000_000 * S;
        let l = lista(&[(111_002, true, 0), (111_001, false, 1_000_100_000)]);
        let r = regiao(&["Primeiro", "Segundo"]);
        let v = chefes_vistos(&l, agora, Some(&r), agora);
        assert_eq!((v.chefes[0].nome.as_str(), v.chefes[1].nome.as_str()), ("Segundo", "Primeiro"));
        assert_eq!(v.regiao, "Altgard");
        // Questlog com outra contagem: não dá para confiar na ordem.
        let v = chefes_vistos(&l, agora, Some(&regiao(&["Só um"])), agora);
        assert_eq!((v.chefes[1].nome.as_str(), v.regiao.as_str()), ("Chefe 1", "Altgard"));
        let v = chefes_vistos(&l, agora, None, agora);
        assert_eq!((v.chefes[1].nome.as_str(), v.regiao.as_str()), ("Chefe 1", "região 1110"));
    }

    #[test]
    fn passou_da_hora_so_vira_vivo_provavel_com_a_lista_antiga() {
        let chegou = 1_000_000 * S;
        // Morto, renascia 10 s depois de a lista chegar.
        let l = lista(&[(111_001, false, 1_000_010_000)]);
        // Lista nova (20 s): ainda morto, com 0 s de contagem até o servidor dizer que nasceu.
        let v = chefes_vistos(&l, chegou, None, chegou + 20 * S);
        assert!(!v.chefes[0].vivo && v.antiga_desde.is_none());
        assert_eq!(falta_para_renascer(&v.chefes[0], chegou + 20 * S), 0);
        // Lista antiga (31 s): vivo provável.
        let v = chefes_vistos(&l, chegou, None, chegou + 31 * S);
        assert!(v.chefes[0].vivo && v.chefes[0].provavel);
        assert_eq!(v.antiga_desde, Some(chegou));
        // Antiga, mas ainda antes da hora: continua morto, contando.
        let l = lista(&[(111_001, false, 1_000_100_000)]);
        let v = chefes_vistos(&l, chegou, None, chegou + 31 * S);
        assert!(!v.chefes[0].vivo);
        assert_eq!(falta_para_renascer(&v.chefes[0], chegou + 31 * S), 69);
    }

    #[test]
    fn hora_absurda_no_pacote_nao_estoura() {
        let l = lista(&[(111_001, false, i64::MAX), (111_002, true, i64::MAX)]);
        let v = chefes_vistos(&l, 0, None, 0);
        assert_eq!(falta_para_renascer(&v.chefes[0], 0), i64::MAX / S);
        assert!(v.chefes[1].vivo);
    }

    #[test]
    fn hora_com_segundos_em_brasilia() {
        assert_eq!(hora_com_segundos(1_791_336_040_627), "22:20:40");
    }
}
