#![allow(dead_code)]

use nucleo::{Hora, TICKS_POR_SEGUNDO};

pub fn hex(texto: &str) -> Vec<u8> {
    (0..texto.len()).step_by(2).map(|i| u8::from_str_radix(&texto[i..i + 2], 16).unwrap()).collect()
}

/// 2026-10-01 22:00:00 UTC.
pub const T0: Hora = 1_790_892_000 * TICKS_POR_SEGUNDO;

pub fn segundos(s: f64) -> i64 {
    (s * TICKS_POR_SEGUNDO as f64) as i64
}

pub fn quase_igual(esperado: f64, obtido: f64) {
    assert!((esperado - obtido).abs() < 1e-6, "esperado {esperado}, obtido {obtido}");
}
