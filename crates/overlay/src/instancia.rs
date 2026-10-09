//! Um Axon só por sessão do Windows. Com o início junto com o Windows, abrir o exe de novo criaria
//! um segundo socket de captura, um segundo ícone e alertas em dobro: a segunda instância pede à
//! primeira que apareça e sai. Um mutex nomeado marca quem chegou primeiro; o Windows o solta quando
//! o processo termina, de qualquer jeito que termine.

use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, RegisterWindowMessageW};

use crate::bandeja::{CLASSE, utf16};

/// O build de debug tem o seu, para rodar ao lado do Axon de verdade.
const MUTEX: &str = if cfg!(debug_assertions) { "Local\\Axon-instancia-debug" } else { "Local\\Axon-instancia" };
/// O pedido de "apareça", que a bandeja da primeira instância atende.
pub const PEDIDO_MOSTRAR: &str = "Axon.Mostrar";

/// Segura o mutex até o processo sair.
pub struct Unica(HANDLE);

impl Drop for Unica {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Some se esta é a primeira instância. `esperar`: depois do Atualizar, a versão velha ainda está
/// fechando; tenta de novo até esse prazo. Sem conseguir criar o mutex, segue sem a trava: abrir em
/// dobro é melhor que não abrir.
pub fn garantir(esperar: Duration) -> Option<Unica> {
    let nome = utf16(MUTEX);
    let fim = Instant::now() + esperar;
    loop {
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, nome.as_ptr()) };
        if mutex.is_null() {
            return Some(Unica(mutex));
        }
        if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
            return Some(Unica(mutex));
        }
        unsafe { CloseHandle(mutex) };
        if Instant::now() >= fim {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Pede à primeira instância que mostre o overlay (pela janela oculta da bandeja dela).
pub fn pedir_para_aparecer() {
    unsafe {
        let janela = FindWindowW(utf16(CLASSE).as_ptr(), std::ptr::null());
        if !janela.is_null() {
            PostMessageW(janela, RegisterWindowMessageW(utf16(PEDIDO_MOSTRAR).as_ptr()), 0, 0);
        }
    }
}
