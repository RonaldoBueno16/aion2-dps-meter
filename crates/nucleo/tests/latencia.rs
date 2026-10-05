mod comum;

use comum::{T0, segundos};
use nucleo::TICKS_POR_SEGUNDO;
use nucleo::captura::latencia::Latencia;

const MS: i64 = TICKS_POR_SEGUNDO / 1000;

#[test]
fn ping_e_o_menor_ida_e_volta_da_janela() {
    let mut l = Latencia::default();
    assert_eq!(l.ping(T0), None);
    // Segmento de 100 bytes, a cópia que a captura entrega 0,035 ms depois, e o ACK 12 ms depois.
    l.enviou(1_000, 100, T0);
    l.enviou(1_000, 100, T0 + segundos(0.000_035));
    l.chegou(1_100, T0 + 12 * MS);
    assert_eq!(l.ping(T0 + 12 * MS), Some(12 * MS));
    // ACK retardado (40 ms) não sobe o ping: vale o menor.
    l.enviou(1_100, 50, T0 + segundos(1.0));
    l.chegou(1_150, T0 + segundos(1.0) + 40 * MS);
    assert_eq!(l.ping(T0 + segundos(1.04)), Some(12 * MS));
    // Um ACK cobre dois segmentos: cada um vira uma amostra do próprio envio.
    l.enviou(1_150, 10, T0 + segundos(2.0));
    l.enviou(1_160, 10, T0 + segundos(2.0) + 5 * MS);
    l.chegou(1_170, T0 + segundos(2.0) + 16 * MS);
    assert_eq!(l.ping(T0 + segundos(2.016)), Some(11 * MS));
    // Mais de 10 s sem amostra: sem ping.
    assert_eq!(l.ping(T0 + segundos(12.1)), None);
}

#[test]
fn retransmissao_nao_vira_amostra() {
    let mut l = Latencia::default();
    // Regra de Karn: reenviado 300 ms depois, o ACK não diz de qual cópia é.
    l.enviou(5_000, 100, T0);
    l.enviou(5_000, 100, T0 + 300 * MS);
    l.chegou(5_100, T0 + 310 * MS);
    assert_eq!(l.ping(T0 + 310 * MS), None);
    // Reenvio do que já foi confirmado: o próximo segmento do servidor não é resposta a ele.
    l.enviou(5_000, 100, T0 + segundos(1.0));
    l.chegou(5_100, T0 + segundos(1.0) + MS);
    assert_eq!(l.ping(T0 + segundos(1.001)), None);
}

#[test]
fn seq_que_da_a_volta() {
    let mut l = Latencia::default();
    l.enviou(u32::MAX - 49, 100, T0);
    l.chegou(50, T0 + 20 * MS);
    assert_eq!(l.ping(T0 + 20 * MS), Some(20 * MS));
}
