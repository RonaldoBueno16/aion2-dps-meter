//! Fenda Espaço-Temporal do servidor SA: abre a cada 3 h e o portal fica aberto 10 min.

use nucleo::{Hora, TICKS_POR_SEGUNDO};

/// Horário de Brasília (UTC-3, sem horário de verão), o do servidor SA, em segundos.
const FUSO: i64 = -3 * 3600;
/// Aberturas às 02h, 05h, ..., 23h de Brasília. A NCSoft não publicou o horário do SA: esta é a
/// tabela do shugo.gg e do aion2timers.com em 2026-10-05 (o relógio de Taiwan no horário de São
/// Paulo), escolhida pelo usuário.
const PRIMEIRA: i64 = 2 * 3600;
const INTERVALO: i64 = 3 * 3600;
/// Tempo com o portal aberto para entrar; lá dentro o evento dura 1 h.
const PORTAL: i64 = 10 * 60;

#[derive(Debug, PartialEq, Eq)]
pub enum Fenda {
    /// Portal aberto: segundos até fechar.
    Aberta(i64),
    /// Segundos até a próxima abertura.
    Fechada(i64),
}

/// Estado da fenda na hora dada (relógio do PC).
pub fn fenda(hora: Hora) -> Fenda {
    let local = hora.div_euclid(TICKS_POR_SEGUNDO) + FUSO;
    let desde_abertura = (local - PRIMEIRA).rem_euclid(INTERVALO);
    if desde_abertura < PORTAL {
        Fenda::Aberta(PORTAL - desde_abertura)
    } else {
        Fenda::Fechada(INTERVALO - desde_abertura)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Unix em segundos para Hora.
    fn utc(segundos: i64) -> Hora {
        segundos * TICKS_POR_SEGUNDO
    }

    #[test]
    fn abre_as_23h_de_brasilia_e_fica_aberta_10_min() {
        // 2026-10-06 02:00:00 UTC = 05/10 às 23:00 em Brasília.
        assert_eq!(fenda(utc(1_791_252_000)), Fenda::Aberta(600));
        assert_eq!(fenda(utc(1_791_252_599)), Fenda::Aberta(1));
        assert_eq!(fenda(utc(1_791_252_600)), Fenda::Fechada(2 * 3600 + 50 * 60));
    }

    #[test]
    fn conta_ate_a_proxima_depois_da_meia_noite() {
        // 00:00 de Brasília (03:00 UTC): a próxima é às 02h, e às 01:59:59 falta 1 s.
        assert_eq!(fenda(utc(1_791_255_600)), Fenda::Fechada(2 * 3600));
        assert_eq!(fenda(utc(1_791_262_799)), Fenda::Fechada(1));
        assert_eq!(fenda(utc(1_791_262_800)), Fenda::Aberta(600));
    }

    #[test]
    fn fracao_de_segundo_nao_adianta_a_contagem() {
        // 01:59:59,5 ainda mostra 1 s; 02:00:00,5 já mostra o portal aberto inteiro.
        assert_eq!(fenda(utc(1_791_262_799) + TICKS_POR_SEGUNDO / 2), Fenda::Fechada(1));
        assert_eq!(fenda(utc(1_791_262_800) + TICKS_POR_SEGUNDO / 2), Fenda::Aberta(600));
    }
}
