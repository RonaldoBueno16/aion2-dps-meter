//! Leitor mínimo de pcapng little-endian (formato gerado pelo pktmon etl2pcap).

use crate::Hora;

pub struct QuadroCapturado {
    pub hora: Hora,
    pub tipo_enlace: u32,
    pub dados: Vec<u8>,
}

const BLOCO_SECAO: u32 = 0x0A0D0D0A;
const BLOCO_INTERFACE: u32 = 1;
const BLOCO_PACOTE_SIMPLES: u32 = 3;
const BLOCO_PACOTE_AVANCADO: u32 = 6;
const MAGICO_LITTLE_ENDIAN: u32 = 0x1A2B3C4D;
const OPCAO_RESOLUCAO_TEMPO: u16 = 9;

pub fn ler(caminho: &std::path::Path) -> Result<Vec<QuadroCapturado>, String> {
    let arquivo = std::fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    let mut quadros = Vec::new();
    // (tipo de enlace, segundos por unidade de tempo)
    let mut interfaces: Vec<(u32, f64)> = Vec::new();
    let mut pos = 0usize;

    while pos + 12 <= arquivo.len() {
        let tipo = le32(&arquivo, pos);
        let total = le32(&arquivo, pos + 4) as usize;
        if total < 12 || pos + total > arquivo.len() {
            return Err(format!("Bloco pcapng inválido na posição {pos}"));
        }
        // O tamanho se repete nos 4 bytes do fim do bloco.
        let corpo = &arquivo[pos + 8..pos + total - 4];
        pos += total;

        match tipo {
            BLOCO_SECAO => {
                if le32(corpo, 0) != MAGICO_LITTLE_ENDIAN {
                    return Err("pcapng big-endian não suportado".into());
                }
                interfaces.clear();
            }
            BLOCO_INTERFACE => {
                let enlace = u32::from(u16::from_le_bytes([corpo[0], corpo[1]]));
                interfaces.push((enlace, ler_resolucao(&corpo[8..])));
            }
            BLOCO_PACOTE_AVANCADO => {
                let iface = le32(corpo, 0) as usize;
                let unidades = u64::from(le32(corpo, 4)) << 32 | u64::from(le32(corpo, 8));
                let capturado = le32(corpo, 12) as usize;
                let (enlace, resolucao) = interfaces.get(iface).copied().unwrap_or((1, 1e-6));
                let hora = (unidades as f64 * resolucao * crate::TICKS_POR_SEGUNDO as f64) as Hora;
                quadros.push(QuadroCapturado { hora, tipo_enlace: enlace, dados: corpo[20..20 + capturado].to_vec() });
            }
            BLOCO_PACOTE_SIMPLES => {
                let enlace = interfaces.first().map_or(1, |i| i.0);
                quadros.push(QuadroCapturado { hora: 0, tipo_enlace: enlace, dados: corpo[4..].to_vec() });
            }
            _ => {}
        }
    }
    Ok(quadros)
}

/// Opção if_tsresol: bit alto 0 = 10^-n segundos, bit alto 1 = 2^-n. Padrão: microssegundo.
fn ler_resolucao(opcoes: &[u8]) -> f64 {
    let mut pos = 0usize;
    while pos + 4 <= opcoes.len() {
        let codigo = u16::from_le_bytes([opcoes[pos], opcoes[pos + 1]]);
        let tamanho = usize::from(u16::from_le_bytes([opcoes[pos + 2], opcoes[pos + 3]]));
        if codigo == 0 {
            break;
        }
        if codigo == OPCAO_RESOLUCAO_TEMPO && tamanho >= 1 && pos + 4 < opcoes.len() {
            let v = opcoes[pos + 4];
            return if v & 0x80 == 0 { 10f64.powf(-f64::from(v)) } else { 2f64.powf(-f64::from(v & 0x7F)) };
        }
        pos += 4 + ((tamanho + 3) & !3);
    }
    1e-6
}

fn le32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
