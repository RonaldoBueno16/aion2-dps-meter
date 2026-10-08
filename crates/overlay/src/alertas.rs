//! Alertas de evento (eventos.rs) e de chefe de campo marcado: N minutos antes e na hora. Só a
//! lógica, sem Windows: o fio da bandeja chama `vencidos` a cada segundo com a hora do PC, e a
//! entrega (faixa, som, balão) fica em quem chama. O gatilho é por janela de tempo, nunca por
//! igualdade: um tick atrasado ainda pega o alerta, e o PC que volta da suspensão não solta alerta
//! velho.

use std::collections::{HashMap, HashSet};

use nucleo::{Hora, TICKS_POR_SEGUNDO};

use crate::config::Alertas;
use crate::eventos::{self, EVENTOS, ROSTO};

/// Quanto o "na hora" espera depois do instante: um tick atrasado ou o Axon aberto logo depois.
const JANELA_NA_HORA: i64 = 60;
/// Sem 0x9101 por mais que isso, a lista da região é de antes (mesma regra da tela Bosses).
const LISTA_ANTIGA: i64 = 30 * TICKS_POR_SEGUNDO;
const DIA: i64 = 86_400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origem {
    Evento(&'static str),
    Chefe(u32),
    /// O "Testar alerta".
    Teste,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Momento {
    Antes,
    NaHora,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Alerta {
    pub origem: Origem,
    pub momento: Momento,
    /// Unix em segundos: o início do evento ou o renascer do chefe.
    pub alvo: i64,
    /// O nome do evento ou do chefe.
    pub titulo: String,
    /// "começa em 10 min", "renasceu", "deve ter renascido (lista das 17:22)".
    pub texto: String,
    /// Ícone da CDN do jogo e o recorte, como na lista de eventos.
    pub icone: Option<(String, [f32; 4])>,
}

/// Um chefe de campo marcado como a última lista da região dele mostrou.
#[derive(Clone, Debug, PartialEq)]
pub struct ChefeMarcado {
    pub id: u32,
    pub nome: String,
    pub retrato: Option<String>,
    /// Como a lista diz (sem o "provável" da tela Bosses).
    pub vivo: bool,
    /// Unix em ms: morto, quando renasce.
    pub hora_ms: i64,
    /// Quando a lista chegou.
    pub lista_em: Hora,
}

/// O que já saiu e o último estado visto de cada chefe. Só em memória: reabrir o Axon dentro da
/// janela "antes" solta o alerta de novo, uma vez.
#[derive(Default)]
pub struct Memoria {
    disparados: HashSet<(Origem, i64, Momento)>,
    ultimo_estado: HashMap<u32, (bool, i64)>,
}

impl Memoria {
    /// Primeira vez da chave: true. Desde então, false.
    fn primeira_vez(&mut self, origem: Origem, alvo: i64, momento: Momento) -> bool {
        self.disparados.insert((origem, alvo, momento))
    }
}

/// Os alertas que vencem agora. Com os alertas desligados ou o jogo fechado (e a opção de só com o
/// jogo aberto), a conta roda igual e nada sai: ao ligar de novo, o que já passou não sai atrasado.
pub fn vencidos(
    regras: &Alertas,
    chefes: &[ChefeMarcado],
    agora: Hora,
    jogo_aberto: bool,
    memoria: &mut Memoria,
) -> Vec<Alerta> {
    let agora_s = agora.div_euclid(TICKS_POR_SEGUNDO);
    memoria.disparados.retain(|&(_, alvo, _)| alvo > agora_s - 2 * DIA);
    let mut saida = Vec::new();
    alertas_de_evento(regras, agora, memoria, &mut saida);
    alertas_de_chefe(regras, chefes, agora, memoria, &mut saida);
    if !regras.ligados || (regras.so_com_jogo_aberto && !jogo_aberto) {
        saida.clear();
    }
    saida
}

fn alertas_de_evento(regras: &Alertas, agora: Hora, memoria: &mut Memoria, saida: &mut Vec<Alerta>) {
    let agora_s = agora.div_euclid(TICKS_POR_SEGUNDO);
    for evento in EVENTOS {
        let Some(&minutos) = regras.eventos.get(evento.id) else { continue };
        let (ultimo, proximo) = eventos::inicios(evento, agora);
        let nao_confirmado = if evento.id == "kairah" { " (horário não confirmado)" } else { "" };
        let alerta = |momento, alvo, texto: String| Alerta {
            origem: Origem::Evento(evento.id),
            momento,
            alvo,
            titulo: evento.nome.to_string(),
            texto: format!("{texto}{nao_confirmado}"),
            icone: evento.icone.as_ref().map(|i| (i.nome.to_string(), i.recorte)),
        };
        let antes = i64::from(minutos) * 60;
        if antes > 0
            && proximo - antes <= agora_s
            && memoria.primeira_vez(Origem::Evento(evento.id), proximo, Momento::Antes)
        {
            let verbo = if evento.aberto == 0 { "em" } else { "começa em" };
            saida.push(alerta(Momento::Antes, proximo, format!("{verbo} {}", minutos_ate(proximo - agora_s))));
        }
        if (regras.na_hora || antes == 0)
            && agora_s < ultimo + JANELA_NA_HORA
            && memoria.primeira_vez(Origem::Evento(evento.id), ultimo, Momento::NaHora)
        {
            let texto = if evento.aberto == 0 { "agora" } else { "começou" };
            saida.push(alerta(Momento::NaHora, ultimo, texto.to_string()));
        }
    }
}

fn alertas_de_chefe(
    regras: &Alertas,
    chefes: &[ChefeMarcado],
    agora: Hora,
    memoria: &mut Memoria,
    saida: &mut Vec<Alerta>,
) {
    let agora_s = agora.div_euclid(TICKS_POR_SEGUNDO);
    let antes = i64::from(regras.chefes_antes_min) * 60;
    let na_hora = regras.na_hora || antes == 0;
    for chefe in chefes.iter().filter(|c| regras.chefes.contains(&c.id)) {
        let em_dia = agora - chefe.lista_em <= LISTA_ANTIGA;
        let renasce = chefe.hora_ms.div_euclid(1000);
        let lista_das = || format!(" (lista das {})", eventos::horario_unix(chefe.lista_em.div_euclid(TICKS_POR_SEGUNDO), agora));
        let alerta = |momento, alvo, texto: String| Alerta {
            origem: Origem::Chefe(chefe.id),
            momento,
            alvo,
            titulo: chefe.nome.clone(),
            texto,
            icone: chefe.retrato.clone().map(|r| (r, ROSTO)),
        };

        // Na região: morto → vivo na lista em dia (pode vir antes da hora). Cancela o pré-aviso.
        let anterior = memoria.ultimo_estado.insert(chefe.id, (chefe.vivo, chefe.hora_ms));
        if em_dia
            && chefe.vivo
            && let Some((false, hora_ms)) = anterior
        {
            let alvo = hora_ms.div_euclid(1000);
            memoria.primeira_vez(Origem::Chefe(chefe.id), alvo, Momento::Antes);
            if na_hora && memoria.primeira_vez(Origem::Chefe(chefe.id), alvo, Momento::NaHora) {
                saida.push(alerta(Momento::NaHora, alvo, "renasceu".into()));
            }
            continue;
        }
        if chefe.vivo {
            continue;
        }
        if antes > 0
            && agora_s < renasce
            && renasce.saturating_sub(antes) <= agora_s
            && memoria.primeira_vez(Origem::Chefe(chefe.id), renasce, Momento::Antes)
        {
            let fora = if em_dia { String::new() } else { lista_das() };
            saida.push(alerta(Momento::Antes, renasce, format!("renasce em {}{fora}", minutos_ate(renasce - agora_s))));
        }
        // Fora da região a lista não muda: em R, a conta sobre a hora que o jogo mostrou.
        if !em_dia
            && na_hora
            && renasce <= agora_s
            && agora_s < renasce.saturating_add(JANELA_NA_HORA)
            && memoria.primeira_vez(Origem::Chefe(chefe.id), renasce, Momento::NaHora)
        {
            saida.push(alerta(Momento::NaHora, renasce, format!("deve ter renascido{}", lista_das())));
        }
    }
}

/// "10 min", arredondando para cima: faltando 9:01, ainda "10 min".
fn minutos_ate(segundos: i64) -> String {
    format!("{} min", (segundos.max(1) + 59) / 60)
}

/// Título (até 48 caracteres) e texto (até 200) do balão da bandeja; vários alertas viram um balão.
pub fn texto_do_balao(alertas: &[Alerta]) -> (String, String) {
    let (titulo, texto) = match alertas {
        [um] => (um.titulo.clone(), um.texto.clone()),
        _ => (
            format!("{} alertas", alertas.len()),
            alertas.iter().map(|a| format!("{}: {}", a.titulo, a.texto)).collect::<Vec<_>>().join("; "),
        ),
    };
    (cortar(&titulo, 48), cortar(&texto, 200))
}

/// Corta em caracteres, nunca no meio de um, com "…" no lugar do que sobrou.
fn cortar(texto: &str, maximo: usize) -> String {
    if texto.chars().count() <= maximo {
        return texto.to_string();
    }
    texto.chars().take(maximo - 1).chain(std::iter::once('…')).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    // Instantes conferidos fora do Rust (os mesmos dos testes de eventos.rs).
    /// Domingo, 04/10/2026, 21:00 de Brasília (Nahma abre).
    const DOMINGO_21H: i64 = 1_791_158_400;
    /// Terça, 06/10/2026, 04:00 de Brasília (reset diário, Shugo Festa).
    const TERCA_4H: i64 = 1_791_270_000;
    /// Gartua: renasce às 22:20:40 de terça (06/10) em Brasília.
    const GARTUA_R: i64 = 1_791_336_040;

    fn hora(unix: i64) -> Hora {
        unix * TICKS_POR_SEGUNDO
    }

    fn com_evento(id: &str, minutos: u32) -> Alertas {
        let mut regras = Alertas::default();
        regras.eventos.insert(id.into(), minutos);
        regras
    }

    fn rodar(regras: &Alertas, chefes: &[ChefeMarcado], unix: i64, memoria: &mut Memoria) -> Vec<(Momento, String)> {
        vencidos(regras, chefes, hora(unix), true, memoria).into_iter().map(|a| (a.momento, a.texto)).collect()
    }

    #[test]
    fn evento_avisa_na_janela_uma_vez_e_de_novo_na_proxima_vez() {
        let regras = com_evento("shugo", 10);
        let mut m = Memoria::default();
        assert!(rodar(&regras, &[], TERCA_4H - 601, &mut m).is_empty());
        assert_eq!(rodar(&regras, &[], TERCA_4H - 600, &mut m), [(Momento::Antes, "começa em 10 min".into())]);
        assert!(rodar(&regras, &[], TERCA_4H - 599, &mut m).is_empty());
        assert_eq!(rodar(&regras, &[], TERCA_4H, &mut m), [(Momento::NaHora, "começou".into())]);
        assert!(rodar(&regras, &[], TERCA_4H + 1, &mut m).is_empty());
        // A Shugo seguinte, uma hora depois.
        assert_eq!(rodar(&regras, &[], TERCA_4H + 3000, &mut m), [(Momento::Antes, "começa em 10 min".into())]);
    }

    #[test]
    fn axon_aberto_no_meio_da_janela_avisa_com_o_tempo_que_falta() {
        let regras = com_evento("shugo", 10);
        let mut m = Memoria::default();
        assert_eq!(rodar(&regras, &[], TERCA_4H - 240, &mut m), [(Momento::Antes, "começa em 4 min".into())]);
        // Aberto depois do início e da janela do "na hora": nada daquela vez.
        let mut tarde = Memoria::default();
        assert!(rodar(&regras, &[], TERCA_4H + 61, &mut tarde).is_empty());
        // Tick atrasado 5 s: sai uma vez.
        let mut atrasado = Memoria::default();
        assert_eq!(rodar(&regras, &[], TERCA_4H - 595, &mut atrasado).len(), 1);
        assert!(rodar(&regras, &[], TERCA_4H - 594, &mut atrasado).is_empty());
    }

    #[test]
    fn evento_aberto_nao_vira_pre_aviso_e_antecedencia_zero_avisa_no_inicio() {
        // Nahma aberto às 21:25 com 10 min: o próximo é sexta; nada de "começa em".
        let mut m = Memoria::default();
        assert!(rodar(&com_evento("nahma", 10), &[], DOMINGO_21H + 25 * 60, &mut m).is_empty());
        let so_na_hora = Alertas { na_hora: false, ..com_evento("nahma", 0) };
        let mut m = Memoria::default();
        assert!(rodar(&so_na_hora, &[], DOMINGO_21H - 60, &mut m).is_empty());
        assert_eq!(rodar(&so_na_hora, &[], DOMINGO_21H, &mut m), [(Momento::NaHora, "começou".into())]);
        // Com antecedência e sem o "na hora": só o antes.
        let sem_na_hora = Alertas { na_hora: false, ..com_evento("nahma", 10) };
        let mut m = Memoria::default();
        assert_eq!(rodar(&sem_na_hora, &[], DOMINGO_21H - 600, &mut m).len(), 1);
        assert!(rodar(&sem_na_hora, &[], DOMINGO_21H, &mut m).is_empty());
    }

    #[test]
    fn reset_que_nunca_abre_avisa_na_hora() {
        let regras = com_evento("reset_diario", 10);
        let mut m = Memoria::default();
        assert_eq!(rodar(&regras, &[], TERCA_4H - 600, &mut m), [(Momento::Antes, "em 10 min".into())]);
        assert_eq!(rodar(&regras, &[], TERCA_4H + 59, &mut m), [(Momento::NaHora, "agora".into())]);
        let mut depois = Memoria::default();
        assert!(rodar(&regras, &[], TERCA_4H + 60, &mut depois).is_empty());
    }

    #[test]
    fn pulo_de_relogio_nao_solta_alerta_velho() {
        // PC suspenso às 03:49 e acordado às 05:30: nem o 04:00 nem o 05:00 saem.
        let regras = com_evento("shugo", 10);
        let mut m = Memoria::default();
        assert!(rodar(&regras, &[], TERCA_4H - 660, &mut m).is_empty());
        assert!(rodar(&regras, &[], TERCA_4H + 5400, &mut m).is_empty());
    }

    #[test]
    fn kairah_avisa_com_o_rotulo() {
        let mut m = Memoria::default();
        // Kairah: 01h, 04h, 07h... A das 04h de terça.
        let saida = rodar(&com_evento("kairah", 5), &[], TERCA_4H - 300, &mut m);
        assert_eq!(saida, [(Momento::Antes, "começa em 5 min (horário não confirmado)".into())]);
    }

    fn gartua(vivo: bool, hora_ms: i64, lista_em: i64) -> ChefeMarcado {
        ChefeMarcado { id: 111021, nome: "Gartua".into(), retrato: None, vivo, hora_ms, lista_em: hora(lista_em) }
    }

    fn marcando_gartua() -> Alertas {
        let mut regras = Alertas::default();
        regras.chefes.insert(111021);
        regras
    }

    #[test]
    fn chefe_marcado_avisa_antes_e_renasce_pela_lista_em_dia() {
        let regras = marcando_gartua();
        let mut m = Memoria::default();
        let morto = |agora| gartua(false, GARTUA_R * 1000, agora);
        assert!(rodar(&regras, &[morto(GARTUA_R - 301)], GARTUA_R - 301, &mut m).is_empty());
        assert_eq!(rodar(&regras, &[morto(GARTUA_R - 300)], GARTUA_R - 300, &mut m), [(Momento::Antes, "renasce em 5 min".into())]);
        // Nasceu 2 min antes da hora: "renasceu" na hora em que a lista mudou.
        let vivo = gartua(true, GARTUA_R * 1000 - 136_000, GARTUA_R - 120);
        assert_eq!(rodar(&regras, &[vivo.clone()], GARTUA_R - 120, &mut m), [(Momento::NaHora, "renasceu".into())]);
        assert!(rodar(&regras, &[vivo], GARTUA_R, &mut m).is_empty());
        // Morreu de novo: hora nova, chave nova.
        let r2 = GARTUA_R + 3600;
        assert_eq!(rodar(&regras, &[gartua(false, r2 * 1000, r2 - 300)], r2 - 300, &mut m).len(), 1);
    }

    #[test]
    fn chefe_que_nasce_antes_do_pre_aviso_cancela_o_pre_aviso() {
        let regras = marcando_gartua();
        let mut m = Memoria::default();
        assert!(rodar(&regras, &[gartua(false, GARTUA_R * 1000, GARTUA_R - 900)], GARTUA_R - 900, &mut m).is_empty());
        let vivo = gartua(true, 0, GARTUA_R - 400);
        assert_eq!(rodar(&regras, &[vivo.clone()], GARTUA_R - 400, &mut m), [(Momento::NaHora, "renasceu".into())]);
        assert!(rodar(&regras, &[vivo], GARTUA_R - 300, &mut m).is_empty());
    }

    #[test]
    fn fora_da_regiao_avisa_pela_hora_da_lista_antiga() {
        let regras = marcando_gartua();
        let mut m = Memoria::default();
        // Lista vista às 17:22 (5 h antes), você em outra região.
        let antiga = gartua(false, GARTUA_R * 1000, GARTUA_R - 17_880);
        let antes = rodar(&regras, &[antiga.clone()], GARTUA_R - 300, &mut m);
        assert_eq!(antes, [(Momento::Antes, "renasce em 5 min (lista das 17:22)".into())]);
        let na_hora = rodar(&regras, &[antiga.clone()], GARTUA_R, &mut m);
        assert_eq!(na_hora, [(Momento::NaHora, "deve ter renascido (lista das 17:22)".into())]);
        assert!(rodar(&regras, &[antiga], GARTUA_R + 30, &mut m).is_empty());
    }

    #[test]
    fn primeira_lista_nao_e_transicao_e_hora_absurda_nao_estoura() {
        let regras = marcando_gartua();
        let mut m = Memoria::default();
        // Chegou já vivo: sem estado anterior, nada.
        assert!(rodar(&regras, &[gartua(true, 0, GARTUA_R)], GARTUA_R, &mut m).is_empty());
        assert!(rodar(&regras, &[gartua(false, i64::MAX, GARTUA_R)], GARTUA_R, &mut m).is_empty());
        assert!(rodar(&regras, &[gartua(false, i64::MIN, GARTUA_R - 100)], GARTUA_R, &mut m).is_empty());
        // Chefe não marcado: nada.
        let mut m = Memoria::default();
        let outro = ChefeMarcado { id: 111013, ..gartua(false, GARTUA_R * 1000, GARTUA_R - 300) };
        assert!(rodar(&regras, &[outro], GARTUA_R - 300, &mut m).is_empty());
    }

    #[test]
    fn desligado_ou_jogo_fechado_nao_avisa_nem_depois() {
        let mut regras = com_evento("shugo", 10);
        regras.ligados = false;
        let mut m = Memoria::default();
        assert!(rodar(&regras, &[], TERCA_4H - 600, &mut m).is_empty());
        // Ligado de novo dentro da janela: o instante já passou, nada sai atrasado.
        regras.ligados = true;
        assert!(rodar(&regras, &[], TERCA_4H - 500, &mut m).is_empty());

        let so_com_jogo = Alertas { so_com_jogo_aberto: true, ..com_evento("shugo", 10) };
        let mut m = Memoria::default();
        assert!(vencidos(&so_com_jogo, &[], hora(TERCA_4H - 600), false, &mut m).is_empty());
        let mut m = Memoria::default();
        assert_eq!(vencidos(&so_com_jogo, &[], hora(TERCA_4H - 600), true, &mut m).len(), 1);
    }

    #[test]
    fn dois_alertas_viram_um_balao_cortado_sem_quebrar_caractere() {
        let mut regras = com_evento("shugo", 0);
        regras.eventos.insert("reset_diario".into(), 0);
        let mut m = Memoria::default();
        let juntos = vencidos(&regras, &[], hora(TERCA_4H), true, &mut m);
        assert_eq!(juntos.len(), 2);
        assert_eq!(texto_do_balao(&juntos), ("2 alertas".into(), "Shugo Festa: começou; Reset diário: agora".into()));

        let longo = Alerta { titulo: "Ã".repeat(60), texto: "é".repeat(300), ..juntos[0].clone() };
        let (titulo, texto) = texto_do_balao(&[longo]);
        assert_eq!(titulo.chars().count(), 48);
        assert_eq!(texto.chars().count(), 200);
        assert!(texto.ends_with('…'));
    }
}
