//! Ping até o servidor do jogo pelo tempo de ida e volta do TCP: da hora em que o PC manda um
//! segmento com dados até a chegada do primeiro segmento do servidor cujo ACK cobre o fim dele. O
//! servidor às vezes segura o ACK (ACK retardado), então o ping é o menor tempo da janela, o da rede:
//! nas capturas de 2026-10-02 e 03, mínimo de 10 a 14 ms em toda janela de 10 s, mediana ~20 ms.

use std::collections::VecDeque;

use crate::{Hora, TICKS_POR_SEGUNDO};

const JANELA: i64 = 10 * TICKS_POR_SEGUNDO;
/// A captura entrega o mesmo segmento de saída duas vezes, 0,035 ms depois (3.519 de 3.604
/// repetições nas capturas); só repetição mais tarde que isso é retransmissão.
const COPIA_DA_CAPTURA: i64 = TICKS_POR_SEGUNDO / 1000;
/// Segmentos do PC esperando ACK; passou disso (servidor que não responde), os mais velhos saem.
const PENDENTES_MAX: usize = 256;

struct Pendente {
    /// seq + tamanho: o ACK que confirma o segmento inteiro.
    fim: u32,
    enviado: Hora,
    retransmitido: bool,
}

#[derive(Default)]
pub struct Latencia {
    pendentes: VecDeque<Pendente>,
    amostras: VecDeque<(Hora, i64)>,
    maior_ack: Option<u32>,
}

/// `a` vem antes de `b` no espaço de 32 bits do seq (que dá a volta).
fn antes(a: u32, b: u32) -> bool {
    a != b && b.wrapping_sub(a) < 1 << 31
}

impl Latencia {
    /// Segmento com dados do PC para o servidor.
    pub fn enviou(&mut self, seq: u32, tamanho: usize, hora: Hora) {
        let fim = seq.wrapping_add(tamanho as u32);
        // Já confirmado: retransmissão à toa, cujo "ACK" seria o próximo segmento qualquer.
        if self.maior_ack.is_some_and(|ack| !antes(ack, fim)) {
            return;
        }
        if let Some(p) = self.pendentes.iter_mut().find(|p| p.fim == fim) {
            // Regra de Karn: o ACK de um segmento retransmitido não diz qual das cópias chegou.
            if hora - p.enviado > COPIA_DA_CAPTURA {
                p.retransmitido = true;
            }
            return;
        }
        if self.pendentes.len() == PENDENTES_MAX {
            self.pendentes.pop_front();
        }
        self.pendentes.push_back(Pendente { fim, enviado: hora, retransmitido: false });
    }

    /// Segmento do servidor com o flag ACK.
    pub fn chegou(&mut self, ack: u32, hora: Hora) {
        if self.maior_ack.is_none_or(|maior| antes(maior, ack)) {
            self.maior_ack = Some(ack);
        }
        while let Some(p) = self.pendentes.front() {
            if antes(ack, p.fim) {
                break;
            }
            if !p.retransmitido {
                self.amostras.push_back((hora, hora - p.enviado));
            }
            self.pendentes.pop_front();
        }
        while self.amostras.front().is_some_and(|&(quando, _)| hora - quando > JANELA) {
            self.amostras.pop_front();
        }
    }

    /// Menor ida e volta dos últimos 10 s, em ticks; None sem amostra nesse tempo.
    pub fn ping(&self, agora: Hora) -> Option<i64> {
        self.amostras.iter().filter(|&&(quando, _)| agora - quando <= JANELA).map(|&(_, rtt)| rtt).min()
    }
}
