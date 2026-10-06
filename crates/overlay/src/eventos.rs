//! Eventos de horário fixo do servidor SA, contados pelo relógio do PC no horário de Brasília.
//! A NCSoft não publicou os horários do SA: a tabela é a do shugo.gg (servidor South America,
//! America/Sao_Paulo), conferida com metabot.gg e aion2rifttimer.com em 2026-10-06. Os nomes em
//! português são nossos, não os do cliente.

use nucleo::{Hora, TICKS_POR_SEGUNDO};

/// Horário de Brasília (UTC-3, sem horário de verão), o do servidor SA, em segundos.
const FUSO: i64 = -3 * 3600;
const HORA: i64 = 3600;
const DIA: i64 = 24 * HORA;
const SEMANA: i64 = 7 * DIA;
const DIAS: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];

/// Explicação comum a todos, no mouse.
pub const ORIGEM: &str = "Horário de Brasília (servidor SA), pelo relógio do PC. A NCSoft não publicou os \
                          horários do SA: a tabela é a do shugo.gg, conferida com metabot.gg e \
                          aion2rifttimer.com em 2026-10-06.";

pub enum Quando {
    /// A cada `intervalo` segundos, a partir de `inicio` segundos depois da meia-noite.
    Ciclo { inicio: i64, intervalo: i64 },
    /// Nos dias da semana dados (0 = domingo), `inicio` segundos depois da meia-noite.
    Semanal { dias: &'static [i64], inicio: i64 },
}

pub struct Evento {
    pub nome: &'static str,
    /// Na linha recolhida.
    pub curto: &'static str,
    pub quando: Quando,
    /// Quanto tempo fica aberto depois do início; 0 nos resets, que só acontecem.
    pub aberto: i64,
    pub dica: &'static str,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Estado {
    /// Aberto: segundos até fechar.
    Aberto(i64),
    /// Segundos até o próximo início.
    Fechado(i64),
}

/// Na ordem de desempate da lista (mesmo tempo até começar).
pub const EVENTOS: &[Evento] = &[
    Evento {
        nome: "Fenda Espaço-Temporal",
        curto: "Fenda",
        quando: Quando::Ciclo { inicio: 2 * HORA, intervalo: 3 * HORA },
        aberto: 10 * 60,
        dica: "Fenda Espaço-Temporal: a cada 3 h, às 02h, 05h, 08h, 11h, 14h, 17h, 20h e 23h. O portal fica \
               aberto 10 min; lá dentro o evento dura 1 h.",
    },
    Evento {
        nome: "Festival Shugo",
        curto: "Shugo",
        // Começa na hora cheia em qualquer fuso de hora inteira: o FUSO não muda nada aqui.
        quando: Quando::Ciclo { inicio: 0, intervalo: HORA },
        aberto: 3 * 60,
        dica: "Festival Shugo: toda hora cheia. Dá para entrar nos 3 min da contagem, pelo timer ao lado do \
               mapa; o festival inteiro leva uns 10 min. A contagem do jogo é a que vale.",
    },
    Evento {
        nome: "Invasão Dimensional",
        curto: "Invasão",
        quando: Quando::Ciclo { inicio: 30 * 60, intervalo: HORA },
        aberto: 3 * 60,
        dica: "Invasão Dimensional: toda hora, aos 30 min. Dá para entrar nos 3 min de preparação; a rodada \
               dura 10 min.",
    },
    Evento {
        nome: "Vigia Kaira",
        curto: "Kaira",
        quando: Quando::Ciclo { inicio: HORA, intervalo: 3 * HORA },
        aberto: 30 * 60,
        dica: "Vigia Kaira (chefe de campo): a cada 3 h, às 01h, 04h, 07h, 10h, 13h, 16h, 19h e 22h; 30 min. \
               Horário não confirmado: o aion2rifttimer marca assim.",
    },
    Evento {
        nome: "Cerco de Artefato",
        curto: "Cerco",
        quando: Quando::Semanal { dias: &[1, 4, 6], inicio: 21 * HORA },
        aberto: 30 * 60,
        dica: "Cerco de Artefato: segunda, quinta e sábado às 21h; 30 min.",
    },
    Evento {
        nome: "Chefes do Cerco",
        curto: "Chefes do cerco",
        quando: Quando::Semanal { dias: &[1, 4, 6], inicio: 21 * HORA + 30 * 60 },
        aberto: 30 * 60,
        dica: "Chefes do Cerco (os Executores): segunda, quinta e sábado às 21h30; 30 min.",
    },
    Evento {
        nome: "General Guardião Nahma",
        curto: "Nahma",
        quando: Quando::Semanal { dias: &[5, 0], inicio: 21 * HORA },
        aberto: 30 * 60,
        dica: "General Guardião Nahma: sexta e domingo às 21h; 30 min.",
    },
    Evento {
        nome: "Reset diário",
        curto: "Reset",
        quando: Quando::Ciclo { inicio: 4 * HORA, intervalo: DIA },
        aberto: 0,
        dica: "Reset diário: todo dia às 04h (07h UTC).",
    },
    Evento {
        nome: "Reset semanal",
        curto: "Reset semanal",
        quando: Quando::Semanal { dias: &[3], inicio: 4 * HORA },
        aberto: 0,
        dica: "Reset semanal: quarta às 04h (07h UTC).",
    },
];

/// Segundos desde 01/01/1970 no horário de Brasília.
fn local(hora: Hora) -> i64 {
    hora.div_euclid(TICKS_POR_SEGUNDO) + FUSO
}

/// Estado do evento na hora dada (relógio do PC).
pub fn estado(evento: &Evento, hora: Hora) -> Estado {
    let local = local(hora);
    let desde = |inicio: i64, periodo: i64| (local - inicio).rem_euclid(periodo);
    // Tempo desde o último início e até o próximo.
    let (desde, ate) = match evento.quando {
        Quando::Ciclo { inicio, intervalo } => {
            let d = desde(inicio, intervalo);
            (d, intervalo - d)
        }
        // 01/01/1970 foi quinta-feira: o dia 0 da contagem cai 4 dias depois de um domingo.
        Quando::Semanal { dias, inicio } => dias
            .iter()
            .map(|dia| desde((dia - 4).rem_euclid(7) * DIA + inicio, SEMANA))
            .fold((SEMANA, SEMANA), |(d, a), cada| (d.min(cada), a.min(SEMANA - cada))),
    };
    if desde < evento.aberto { Estado::Aberto(evento.aberto - desde) } else { Estado::Fechado(ate) }
}

/// Início da vez aberta ou da próxima: "21:00" se for hoje, "qui 21:00" se não.
pub fn horario(evento: &Evento, hora: Hora) -> String {
    let local = local(hora);
    let inicio = match estado(evento, hora) {
        Estado::Aberto(falta) => local - (evento.aberto - falta),
        Estado::Fechado(falta) => local + falta,
    };
    let hh_mm = format!("{:02}:{:02}", inicio.rem_euclid(DIA) / HORA, inicio.rem_euclid(HORA) / 60);
    let dia = inicio.div_euclid(DIA);
    if dia == local.div_euclid(DIA) { hh_mm } else { format!("{} {hh_mm}", DIAS[(dia + 4).rem_euclid(7) as usize]) }
}

/// Os abertos primeiro (o que fecha antes no topo), depois o que começa antes.
pub fn em_ordem(hora: Hora) -> Vec<(&'static Evento, Estado)> {
    let mut lista: Vec<_> = EVENTOS.iter().map(|e| (e, estado(e, hora))).collect();
    lista.sort_by_key(|(_, estado)| match *estado {
        Estado::Aberto(s) => (0, s),
        Estado::Fechado(s) => (1, s),
    });
    lista
}

/// "mm:ss" abaixo de 1 h, "h:mm:ss" abaixo de 1 dia, "2d 05h" a partir daí.
pub fn contagem(segundos: i64) -> String {
    if segundos < HORA {
        format!("{:02}:{:02}", segundos / 60, segundos % 60)
    } else if segundos < DIA {
        format!("{}:{:02}:{:02}", segundos / HORA, segundos % HORA / 60, segundos % 60)
    } else {
        format!("{}d {:02}h", segundos / DIA, segundos % DIA / HORA)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Unix em segundos para Hora.
    fn utc(segundos: i64) -> Hora {
        segundos * TICKS_POR_SEGUNDO
    }

    fn evento(curto: &str) -> &'static Evento {
        EVENTOS.iter().find(|e| e.curto == curto).unwrap()
    }

    // Instantes conferidos fora do Rust (Python, datetime com UTC-3).
    /// Segunda, 05/10/2026, 21:00 de Brasília.
    const SEGUNDA_21H: i64 = 1_791_244_800;
    /// Domingo, 04/10/2026, 21:00 de Brasília.
    const DOMINGO_21H: i64 = 1_791_158_400;
    /// Terça, 06/10/2026, 04:00 de Brasília.
    const TERCA_4H: i64 = 1_791_270_000;
    /// Quarta, 07/10/2026, 04:00 de Brasília.
    const QUARTA_4H: i64 = 1_791_356_400;
    /// Quinta, 08/10/2026, 21:30 de Brasília.
    const QUINTA_21H30: i64 = 1_791_505_800;
    /// Sábado, 10/10/2026, 21:00 de Brasília.
    const SABADO_21H: i64 = 1_791_676_800;

    #[test]
    fn fenda_abre_as_23h_de_brasilia_e_fica_aberta_10_min() {
        // 2026-10-06 02:00:00 UTC = 05/10 às 23:00 em Brasília.
        let fenda = evento("Fenda");
        assert_eq!(estado(fenda, utc(1_791_252_000)), Estado::Aberto(600));
        assert_eq!(estado(fenda, utc(1_791_252_599)), Estado::Aberto(1));
        assert_eq!(estado(fenda, utc(1_791_252_600)), Estado::Fechado(2 * 3600 + 50 * 60));
    }

    #[test]
    fn fenda_conta_ate_a_proxima_depois_da_meia_noite() {
        // 00:00 de Brasília (03:00 UTC): a próxima é às 02h, e às 01:59:59 falta 1 s.
        let fenda = evento("Fenda");
        assert_eq!(estado(fenda, utc(1_791_255_600)), Estado::Fechado(2 * 3600));
        assert_eq!(estado(fenda, utc(1_791_262_799)), Estado::Fechado(1));
        assert_eq!(estado(fenda, utc(1_791_262_800)), Estado::Aberto(600));
    }

    #[test]
    fn fracao_de_segundo_nao_adianta_a_contagem() {
        // 01:59:59,5 ainda mostra 1 s; 02:00:00,5 já mostra o portal aberto inteiro.
        let fenda = evento("Fenda");
        assert_eq!(estado(fenda, utc(1_791_262_799) + TICKS_POR_SEGUNDO / 2), Estado::Fechado(1));
        assert_eq!(estado(fenda, utc(1_791_262_800) + TICKS_POR_SEGUNDO / 2), Estado::Aberto(600));
    }

    #[test]
    fn shugo_abre_na_hora_cheia_por_3_min_e_invasao_aos_30() {
        let (shugo, invasao) = (evento("Shugo"), evento("Invasão"));
        assert_eq!(estado(shugo, utc(TERCA_4H - 1)), Estado::Fechado(1));
        assert_eq!(estado(shugo, utc(TERCA_4H)), Estado::Aberto(180));
        assert_eq!(estado(shugo, utc(TERCA_4H + 179)), Estado::Aberto(1));
        assert_eq!(estado(shugo, utc(TERCA_4H + 180)), Estado::Fechado(57 * 60));
        assert_eq!(estado(invasao, utc(TERCA_4H)), Estado::Fechado(30 * 60));
        assert_eq!(estado(invasao, utc(TERCA_4H + 30 * 60)), Estado::Aberto(180));
    }

    #[test]
    fn cerco_so_na_segunda_quinta_e_sabado_as_21h() {
        let cerco = evento("Cerco");
        assert_eq!(estado(cerco, utc(SEGUNDA_21H - 1)), Estado::Fechado(1));
        assert_eq!(estado(cerco, utc(SEGUNDA_21H)), Estado::Aberto(1800));
        assert_eq!(estado(cerco, utc(SEGUNDA_21H + 1799)), Estado::Aberto(1));
        // Fechou segunda às 21:30: a próxima é quinta às 21:00.
        assert_eq!(estado(cerco, utc(SEGUNDA_21H + 1800)), Estado::Fechado(3 * 86400 - 1800));
        assert_eq!(estado(cerco, utc(SABADO_21H)), Estado::Aberto(1800));
        // De sábado para segunda: 2 dias.
        assert_eq!(estado(cerco, utc(SABADO_21H + 1800)), Estado::Fechado(2 * 86400 - 1800));
        // Domingo às 21h não tem cerco: falta 1 dia.
        assert_eq!(estado(cerco, utc(DOMINGO_21H)), Estado::Fechado(86400));
        assert_eq!(estado(evento("Chefes do cerco"), utc(QUINTA_21H30)), Estado::Aberto(1800));
        assert_eq!(estado(evento("Chefes do cerco"), utc(SEGUNDA_21H)), Estado::Fechado(1800));
    }

    #[test]
    fn nahma_so_na_sexta_e_no_domingo() {
        let nahma = evento("Nahma");
        assert_eq!(estado(nahma, utc(DOMINGO_21H)), Estado::Aberto(1800));
        assert_eq!(estado(nahma, utc(DOMINGO_21H - 1)), Estado::Fechado(1));
        // Segunda às 21h: a próxima é sexta, 4 dias depois.
        assert_eq!(estado(nahma, utc(SEGUNDA_21H)), Estado::Fechado(4 * 86400));
    }

    #[test]
    fn resets_as_4h_e_o_semanal_so_na_quarta() {
        let (diario, semanal) = (evento("Reset"), evento("Reset semanal"));
        assert_eq!(estado(diario, utc(TERCA_4H - 1)), Estado::Fechado(1));
        // Acabou de acontecer: o próximo é amanhã.
        assert_eq!(estado(diario, utc(TERCA_4H)), Estado::Fechado(86400));
        assert_eq!(estado(semanal, utc(TERCA_4H)), Estado::Fechado(86400));
        assert_eq!(estado(semanal, utc(QUARTA_4H - 1)), Estado::Fechado(1));
        assert_eq!(estado(semanal, utc(QUARTA_4H)), Estado::Fechado(7 * 86400));
    }

    #[test]
    fn horario_mostra_o_dia_so_quando_nao_e_hoje() {
        assert_eq!(horario(evento("Cerco"), utc(DOMINGO_21H)), "seg 21:00");
        assert_eq!(horario(evento("Nahma"), utc(DOMINGO_21H)), "21:00");
        // Aberto: o início da vez que está aberta.
        assert_eq!(horario(evento("Shugo"), utc(TERCA_4H + 90)), "04:00");
        assert_eq!(horario(evento("Reset semanal"), utc(TERCA_4H)), "qua 04:00");
    }

    #[test]
    fn ordem_poe_os_abertos_primeiro_e_depois_o_que_comeca_antes() {
        let nomes: Vec<_> = em_ordem(utc(SEGUNDA_21H)).iter().map(|(e, _)| e.curto).collect();
        assert_eq!(
            nomes,
            ["Shugo", "Cerco", "Invasão", "Chefes do cerco", "Kaira", "Fenda", "Reset", "Reset semanal", "Nahma"]
        );
    }

    #[test]
    fn contagem_cresce_de_minutos_para_horas_e_dias() {
        assert_eq!(contagem(59), "00:59");
        assert_eq!(contagem(3599), "59:59");
        assert_eq!(contagem(3600), "1:00:00");
        assert_eq!(contagem(86399), "23:59:59");
        assert_eq!(contagem(86400), "1d 00h");
        assert_eq!(contagem(2 * 86400 - 1800), "1d 23h");
    }
}
