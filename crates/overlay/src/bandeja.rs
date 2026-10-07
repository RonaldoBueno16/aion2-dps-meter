//! Ícone na área de notificação (a seta ao lado do relógio) enquanto o Axon roda. Clique esquerdo
//! liga ou desliga o overlay; clique direito abre o menu. Ligado, o overlay fica visível com o jogo
//! na frente, atrás de outra janela, minimizado ou fechado, e onde for arrastado (outro monitor
//! inclusive); escondido, o medidor continua contando.
//! Fica numa thread própria, com uma janela oculta para receber os cliques, o temporizador e os
//! atalhos globais: com o overlay escondido, o egui para de desenhar e não teria como trazê-lo de
//! volta. O RegisterHotKey só vale na thread da janela que recebe o WM_HOTKEY, por isso fica aqui.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, ChangeWindowMessageFilterEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW,
    IMAGE_ICON, IsWindowVisible, LR_DEFAULTCOLOR, LoadImageW, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    MF_UNCHECKED, MSG, MSGFLT_ALLOW, PostMessageW, PostQuitMessage,
    RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SM_CYSMICON, SW_HIDE, SW_SHOWNOACTIVATE,
    SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE,
    WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
};

use crate::atalho::Atalho;
use crate::jogo;

/// Mensagem que o Windows manda à janela oculta quando o ícone é clicado.
const AVISO: u32 = WM_APP + 1;
const ALTERNAR: usize = 1;
const FECHAR: usize = 2;
const ATRAVESSAR: usize = 3;
/// "TaskbarCreated": o Explorer reiniciou e o ícone precisa ser posto de novo.
static BARRA_RECRIADA: AtomicU32 = AtomicU32::new(0);
/// A chave da bandeja. Desligado, o overlay não aparece.
static LIGADO: AtomicBool = AtomicBool::new(true);
/// O clique passa pelo overlay e chega ao jogo. Quem aplica na janela é o overlay, a cada quadro.
static ATRAVESSANDO: AtomicBool = AtomicBool::new(false);
/// Fechando: o temporizador para de esconder o overlay.
static SAINDO: AtomicBool = AtomicBool::new(false);
const CONFERIR_A_CADA_MS: u32 = 200;

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

impl Bandeja {
    /// `overlay` = HWND da janela do medidor. None se a janela oculta não pôde ser criada.
    pub fn iniciar(overlay: isize, atalhos: [Option<Atalho>; ATALHOS_TOTAL]) -> Option<Self> {
        let _ = ATALHOS.set(atalhos);
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
        let barra_recriada = RegisterWindowMessageW(utf16("TaskbarCreated").as_ptr());
        BARRA_RECRIADA.store(barra_recriada, Ordering::Relaxed);
        // A release roda como administrador e o Explorer não: sem isto o Windows (UIPI) pode
        // barrar os cliques no ícone e o aviso de Explorer reiniciado.
        for mensagem in [AVISO, barra_recriada] {
            ChangeWindowMessageFilterEx(janela, mensagem, MSGFLT_ALLOW, std::ptr::null_mut());
        }
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
            FECHAR => fechar(overlay),
            _ => {}
        }
    }
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

/// Mostra o overlay com ele ligado (sem tirar o foco do jogo) e o esconde desligado. Não depende
/// do jogo: minimizado ou atrás de outra janela, o overlay continua onde está.
unsafe fn atualizar(overlay: HWND) {
    if SAINDO.load(Ordering::Relaxed) {
        return;
    }
    let aparece = LIGADO.load(Ordering::Relaxed);
    unsafe {
        if aparece != (IsWindowVisible(overlay) != 0) {
            ShowWindow(overlay, if aparece { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
    }
}

pub fn utf16(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain([0]).collect()
}
