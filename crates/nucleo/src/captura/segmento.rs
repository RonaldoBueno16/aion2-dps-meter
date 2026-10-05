use std::net::Ipv4Addr;

#[derive(Clone, Debug)]
pub struct SegmentoTcp {
    pub origem: Ipv4Addr,
    pub porta_origem: u16,
    pub destino: Ipv4Addr,
    pub porta_destino: u16,
    pub seq: u32,
    /// O número de ACK, quando o flag ACK vem ligado.
    pub ack: Option<u32>,
    pub syn: bool,
    pub dados: Vec<u8>,
    /// "ip:porta > ip:porta", a identidade de uma direção do fluxo.
    pub chave: String,
}

pub const ENLACE_ETHERNET: u32 = 1;
pub const ENLACE_IP_CRU: u32 = 101;
pub const ENLACE_IPV4: u32 = 228;

impl SegmentoTcp {
    pub fn novo(
        origem: Ipv4Addr,
        porta_origem: u16,
        destino: Ipv4Addr,
        porta_destino: u16,
        seq: u32,
        syn: bool,
        dados: Vec<u8>,
    ) -> Self {
        let chave = format!("{origem}:{porta_origem} > {destino}:{porta_destino}");
        Self { origem, porta_origem, destino, porta_destino, seq, ack: None, syn, dados, chave }
    }

    /// Extrai um segmento TCP sobre IPv4 de um quadro capturado.
    pub fn extrair(quadro: &[u8], tipo_enlace: u32) -> Option<Self> {
        let ip = match tipo_enlace {
            ENLACE_ETHERNET => {
                if quadro.len() < 14 {
                    return None;
                }
                let mut ether_type = be16(quadro, 12);
                let mut ip = 14;
                if ether_type == 0x8100 && quadro.len() >= 18 {
                    ether_type = be16(quadro, 16);
                    ip = 18;
                }
                if ether_type != 0x0800 {
                    return None;
                }
                ip
            }
            ENLACE_IP_CRU | ENLACE_IPV4 => 0,
            _ => return None,
        };

        if quadro.len() < ip + 20 || quadro[ip] >> 4 != 4 || quadro[ip + 9] != 6 {
            return None;
        }
        let cabecalho_ip = usize::from(quadro[ip] & 0x0F) * 4;
        let mut fim_ip = ip + usize::from(be16(quadro, ip + 2));
        // Placa com coalescência de recepção pode gravar tamanho 0 ou menor que o real.
        if fim_ip <= ip + cabecalho_ip || fim_ip > quadro.len() {
            fim_ip = quadro.len();
        }

        let tcp = ip + cabecalho_ip;
        if fim_ip < tcp + 20 {
            return None;
        }
        let cabecalho_tcp = usize::from(quadro[tcp + 12] >> 4) * 4;
        let inicio_dados = tcp + cabecalho_tcp;
        if inicio_dados > fim_ip {
            return None;
        }

        let ipv4 = |i: usize| Ipv4Addr::new(quadro[i], quadro[i + 1], quadro[i + 2], quadro[i + 3]);
        let be32 = |i: usize| u32::from_be_bytes([quadro[i], quadro[i + 1], quadro[i + 2], quadro[i + 3]]);
        let mut segmento = Self::novo(
            ipv4(ip + 12),
            be16(quadro, tcp),
            ipv4(ip + 16),
            be16(quadro, tcp + 2),
            be32(tcp + 4),
            quadro[tcp + 13] & 0x02 != 0,
            quadro[inicio_dados..fim_ip].to_vec(),
        );
        segmento.ack = (quadro[tcp + 13] & 0x10 != 0).then(|| be32(tcp + 8));
        Some(segmento)
    }
}

fn be16(q: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([q[i], q[i + 1]])
}
