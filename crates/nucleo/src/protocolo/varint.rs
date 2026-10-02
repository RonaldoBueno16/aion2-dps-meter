//! Varint estilo protobuf: 7 bits por byte, bit 0x80 indica que há mais bytes.

/// Lê um varint a partir de `posicao` e devolve (valor, bytes lidos). None quando os bytes
/// acabam antes do fim do varint ou quando ele passa de 64 bits.
pub fn ler(dados: &[u8], posicao: usize) -> Option<(u64, usize)> {
    let mut valor = 0u64;
    let mut tamanho = 0usize;
    let mut deslocamento = 0u32;
    while posicao + tamanho < dados.len() {
        let b = dados[posicao + tamanho];
        tamanho += 1;
        valor |= u64::from(b & 0x7F) << deslocamento;
        if b & 0x80 == 0 {
            return Some((valor, tamanho));
        }
        deslocamento += 7;
        if deslocamento >= 64 {
            return None;
        }
    }
    None
}
