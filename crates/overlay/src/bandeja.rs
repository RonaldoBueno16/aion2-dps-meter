//! Ícone na área de notificação (a seta ao lado do relógio) enquanto o Axon roda. Clique esquerdo
//! liga ou desliga o overlay; clique direito abre o menu. Ligado, o overlay fica visível com o jogo
//! na frente, atrás de outra janela, minimizado ou fechado, sempre dentro da área dele; escondido,
//! o medidor continua contando.
//! Fica numa thread própria, com uma janela oculta para receber os cliques, o temporizador e os
//! atalhos globais: com o overlay escondido, o egui para de desenhar e não teria como trazê-lo de
//! volta. O RegisterHotKey só vale na thread da janela que recebe o WM_HOTKEY, por isso fica aqui.
//! Os alertas também: um segundo temporizador confere a cada segundo, com o overlay escondido ou não.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, mpsc};
use std::thread::JoinHandle;

use eframe::egui;
use nucleo::medicao::sessao::Sessao;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_REALTIME, NIF_TIP, NIIF_LARGE_ICON, NIIF_NOSOUND, NIIF_RESPECT_QUIET_TIME,
    NIIF_USER, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE,
    SHQueryUserNotificationState, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, ChangeWindowMessageFilterEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW,
    GetWindowRect, IMAGE_ICON, IsWindowVisible, LR_DEFAULTCOLOR, LoadImageW, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    MF_UNCHECKED, MSG, MSGFLT_ALLOW, PostMessageW, PostQuitMessage,
    RegisterClassW, RegisterWindowMessageW, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE,
    WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
};

use crate::alertas::{self, Alerta};
use crate::atalho::Atalho;
use crate::config::{Alertas, Balao};
use crate::{janela, jogo};

/// Mensagem que o Windows manda à janela oculta quando o ícone é clicado.
const AVISO: u32 = WM_APP + 1;
const ALTERNAR: usize = 1;
const FECHAR: usize = 2;
const ATRAVESSAR: usize = 3;
const ALERTAS: usize = 4;
/// "TaskbarCreated": o Explorer reiniciou e o ícone precisa ser posto de novo.
static BARRA_RECRIADA: AtomicU32 = AtomicU32::new(0);
/// A chave da bandeja. Desligado, o overlay não aparece.
static LIGADO: AtomicBool = AtomicBool::new(true);
/// O clique passa pelo overlay e chega ao jogo. Quem aplica na janela é o overlay, a cada quadro.
static ATRAVESSANDO: AtomicBool = AtomicBool::new(false);
/// Fechando: o temporizador para de esconder o overlay.
static SAINDO: AtomicBool = AtomicBool::new(false);
const CONFERIR_A_CADA_MS: u32 = 200;
/// Os temporizadores da janela oculta: o do overlay e dos atalhos, e o dos alertas.
const TEMPORIZADOR_OVERLAY: usize = 1;
const TEMPORIZADOR_ALERTAS: usize = 2;
const ALERTAS_A_CADA_MS: u32 = 1000;
/// O overlay recolhido na borda: a faixa do alerta não aparece. Quem grava é o overlay, a cada quadro.
static RECOLHIDO: AtomicBool = AtomicBool::new(false);

/// Os atalhos, na ordem do id do WM_HOTKEY menos 1.
pub const MOSTRAR: usize = 0;
pub const ALTERNAR_CLIQUE: usize = 1;
pub const COPIAR_RESUMO: usize = 2;
pub const ALTERNAR_COMPACTA: usize = 3;
pub const ATALHOS_TOTAL: usize = 4;
static ATALHOS: OnceLock<[Option<Atalho>; ATALHOS_TOTAL]> = OnceLock::new();
static REGISTRADO: [AtomicBool; ATALHOS_TOTAL] = [const { AtomicBool::new(false) }; ATALHOS_TOTAL];
/// O Windows recusou o atalho: outro programa já usa a combinação.
static OCUPADO: [AtomicBool; ATALHOS_TOTAL] = [const { AtomicBool::new(false) }; ATALHOS_TOTAL];
/// Pedidos dos atalhos que mexem no conteúdo do overlay: ele atende no próximo quadro.
static PEDIU_RESUMO: AtomicBool = AtomicBool::new(false);
static PEDIU_COMPACTA: AtomicBool = AtomicBool::new(false);

pub struct Bandeja {
    janela: isize,
    fio: Option<JoinHandle<()>>,
}

/// O que o temporizador dos alertas precisa: a sessão (as listas de chefes de campo) e o egui, para
/// pedir um quadro quando há faixa nova.
pub struct Vigia {
    pub sessao: Arc<Mutex<Sessao>>,
    pub ctx: egui::Context,
    pub memoria: alertas::Memoria,
}

thread_local! {
    static VIGIA: RefCell<Option<Vigia>> = const { RefCell::new(None) };
}

impl Bandeja {
    /// `overlay` = HWND da janela do medidor. None se a janela oculta não pôde ser criada.
    pub fn iniciar(overlay: isize, atalhos: [Option<Atalho>; ATALHOS_TOTAL], vigia: Vigia) -> Option<Self> {
        let _ = ATALHOS.set(atalhos);
        let (avisar, pronta) = mpsc::channel();
        let fio = std::thread::spawn(move || unsafe {
            VIGIA.set(Some(vigia));
            laco(overlay, avisar)
        });
        let janela = pronta.recv().ok().filter(|&j| j != 0)?;
        Some(Self { janela, fio: Some(fio) })
    }
}

impl Drop for Bandeja {
    /// Tira o ícone na hora: sem isto ele fica na bandeja até o mouse passar por cima.
    fn drop(&mut self) {
        unsafe { SendMessageW(self.janela as HWND, WM_CLOSE, 0, 0) };
        if let Some(fio) = self.fio.take() {
            let _ = fio.join();
        }
    }
}

unsafe fn laco(overlay: isize, avisar: mpsc::Sender<isize>) {
    let classe = utf16("AxonBandeja");
    unsafe {
        let instancia = GetModuleHandleW(std::ptr::null());
        let mut registro: WNDCLASSW = std::mem::zeroed();
        registro.lpfnWndProc = Some(procedimento);
        registro.hInstance = instancia;
        registro.lpszClassName = classe.as_ptr();
        RegisterClassW(&registro);
        // Janela comum e oculta, e não "message-only": só as comuns recebem o TaskbarCreated.
        let janela = CreateWindowExW(
            0,
            classe.as_ptr(),
            classe.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instancia,
            std::ptr::null(),
        );
        if janela.is_null() {
            let _ = avisar.send(0);
            return;
        }
        SetWindowLongPtrW(janela, GWLP_USERDATA, overlay);
        let barra_recriada = RegisterWindowMessageW(utf16("TaskbarCreated").as_ptr());
        BARRA_RECRIADA.store(barra_recriada, Ordering::Relaxed);
        // A release roda como administrador e o Explorer não: sem isto o Windows (UIPI) pode
        // barrar os cliques no ícone e o aviso de Explorer reiniciado.
        for mensagem in [AVISO, barra_recriada] {
            ChangeWindowMessageFilterEx(janela, mensagem, MSGFLT_ALLOW, std::ptr::null_mut());
        }
        // Sem Explorer agora: o TaskbarCreated põe o ícone quando ele voltar.
        icone_na_bandeja(janela, NIM_ADD);
        SetTimer(janela, TEMPORIZADOR_OVERLAY, CONFERIR_A_CADA_MS, None);
        SetTimer(janela, TEMPORIZADOR_ALERTAS, ALERTAS_A_CADA_MS, None);
        let _ = avisar.send(janela as isize);

        let mut mensagem: MSG = std::mem::zeroed();
        while GetMessageW(&mut mensagem, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&mensagem);
            DispatchMessageW(&mensagem);
        }
    }
}

unsafe extern "system" fn procedimento(janela: HWND, mensagem: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        let overlay = GetWindowLongPtrW(janela, GWLP_USERDATA) as HWND;
        match mensagem {
            AVISO => match (l & 0xFFFF) as u32 {
                WM_LBUTTONUP => alternar(overlay),
                WM_RBUTTONUP => menu(janela, overlay),
                _ => {}
            },
            WM_TIMER if w == TEMPORIZADOR_ALERTAS => vigiar(janela),
            WM_TIMER => {
                atualizar(overlay);
                conferir_atalhos(janela);
            }
            WM_HOTKEY => match w {
                1 => alternar(overlay),
                2 => alternar_clique(),
                3 => PEDIU_RESUMO.store(true, Ordering::Relaxed),
                4 => PEDIU_COMPACTA.store(true, Ordering::Relaxed),
                _ => {}
            },
            WM_CLOSE => {
                icone_na_bandeja(janela, NIM_DELETE);
                DestroyWindow(janela);
            }
            WM_DESTROY => PostQuitMessage(0),
            _ if mensagem != 0 && mensagem == BARRA_RECRIADA.load(Ordering::Relaxed) => {
                icone_na_bandeja(janela, NIM_ADD);
            }
            _ => return DefWindowProcW(janela, mensagem, w, l),
        }
        0
    }
}

unsafe fn icone_na_bandeja(janela: HWND, acao: u32) {
    unsafe {
        let mut dados: NOTIFYICONDATAW = std::mem::zeroed();
        dados.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        dados.hWnd = janela;
        dados.uID = 1;
        if acao == NIM_ADD {
            dados.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            dados.uCallbackMessage = AVISO;
            // Ícone 1 do exe (assets/axon.ico, embutido pelo build.rs), no tamanho pequeno do sistema.
            let (largura, altura) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
            dados.hIcon = LoadImageW(GetModuleHandleW(std::ptr::null()), 1 as _, IMAGE_ICON, largura, altura, LR_DEFAULTCOLOR);
            for (destino, letra) in dados.szTip.iter_mut().zip("Axon".encode_utf16()) {
                *destino = letra;
            }
        }
        Shell_NotifyIconW(acao, &dados);
    }
}

unsafe fn menu(janela: HWND, overlay: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        let rotulo = if LIGADO.load(Ordering::Relaxed) { "Esconder overlay" } else { "Mostrar overlay" };
        AppendMenuW(menu, MF_STRING, ALTERNAR, utf16(rotulo).as_ptr());
        // Com o clique atravessando, o overlay não responde ao mouse: o menu é a saída garantida
        // (o atalho pode estar ocupado ou desligado).
        let marca = if ATRAVESSANDO.load(Ordering::Relaxed) { MF_CHECKED } else { MF_UNCHECKED };
        AppendMenuW(menu, MF_STRING | marca, ATRAVESSAR, utf16("Clique atravessa o overlay").as_ptr());
        let marca = if alertas::ligados() { MF_CHECKED } else { MF_UNCHECKED };
        AppendMenuW(menu, MF_STRING | marca, ALERTAS, utf16("Alertas").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, FECHAR, utf16("Fechar Axon").as_ptr());
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        // Sem a janela em primeiro plano, o menu não fecha ao clicar fora dele.
        SetForegroundWindow(janela);
        let escolha = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            cursor.x,
            cursor.y,
            0,
            janela,
            std::ptr::null(),
        );
        PostMessageW(janela, WM_NULL, 0, 0);
        DestroyMenu(menu);
        match escolha as usize {
            ALTERNAR => alternar(overlay),
            ATRAVESSAR => alternar_clique(),
            ALERTAS => alertas::alternar_ligados(),
            FECHAR => fechar(overlay),
            _ => {}
        }
    }
}

/// O overlay está recolhido na borda (a faixa do alerta não aparece).
pub fn definir_recolhido(recolhido: bool) {
    RECOLHIDO.store(recolhido, Ordering::Relaxed);
}

/// A cada segundo: os alertas que vencem agora e o "Testar alerta". No replay (sem seguir o jogo),
/// só o teste.
unsafe fn vigiar(janela: HWND) {
    VIGIA.with_borrow_mut(|vigia| {
        let Some(vigia) = vigia else { return };
        let Some(regras) = alertas::regras() else { return };
        let agora = nucleo::agora();
        let mut saida = Vec::new();
        if jogo::seguindo() {
            let (desejos, minima) = alertas::desejos();
            let chefes = if regras.chefes.is_empty() && desejos.is_empty() {
                Vec::new()
            } else {
                let sessao = vigia.sessao.lock().unwrap_or_else(PoisonError::into_inner);
                janela::chefes_marcados(&sessao.medidor.chefes_por_regiao, &regras.chefes, &desejos, minima, agora)
            };
            saida = alertas::vencidos(&regras, &chefes, agora, jogo::aberto(), &mut vigia.memoria);
        }
        if alertas::pediu_teste() {
            saida.push(alertas::alerta_de_teste(agora));
        }
        if !saida.is_empty() {
            unsafe { entregar(janela, &regras, saida, &vigia.ctx) };
        }
    });
}

/// Som, balão e faixa. Com a tela bloqueada ou o modo apresentação, nem som nem balão: a faixa
/// fica para quando o overlay voltar a desenhar.
unsafe fn entregar(janela: HWND, regras: &Alertas, saida: Vec<Alerta>, ctx: &egui::Context) {
    let mut estado = 0;
    let ausente = unsafe { SHQueryUserNotificationState(&mut estado) } == 0
        && matches!(estado, QUNS_NOT_PRESENT | QUNS_PRESENTATION_MODE);
    let som = regras.som && !ausente;
    if som {
        alertas::tocar();
    }
    let faixa_aparece = LIGADO.load(Ordering::Relaxed) && !RECOLHIDO.load(Ordering::Relaxed);
    let balao = !ausente
        && match regras.balao() {
            Balao::Nunca => false,
            Balao::SemBanner => !faixa_aparece,
            Balao::Sempre => true,
        };
    if balao {
        let (titulo, texto) = alertas::texto_do_balao(&saida);
        unsafe { balao_na_bandeja(janela, &titulo, &texto, som) };
    }
    alertas::enfileirar(saida);
    ctx.request_repaint();
}

/// O balão do ícone. NIF_REALTIME: se o Windows não puder mostrar agora ("Não incomodar"), descarta
/// em vez de mostrar depois, com a hora errada. Sem som próprio quando o do Axon já tocou.
unsafe fn balao_na_bandeja(janela: HWND, titulo: &str, texto: &str, mudo: bool) {
    unsafe {
        let mut dados: NOTIFYICONDATAW = std::mem::zeroed();
        dados.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        dados.hWnd = janela;
        dados.uID = 1;
        dados.uFlags = NIF_INFO | NIF_REALTIME;
        copiar_terminado(&mut dados.szInfoTitle, titulo);
        copiar_terminado(&mut dados.szInfo, texto);
        dados.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON | NIIF_RESPECT_QUIET_TIME | if mudo { NIIF_NOSOUND } else { 0 };
        dados.hBalloonIcon = icone_grande() as _;
        Shell_NotifyIconW(NIM_MODIFY, &dados);
    }
}

/// O ícone do exe no tamanho grande do sistema, que o NIIF_LARGE_ICON pede; carregado uma vez.
unsafe fn icone_grande() -> isize {
    static ICONE: AtomicIsize = AtomicIsize::new(0);
    let carregado = ICONE.load(Ordering::Relaxed);
    if carregado != 0 {
        return carregado;
    }
    let icone = unsafe {
        let (largura, altura) = (GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON));
        LoadImageW(GetModuleHandleW(std::ptr::null()), 1 as _, IMAGE_ICON, largura, altura, LR_DEFAULTCOLOR)
    } as isize;
    ICONE.store(icone, Ordering::Relaxed);
    icone
}

/// UTF-16 com o zero no fim, cortado no tamanho do campo.
fn copiar_terminado(destino: &mut [u16], texto: &str) {
    let letras: Vec<u16> = texto.encode_utf16().take(destino.len() - 1).collect();
    destino[..letras.len()].copy_from_slice(&letras);
    destino[letras.len()] = 0;
}

fn alternar_clique() {
    ATRAVESSANDO.fetch_xor(true, Ordering::Relaxed);
}

/// O clique deve passar pelo overlay até o jogo.
pub fn atravessando() -> bool {
    ATRAVESSANDO.load(Ordering::Relaxed)
}

/// Atalhos de resumo e de barra compacta apertados desde a última pergunta: (resumo, compacta).
pub fn pedidos() -> (bool, bool) {
    (PEDIU_RESUMO.swap(false, Ordering::Relaxed), PEDIU_COMPACTA.swap(false, Ordering::Relaxed))
}

/// O atalho (MOSTRAR, ALTERNAR_CLIQUE...) e se o Windows o recusou por estar em uso por outro
/// programa. None com o atalho desligado ou inválido na config.
pub fn atalho(qual: usize) -> Option<(&'static str, bool)> {
    let atalho = ATALHOS.get()?[qual].as_ref()?;
    Some((atalho.texto.as_str(), OCUPADO[qual].load(Ordering::Relaxed)))
}

/// Registra os atalhos só com o jogo na frente (sem seguir o jogo, sempre) e tira quando ele sai:
/// enquanto registrada, a combinação não chega a nenhum outro programa (Ctrl+T abre aba no
/// navegador, Ctrl+H o histórico).
unsafe fn conferir_atalhos(janela: HWND) {
    let Some(atalhos) = ATALHOS.get() else { return };
    let querer = !jogo::seguindo() || jogo::em_primeiro_plano();
    for (i, atalho) in atalhos.iter().enumerate() {
        let Some(a) = atalho else { continue };
        let id = i as i32 + 1;
        let registrado = REGISTRADO[i].load(Ordering::Relaxed);
        if querer && !registrado {
            let ok = unsafe { RegisterHotKey(janela, id, a.modificadores | MOD_NOREPEAT, a.tecla) } != 0;
            REGISTRADO[i].store(ok, Ordering::Relaxed);
            OCUPADO[i].store(!ok, Ordering::Relaxed);
        } else if !querer && registrado {
            unsafe { UnregisterHotKey(janela, id) };
            REGISTRADO[i].store(false, Ordering::Relaxed);
        }
    }
}

/// O mesmo caminho do ✕, que salva a memória antes de sair. O eframe só fecha no quadro seguinte
/// ao WM_CLOSE, e janela escondida não ganha quadro: ela volta à tela (sem ativar) por um instante.
unsafe fn fechar(overlay: HWND) {
    SAINDO.store(true, Ordering::Relaxed);
    unsafe {
        ShowWindow(overlay, SW_SHOWNOACTIVATE);
        PostMessageW(overlay, WM_CLOSE, 0, 0);
    }
}

unsafe fn alternar(overlay: HWND) {
    LIGADO.fetch_xor(true, Ordering::Relaxed);
    unsafe { atualizar(overlay) };
}

/// Mostra o overlay com ele ligado (sem tirar o foco do jogo), com o jogo na frente ou não, e o
/// traz para dentro da área do jogo se estiver fora dela (outro monitor, jogo em janela que mudou
/// de lugar, zoom que passou da borda). Com o jogo minimizado, fica onde está.
unsafe fn atualizar(overlay: HWND) {
    if SAINDO.load(Ordering::Relaxed) {
        return;
    }
    let aparece = LIGADO.load(Ordering::Relaxed);
    unsafe {
        if aparece != (IsWindowVisible(overlay) != 0) {
            ShowWindow(overlay, if aparece { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if !aparece || GetWindowRect(overlay, &mut r) == 0 {
            return;
        }
        let (x, y) = jogo::dentro(r.left, r.top, r.right - r.left, r.bottom - r.top);
        if (x, y) != (r.left, r.top) {
            SetWindowPos(overlay, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

pub fn utf16(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain([0]).collect()
}
