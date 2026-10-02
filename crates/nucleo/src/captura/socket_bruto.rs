//! Captura passiva com raw socket do Windows (SIO_RCVALL), sem driver extra. Exige o processo
//! elevado. Abre um socket por IPv4 local e entrega cada segmento TCP no callback, que roda nas
//! threads de captura (uma por socket): quem recebe serializa (o pipeline não é thread-safe).
//! Não abre handle nem lê nada do processo do jogo.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    IP_ADAPTER_ADDRESSES_LH,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, INVALID_SOCKET, IN_ADDR, IN_ADDR_0, IP_HDRINCL, IPPROTO_IP, RCVALL_ON, SIO_RCVALL, SO_RCVBUF, SOCK_RAW,
    SOCKADDR, SOCKADDR_IN, SOCKET, SOL_SOCKET, WSADATA, WSAEACCES, WSAGetLastError, WSAIoctl, WSAStartup, bind,
    closesocket, recv, setsockopt, socket,
};

use super::segmento::{ENLACE_IPV4, SegmentoTcp};
use crate::Hora;

const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const ERRO_BUFFER_PEQUENO: u32 = 111;

pub type AoSegmento = Arc<dyn Fn(SegmentoTcp, Hora) + Send + Sync>;

#[derive(Default)]
pub struct Contadores {
    pub recebidos: AtomicU64,
    pub segmentos_tcp: AtomicU64,
    /// Segmentos TCP que chegam a este PC e que saem dele. Saída sem entrada nenhuma = algo (o
    /// firewall) descarta o que chega antes de o socket ver.
    pub tcp_entrada: AtomicU64,
    pub tcp_saida: AtomicU64,
}

pub struct CapturaSocketBruto {
    sockets: Vec<SOCKET>,
    parando: Arc<AtomicBool>,
    pub enderecos: Vec<Ipv4Addr>,
    pub contadores: Arc<Contadores>,
}

impl CapturaSocketBruto {
    /// Abre os sockets e começa a capturar. Err com a mensagem para o usuário.
    pub fn iniciar(ao_segmento: AoSegmento) -> Result<Self, String> {
        let mut dados = unsafe { std::mem::zeroed::<WSADATA>() };
        let erro = unsafe { WSAStartup(0x0202, &mut dados) };
        if erro != 0 {
            return Err(format!("WSAStartup falhou ({erro})."));
        }

        let enderecos = enderecos_locais()?;
        if enderecos.is_empty() {
            return Err("Nenhuma interface IPv4 ativa.".into());
        }

        let mut captura = Self {
            sockets: Vec::new(),
            parando: Arc::new(AtomicBool::new(false)),
            enderecos: enderecos.clone(),
            contadores: Arc::new(Contadores::default()),
        };
        for endereco in enderecos {
            let s = abrir(endereco)?;
            captura.sockets.push(s);
            let (parando, contadores, ao_segmento) =
                (captura.parando.clone(), captura.contadores.clone(), ao_segmento.clone());
            std::thread::Builder::new()
                .name(format!("captura {endereco}"))
                .spawn(move || receber(s, endereco, &parando, &contadores, &*ao_segmento))
                .map_err(|e| e.to_string())?;
        }
        Ok(captura)
    }
}

impl Drop for CapturaSocketBruto {
    fn drop(&mut self) {
        self.parando.store(true, Ordering::SeqCst);
        for s in self.sockets.drain(..) {
            unsafe { closesocket(s) };
        }
    }
}

fn abrir(endereco: Ipv4Addr) -> Result<SOCKET, String> {
    unsafe {
        let s = socket(i32::from(AF_INET), SOCK_RAW, IPPROTO_IP);
        if s == INVALID_SOCKET {
            // Sem elevação o Windows recusa já na criação do raw socket.
            let erro = WSAGetLastError();
            return Err(if erro == WSAEACCES {
                "Captura por raw socket precisa rodar como administrador.".to_string()
            } else {
                format!("socket falhou ({erro}).")
            });
        }
        let falhou = |etapa: &str| {
            let erro = WSAGetLastError();
            closesocket(s);
            if erro == WSAEACCES {
                "Captura por raw socket precisa rodar como administrador.".to_string()
            } else {
                format!("{etapa} em {endereco} falhou ({erro}).")
            }
        };

        let local = SOCKADDR_IN {
            sin_family: AF_INET,
            sin_port: 0,
            sin_addr: IN_ADDR { S_un: IN_ADDR_0 { S_addr: u32::from_ne_bytes(endereco.octets()) } },
            sin_zero: [0; 8],
        };
        if bind(s, (&raw const local).cast::<SOCKADDR>(), size_of::<SOCKADDR_IN>() as i32) != 0 {
            return Err(falhou("bind"));
        }
        let um: i32 = 1;
        if setsockopt(s, IPPROTO_IP, IP_HDRINCL, (&raw const um).cast(), 4) != 0 {
            return Err(falhou("IP_HDRINCL"));
        }
        let buffer: i32 = 8 * 1024 * 1024;
        setsockopt(s, SOL_SOCKET, SO_RCVBUF, (&raw const buffer).cast(), 4);

        let ligar: u32 = RCVALL_ON as u32;
        let mut saida = [0u8; 4];
        let mut devolvidos = 0u32;
        if WSAIoctl(
            s,
            SIO_RCVALL,
            (&raw const ligar).cast(),
            4,
            saida.as_mut_ptr().cast(),
            4,
            &mut devolvidos,
            std::ptr::null_mut(),
            None,
        ) != 0
        {
            return Err(falhou("SIO_RCVALL"));
        }
        Ok(s)
    }
}

fn receber(
    s: SOCKET,
    endereco: Ipv4Addr,
    parando: &AtomicBool,
    contadores: &Contadores,
    ao_segmento: &(dyn Fn(SegmentoTcp, Hora) + Send + Sync),
) {
    let mut buffer = vec![0u8; 65536];
    while !parando.load(Ordering::SeqCst) {
        let n = unsafe { recv(s, buffer.as_mut_ptr(), buffer.len() as i32, 0) };
        // Socket fechado (parando) ou interface que caiu: a thread deste endereço termina.
        if n <= 0 {
            return;
        }
        contadores.recebidos.fetch_add(1, Ordering::Relaxed);
        let Some(seg) = SegmentoTcp::extrair(&buffer[..n as usize], ENLACE_IPV4) else { continue };
        contadores.segmentos_tcp.fetch_add(1, Ordering::Relaxed);
        if seg.destino == endereco {
            contadores.tcp_entrada.fetch_add(1, Ordering::Relaxed);
        } else if seg.origem == endereco {
            contadores.tcp_saida.fetch_add(1, Ordering::Relaxed);
        }
        ao_segmento(seg, crate::agora());
    }
}

/// IPv4 das interfaces ligadas, sem a de loopback.
fn enderecos_locais() -> Result<Vec<Ipv4Addr>, String> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut tamanho: u32 = 16 * 1024;
    let mut buffer: Vec<u64> = Vec::new();
    for _ in 0..3 {
        // u64 para o alinhamento das estruturas.
        buffer = vec![0u64; (tamanho as usize).div_ceil(8)];
        let erro = unsafe {
            GetAdaptersAddresses(u32::from(AF_INET), flags, std::ptr::null(), buffer.as_mut_ptr().cast(), &mut tamanho)
        };
        match erro {
            0 => break,
            ERRO_BUFFER_PEQUENO => continue,
            _ => return Err(format!("GetAdaptersAddresses falhou ({erro}).")),
        }
    }

    let mut enderecos = Vec::new();
    let mut adaptador = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    unsafe {
        while !adaptador.is_null() {
            let a = &*adaptador;
            if a.OperStatus == IfOperStatusUp && a.IfType != IF_TYPE_SOFTWARE_LOOPBACK {
                let mut unicast = a.FirstUnicastAddress;
                while !unicast.is_null() {
                    let sockaddr = (*unicast).Address.lpSockaddr;
                    if !sockaddr.is_null() && (*sockaddr).sa_family == AF_INET {
                        let sin = &*sockaddr.cast::<SOCKADDR_IN>();
                        enderecos.push(Ipv4Addr::from(sin.sin_addr.S_un.S_addr.to_ne_bytes()));
                    }
                    unicast = (*unicast).Next;
                }
            }
            adaptador = a.Next;
        }
    }
    Ok(enderecos)
}
