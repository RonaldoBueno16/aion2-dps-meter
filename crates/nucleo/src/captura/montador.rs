//! Remonta uma direção de um stream TCP pelo número de sequência: descarta retransmissões,
//! segura segmentos fora de ordem e, se a lacuna não fechar, avisa que dados se perderam
//! (o enquadrador precisa ressincronizar).

const PENDENTES_MAXIMO: usize = 64;

/// O que o montador entrega: bytes novos, na ordem, ou o aviso de que algo se perdeu.
pub enum Entrega<'a> {
    Dados(&'a [u8]),
    Perda,
}

#[derive(Default)]
pub struct MontadorTcp {
    /// (seq, dados) na ordem de chegada.
    pendentes: Vec<(u32, Vec<u8>)>,
    proximo: u32,
    iniciado: bool,
    pub retransmissoes: u32,
    pub lacunas: u32,
    pub bytes_entregues: u64,
}

impl MontadorTcp {
    pub fn adicionar(&mut self, seq: u32, syn: bool, dados: &[u8], entregar: &mut dyn FnMut(Entrega)) {
        if syn {
            self.proximo = seq.wrapping_add(1);
            self.iniciado = true;
            self.pendentes.clear();
            return;
        }
        if dados.is_empty() {
            return;
        }

        if !self.iniciado {
            self.proximo = seq;
            self.iniciado = true;
        }

        let delta = seq.wrapping_sub(self.proximo) as i32;
        if delta > 0 {
            if !self.pendentes.iter().any(|(s, _)| *s == seq) {
                self.pendentes.push((seq, dados.to_vec()));
            }
            if self.pendentes.len() > PENDENTES_MAXIMO {
                // A lacuna não vai fechar: pula para o segmento pendente mais antigo.
                self.lacunas += 1;
                entregar(Entrega::Perda);
                let proximo = self.proximo;
                self.proximo = self
                    .pendentes
                    .iter()
                    .map(|(s, _)| *s)
                    .min_by_key(|s| s.wrapping_sub(proximo) as i32)
                    .unwrap_or(proximo);
                self.escoar(entregar);
            }
            return;
        }

        self.entregar(dados, delta, entregar);
        self.escoar(entregar);
    }

    /// delta <= 0: o segmento começa em ou antes de `proximo`.
    fn entregar(&mut self, dados: &[u8], delta: i32, entregar: &mut dyn FnMut(Entrega)) {
        let novos = dados.len() as i64 + i64::from(delta);
        if novos <= 0 {
            self.retransmissoes += 1;
            return;
        }
        entregar(Entrega::Dados(&dados[(-delta) as usize..]));
        self.proximo = self.proximo.wrapping_add(novos as u32);
        self.bytes_entregues += novos as u64;
    }

    fn escoar(&mut self, entregar: &mut dyn FnMut(Entrega)) {
        while let Some(i) = self.pendentes.iter().position(|(s, _)| s.wrapping_sub(self.proximo) as i32 <= 0) {
            let (seq, dados) = self.pendentes.remove(i);
            let delta = seq.wrapping_sub(self.proximo) as i32;
            self.entregar(&dados, delta, entregar);
        }
    }
}
