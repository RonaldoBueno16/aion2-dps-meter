//! Abre os pacotes comprimidos e entrega os pacotes de dentro, na ordem.
//! Formato: [varint][flag 0xF? opcional][FF FF][u32 tamanho descomprimido][bloco LZ4].
//! O conteúdo descomprimido é uma sequência de pacotes no mesmo framing, que pode ter
//! outro bloco comprimido dentro.

use super::{enquadrador::tamanho_total, lz4, varint};

const PROFUNDIDADE_MAXIMA: u32 = 8;
const TAMANHO_DESCOMPRIMIDO_MAXIMO: i32 = 10_000_000;

#[derive(Default)]
pub struct Desempacotador {
    pub blocos_comprimidos: u32,
    pub falhas_descompressao: u32,
    pub tamanhos_divergentes: u32,
    pub bytes_sobrando: u64,
}

enum Resultado {
    NaoComprimido,
    Comprimido(Vec<u8>),
    Falhou,
}

impl Desempacotador {
    pub fn expandir(&mut self, pacote: &[u8], ao_pacote: &mut dyn FnMut(&[u8])) {
        self.abrir(pacote, ao_pacote, 0);
    }

    fn abrir(&mut self, buffer: &[u8], ao_pacote: &mut dyn FnMut(&[u8]), profundidade: u32) {
        let mut pos = 0usize;
        while pos < buffer.len() {
            // Bytes 00 entre quadros são preenchimento.
            if buffer[pos] == 0 {
                pos += 1;
                continue;
            }

            let Some((valor, n)) = varint::ler(buffer, pos) else { break };
            if valor > 2_000_000 {
                break;
            }
            let tamanho = tamanho_total(valor, n);
            if tamanho <= 0 {
                pos += 1;
                continue;
            }
            let tamanho = tamanho as usize;
            if pos + tamanho > buffer.len() {
                break;
            }

            let quadro = &buffer[pos..pos + tamanho];
            match self.tentar_descomprimir(quadro, n) {
                Resultado::Comprimido(interno) => {
                    self.blocos_comprimidos += 1;
                    if profundidade < PROFUNDIDADE_MAXIMA {
                        self.abrir(&interno, ao_pacote, profundidade + 1);
                    }
                }
                Resultado::Falhou => self.falhas_descompressao += 1,
                Resultado::NaoComprimido => ao_pacote(quadro),
            }
            pos += tamanho;
        }
        self.bytes_sobrando += (buffer.len() - pos) as u64;
    }

    fn tentar_descomprimir(&mut self, quadro: &[u8], bytes_varint: usize) -> Resultado {
        let mut cabecalho = bytes_varint;
        if cabecalho < quadro.len() && quadro[cabecalho] & 0xF0 == 0xF0 && quadro[cabecalho] != 0xFF {
            cabecalho += 1;
        }

        if quadro.len() < cabecalho + 6 || quadro[cabecalho] != 0xFF || quadro[cabecalho + 1] != 0xFF {
            return Resultado::NaoComprimido;
        }

        let t = &quadro[cabecalho + 2..cabecalho + 6];
        let tamanho_real = i32::from_le_bytes([t[0], t[1], t[2], t[3]]);
        if tamanho_real <= 0 || tamanho_real > TAMANHO_DESCOMPRIMIDO_MAXIMO {
            return Resultado::Falhou;
        }

        let mut saida = vec![0u8; tamanho_real as usize];
        let escritos = match lz4::descomprimir(&quadro[cabecalho + 6..], &mut saida) {
            Some(e) if e > 0 => e,
            _ => return Resultado::Falhou,
        };
        if escritos != saida.len() {
            self.tamanhos_divergentes += 1;
            saida.truncate(escritos);
        }
        Resultado::Comprimido(saida)
    }
}
