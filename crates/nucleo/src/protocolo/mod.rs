pub mod combate;
pub mod desempacotador;
pub mod enquadrador;
pub mod leitor;
pub mod lz4;
pub mod opcodes;
pub mod varint;

/// Posição da primeira ocorrência de `agulha` em `dados`.
pub fn procurar(dados: &[u8], agulha: &[u8]) -> Option<usize> {
    if agulha.is_empty() || dados.len() < agulha.len() {
        return None;
    }
    dados.windows(agulha.len()).position(|janela| janela == agulha)
}

/// O heartbeat do servidor de jogo: tamanho 0x0E e opcode 0x3600.
pub const PADRAO_HEARTBEAT: [u8; 3] = [0x0E, 0x00, 0x36];
