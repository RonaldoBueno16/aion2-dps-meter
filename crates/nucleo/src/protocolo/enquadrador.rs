//! Corta um stream TCP já remontado em pacotes do jogo. Começa dessincronizado e só confia
//! no varint de tamanho depois de achar o heartbeat `0E 00 36`, porque a captura pode
//! começar no meio de um pacote.

use super::{PADRAO_HEARTBEAT, procurar, varint};

const TAMANHO_MAXIMO_PACOTE: i64 = 40 * 1024;

/// tamanho total do pacote = valor do varint + bytes do varint - 4
pub fn tamanho_total(valor_varint: u64, bytes_varint: usize) -> i64 {
    (valor_varint as i64).wrapping_add(bytes_varint as i64 - 4)
}

#[derive(Default)]
pub struct Enquadrador {
    buffer: Vec<u8>,
    pub sincronizado: bool,
    pub dessincronizacoes: u32,
    pub bytes_descartados: u64,
}

impl Enquadrador {
    pub fn adicionar(&mut self, dados: &[u8], ao_extrair: &mut dyn FnMut(&[u8])) {
        self.buffer.extend_from_slice(dados);
        self.processar(ao_extrair);
    }

    /// Conexão nova (SYN): o primeiro byte que vier é o começo de um pacote.
    pub fn iniciar_conexao(&mut self) {
        self.buffer.clear();
        self.sincronizado = true;
    }

    /// Chamar quando o TCP perde dados: o que está no buffer não emenda com o que vem.
    pub fn reiniciar(&mut self) {
        self.bytes_descartados += self.buffer.len() as u64;
        self.buffer.clear();
        self.sincronizado = false;
    }

    fn processar(&mut self, ao_extrair: &mut dyn FnMut(&[u8])) {
        let mut inicio = 0usize;
        while inicio < self.buffer.len() {
            let janela = &self.buffer[inicio..];

            if !self.sincronizado {
                match procurar(janela, &PADRAO_HEARTBEAT) {
                    None => {
                        // Guarda os 2 últimos bytes: o padrão pode estar cortado entre dois segmentos.
                        let descartar = janela.len().saturating_sub(2);
                        inicio += descartar;
                        self.bytes_descartados += descartar as u64;
                        break;
                    }
                    Some(indice) => {
                        inicio += indice;
                        self.bytes_descartados += indice as u64;
                        self.sincronizado = true;
                        continue;
                    }
                }
            }

            let Some((valor, n)) = varint::ler(janela, 0) else {
                if janela.len() >= 10 {
                    self.perder_sincronia(&mut inicio);
                }
                break;
            };

            let tamanho = tamanho_total(valor, n);
            if tamanho <= 0 || tamanho > TAMANHO_MAXIMO_PACOTE {
                self.perder_sincronia(&mut inicio);
                continue;
            }
            let tamanho = tamanho as usize;
            if janela.len() < tamanho {
                break;
            }

            ao_extrair(&janela[..tamanho]);
            inicio += tamanho;
        }

        if inicio > 0 {
            self.buffer.drain(..inicio);
        }
    }

    fn perder_sincronia(&mut self, inicio: &mut usize) {
        self.sincronizado = false;
        self.dessincronizacoes += 1;
        *inicio += 1;
        self.bytes_descartados += 1;
    }
}
