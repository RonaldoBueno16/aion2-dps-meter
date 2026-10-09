//! Captura passiva com raw socket do Windows (SIO_RCVALL), sem driver extra. Exige o processo
//! elevado. Abre um socket por IPv4 local e entrega cada segmento TCP no callback, que roda nas
//! threads de captura (uma por socket): quem recebe serializa (o pipeline não é thread-safe).
//! Uma thread vigia acompanha a rede: IP novo (outra Wi-Fi, DHCP, VPN, volta da suspensão) ganha
//! socket, o que sumiu é fechado, e o socket cuja thread parou é reaberto. Antes a captura morria
//! calada quando o IP mudava. Não abre handle nem lê nada do processo do jogo.

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

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
/// O jogo reconecta logo depois de o IP mudar, e o pacote de login (com o seu nome) vem nessa
/// conexão: o socket do endereço novo precisa abrir antes dele.
const CONFERIR_REDE_A_CADA: Duration = Duration::from_secs(2);

pub type AoSegmento = Arc<dyn Fn(SegmentoTcp, Hora) + Send + Sync>;
/// Se os sockets devem ficar abertos agora; a vigia pergunta a cada volta. O overlay fecha sem o jogo.
pub type Ativa = Arc<dyn Fn() -> bool + Send + Sync>;

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
    parando: Arc<AtomicBool>,
    pub contadores: Arc<Contadores>,
    enderecos: Arc<Mutex<Vec<Ipv4Addr>>>,
    vigia: Option<JoinHandle<()>>,
}

/// Socket aberto num endereço e a thread que recebe dele. Só a vigia mexe nisto.
struct Aberto {
    endereco: Ipv4Addr,
    socket: SOCKET,
    fio: JoinHandle<()>,
}

/// O que as threads de recepção compartilham.
struct Comum {
    parando: Arc<AtomicBool>,
    contadores: Arc<Contadores>,
    ao_segmento: AoSegmento,
    ativa: Ativa,
}

impl CapturaSocketBruto {
    /// Abre os sockets e começa a capturar. Err com a mensagem para o usuário quando havia
    /// interface e nenhuma abriu (sem administrador, todas falham). Sem interface nenhuma agora,
    /// começa assim mesmo: a vigia abre quando a rede voltar.
    pub fn iniciar(ao_segmento: AoSegmento) -> Result<Self, String> {
        Self::iniciar_quando(ao_segmento, Arc::new(|| true))
    }

    /// Como `iniciar`, com os sockets abertos só enquanto `ativa` responder sim: fechados, não
    /// recebem nada, e a vigia reabre em até CONFERIR_REDE_A_CADA. Inativa na largada, nada abre
    /// agora (e o erro de abrir só apareceria na vigia, que tenta de novo).
    pub fn iniciar_quando(ao_segmento: AoSegmento, ativa: Ativa) -> Result<Self, String> {
        let mut dados = unsafe { std::mem::zeroed::<WSADATA>() };
        let erro = unsafe { WSAStartup(0x0202, &mut dados) };
        if erro != 0 {
            return Err(format!("WSAStartup falhou ({erro})."));
        }

        let comum = Comum {
            parando: Arc::new(AtomicBool::new(false)),
            contadores: Arc::new(Contadores::default()),
            ao_segmento,
            ativa,
        };
        let enderecos = if (comum.ativa)() { enderecos_locais()? } else { Vec::new() };
        let mut abertos = Vec::new();
        let falha = abrir_varios(&mut abertos, &enderecos, &comum);
        if abertos.is_empty()
            && let Some(erro) = falha
        {
            return Err(erro);
        }

        let publicados = Arc::new(Mutex::new(abertos.iter().map(|a| a.endereco).collect()));
        let (parando, contadores) = (comum.parando.clone(), comum.contadores.clone());
        let vigia = {
            let publicados = publicados.clone();
            std::thread::Builder::new()
                .name("captura vigia".into())
                .spawn(move || vigiar(abertos, &comum, &publicados))
                .map_err(|e| e.to_string())?
        };
        Ok(Self { parando, contadores, enderecos: publicados, vigia: Some(vigia) })
    }

    /// Endereços com socket aberto agora (mudam com a rede).
    pub fn enderecos(&self) -> Vec<Ipv4Addr> {
        self.enderecos.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

/// Espera a vigia fechar todos os sockets e as threads de recepção terminarem. Quem derruba a
/// captura não pode estar segurando o lock que o callback usa, senão a espera não acaba.
impl Drop for CapturaSocketBruto {
    fn drop(&mut self) {
        self.parando.store(true, Ordering::SeqCst);
        if let Some(vigia) = self.vigia.take() {
            vigia.thread().unpark();
            let _ = vigia.join();
        }
    }
}

/// A cada CONFERIR_REDE_A_CADA compara os endereços da máquina com os sockets abertos. Sempre
/// fecha, espera a thread e só então abre: o Windows pode reaproveitar o número do socket fechado
/// no novo, e uma thread velha ainda viva leria dele.
fn vigiar(mut abertos: Vec<Aberto>, comum: &Comum, publicados: &Mutex<Vec<Ipv4Addr>>) {
    while !comum.parando.load(Ordering::SeqCst) {
        std::thread::park_timeout(CONFERIR_REDE_A_CADA);
        if comum.parando.load(Ordering::SeqCst) {
            break;
        }
        // Falha passageira ao listar as interfaces: tenta de novo na próxima volta. Inativa, é como
        // ficar sem rede: fecha tudo e espera.
        let atuais = if (comum.ativa)() { enderecos_locais() } else { Ok(Vec::new()) };
        let Ok(atuais) = atuais else { continue };
        let situacao: Vec<(Ipv4Addr, bool)> = abertos.iter().map(|a| (a.endereco, !a.fio.is_finished())).collect();
        let (fechar, abrir) = reconciliar(&situacao, &atuais);
        if fechar.is_empty() && abrir.is_empty() {
            continue;
        }
        let (manter, sair): (Vec<Aberto>, Vec<Aberto>) = abertos.into_iter().partition(|a| !fechar.contains(&a.endereco));
        abertos = manter;
        fechar_todos(sair);
        // Adaptador que não abre (virtual de VPN ou Hyper-V) fica de fora e é tentado de novo depois.
        let _ = abrir_varios(&mut abertos, &abrir, comum);
        *publicados.lock().unwrap_or_else(PoisonError::into_inner) = abertos.iter().map(|a| a.endereco).collect();
    }
    fechar_todos(abertos);
}

/// O que fechar (endereço que sumiu ou cuja thread parou) e o que abrir (endereço sem socket vivo).
fn reconciliar(abertos: &[(Ipv4Addr, bool)], atuais: &[Ipv4Addr]) -> (Vec<Ipv4Addr>, Vec<Ipv4Addr>) {
    let fechar = abertos.iter().filter(|(e, vivo)| !vivo || !atuais.contains(e)).map(|(e, _)| *e).collect();
    let mut abrir: Vec<Ipv4Addr> = Vec::new();
    for e in atuais {
        if !abertos.contains(&(*e, true)) && !abrir.contains(e) {
            abrir.push(*e);
        }
    }
    (fechar, abrir)
}

/// Abre um socket e uma thread por endereço; devolve o erro do primeiro que falhou.
fn abrir_varios(abertos: &mut Vec<Aberto>, enderecos: &[Ipv4Addr], comum: &Comum) -> Option<String> {
    let mut primeira_falha = None;
    for &endereco in enderecos {
        let socket = match abrir(endereco) {
            Ok(s) => s,
            Err(erro) => {
                primeira_falha.get_or_insert(erro);
                continue;
            }
        };
        let (parando, contadores, ao_segmento) =
            (comum.parando.clone(), comum.contadores.clone(), comum.ao_segmento.clone());
        match std::thread::Builder::new()
            .name(format!("captura {endereco}"))
            .spawn(move || receber(socket, endereco, &parando, &contadores, &*ao_segmento))
        {
            Ok(fio) => abertos.push(Aberto { endereco, socket, fio }),
            Err(erro) => {
                unsafe { closesocket(socket) };
                primeira_falha.get_or_insert(erro.to_string());
            }
        }
    }
    primeira_falha
}

/// Fechar o socket destrava o recv da thread dele, que então termina.
fn fechar_todos(abertos: Vec<Aberto>) {
    for a in abertos {
        unsafe { closesocket(a.socket) };
        let _ = a.fio.join();
    }
}

fn abrir(endereco: Ipv4Addr) -> Result<SOCKET, String> {
    unsafe {
        let s = socket(i32::from(AF_INET), SOCK_RAW, IPPROTO_IP);
        if s == INVALID_SOCKET {
            // Sem elevação o Windows recusa já na criação do raw socket.
            return Err(mensagem_de_erro(WSAGetLastError(), "socket"));
        }
        let falhou = |etapa: &str| {
            let erro = WSAGetLastError();
            closesocket(s);
            mensagem_de_erro(erro, &format!("{etapa} em {endereco}"))
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

fn mensagem_de_erro(erro: i32, etapa: &str) -> String {
    if erro == WSAEACCES {
        "Captura por raw socket precisa rodar como administrador.".to_string()
    } else {
        format!("{etapa} falhou ({erro}).")
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn vigia_fecha_o_que_sumiu_e_abre_o_que_apareceu() {
        let (casa, vpn, outra) = (Ipv4Addr::new(192, 168, 0, 2), Ipv4Addr::new(26, 1, 2, 3), Ipv4Addr::new(10, 0, 0, 7));
        // Nada mudou.
        assert_eq!(reconciliar(&[(casa, true)], &[casa]), (vec![], vec![]));
        // IP mudou (outra Wi-Fi, DHCP).
        assert_eq!(reconciliar(&[(casa, true)], &[outra]), (vec![casa], vec![outra]));
        // VPN ligou: só abre a nova, a de casa continua.
        assert_eq!(reconciliar(&[(casa, true)], &[casa, vpn]), (vec![], vec![vpn]));
        // Thread parou num endereço que continua: fecha e reabre.
        assert_eq!(reconciliar(&[(casa, false)], &[casa]), (vec![casa], vec![casa]));
        // Sem rede: fecha tudo e espera.
        assert_eq!(reconciliar(&[(casa, true)], &[]), (vec![casa], vec![]));
        // Endereço que não abriu antes (fora da lista de abertos) é tentado de novo.
        assert_eq!(reconciliar(&[], &[vpn, vpn]), (vec![], vec![vpn]));
    }
}
