//! Ícone na área de notificação (a seta ao lado do relógio) enquanto o Axon roda. Clique esquerdo
//! liga ou desliga o overlay; clique direito abre o menu. Ligado, o overlay só aparece com o jogo
//! em primeiro plano e fica dentro da área dele; escondido, o medidor continua contando.
//! Fica numa thread própria, com uma janela oculta para receber os cliques e o temporizador: com o
//! overlay escondido, o egui para de desenhar e não teria como trazê-lo de volta.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, IMAGE_ICON,
    IsWindowVisible, LR_DEFAULTCOLOR, LoadImageW, MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SM_CYSMICON, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE,
    WM_DESTROY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
};

use crate::jogo;

/// Mensagem que o Windows manda à janela oculta quando o ícone é clicado.
const AVISO: u32 = WM_APP + 1;
/// "Fechar Axon" do menu (também usada pelo teste, que não tem como abrir o menu).
const FECHAR_AXON: u32 = WM_APP + 2;
const ALTERNAR: usize = 1;
const FECHAR: usize = 2;
/// "TaskbarCreated": o Explorer reiniciou e o ícone precisa ser posto de novo.
static BARRA_RECRIADA: AtomicU32 = AtomicU32::new(0);
/// A chave da bandeja. Desligado, o overlay não aparece nem por cima do jogo.
static LIGADO: AtomicBool = AtomicBool::new(true);
/// Fechando: o temporizador para de esconder o overlay.
static SAINDO: AtomicBool = AtomicBool::new(false);
const CONFERIR_A_CADA_MS: u32 = 200;

pub struct Bandeja {
    janela: isize,
    fio: Option<JoinHandle<()>>,
}

impl Bandeja {
    /// `overlay` = HWND da janela do medidor. None se a janela oculta não pôde ser criada.
    pub fn iniciar(overlay: isize) -> Option<Self> {
        let (avisar, pronta) = mpsc::channel();
        let fio = std::thread::spawn(move || unsafe { laco(overlay, avisar) });
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
        BARRA_RECRIADA.store(RegisterWindowMessageW(utf16("TaskbarCreated").as_ptr()), Ordering::Relaxed);
        // Sem Explorer agora: o TaskbarCreated põe o ícone quando ele voltar.
        icone_na_bandeja(janela, NIM_ADD);
        SetTimer(janela, 1, CONFERIR_A_CADA_MS, None);
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
            WM_TIMER => atualizar(overlay),
            FECHAR_AXON => fechar(overlay),
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
            FECHAR => fechar(overlay),
            _ => {}
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

/// Mostra o overlay só com ele ligado e o jogo em primeiro plano (sem tirar o foco do jogo), e o
/// traz para dentro da área do jogo se estiver fora dela (outro monitor, jogo em janela que mudou
/// de lugar, zoom que passou da borda).
unsafe fn atualizar(overlay: HWND) {
    if SAINDO.load(Ordering::Relaxed) {
        return;
    }
    unsafe {
        let aparece = LIGADO.load(Ordering::Relaxed) && (!jogo::seguindo() || jogo::em_primeiro_plano());
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

fn utf16(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain([0]).collect()
}
