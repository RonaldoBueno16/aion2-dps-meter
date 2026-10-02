//! Núcleo do medidor: protocolo do AION 2 Global (varint, LZ4, framing, parsers), captura
//! (pcapng, raw socket, remontagem TCP) e medição (placar). Formato em PROTOCOLO.md.

pub mod captura;
pub mod formato;
pub mod medicao;
pub mod protocolo;

/// Hora em ticks de 100 ns desde 1970-01-01 UTC (a mesma unidade do DateTime do .NET).
pub type Hora = i64;

pub const TICKS_POR_SEGUNDO: i64 = 10_000_000;

/// Hora atual do relógio do sistema.
pub fn agora() -> Hora {
    let desde = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (desde.as_nanos() / 100) as Hora
}

/// Ticks em segundos, como o TimeSpan.TotalSeconds do .NET.
pub fn segundos(ticks: i64) -> f64 {
    ticks as f64 / TICKS_POR_SEGUNDO as f64
}
