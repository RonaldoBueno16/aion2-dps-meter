//! Atalho global de teclado no formato do config.json ("Ctrl+H", "Ctrl+Shift+F9"), lido para o
//! RegisterHotKey. A bandeja registra só com o jogo em primeiro plano: enquanto registrado, a
//! combinação não chega a nenhum outro programa.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

#[derive(Clone, Debug, PartialEq)]
pub struct Atalho {
    pub modificadores: u32,
    /// Virtual-key code do Windows.
    pub tecla: u32,
    /// Como foi escrito na config, para mostrar na tela.
    pub texto: String,
}

impl Atalho {
    /// None com texto vazio (atalho desligado) ou inválido. Exige Ctrl, Alt ou Win: uma letra
    /// sozinha (ou só com Shift) roubaria o que se digita no chat do jogo.
    pub fn ler(texto: &str) -> Option<Self> {
        let mut modificadores = 0;
        let mut tecla = None;
        for parte in texto.split('+').map(str::trim) {
            let maiuscula = parte.to_ascii_uppercase();
            match maiuscula.as_str() {
                "CTRL" | "CONTROL" => modificadores |= MOD_CONTROL,
                "SHIFT" => modificadores |= MOD_SHIFT,
                "ALT" => modificadores |= MOD_ALT,
                "WIN" => modificadores |= MOD_WIN,
                _ if tecla.is_none() => tecla = Some(codigo_da_tecla(&maiuscula)?),
                _ => return None,
            }
        }
        let tecla = tecla?;
        (modificadores & (MOD_CONTROL | MOD_ALT | MOD_WIN) != 0).then(|| Atalho {
            modificadores,
            tecla,
            texto: texto.trim().to_string(),
        })
    }
}

/// A..Z, 0..9 e F1..F24.
fn codigo_da_tecla(nome: &str) -> Option<u32> {
    let bytes = nome.as_bytes();
    if let [c] = bytes
        && (c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Some(u32::from(*c)); // VK_A..VK_Z e VK_0..VK_9 são os próprios códigos ASCII
    }
    let numero: u32 = nome.strip_prefix('F')?.parse().ok()?;
    (1..=24).contains(&numero).then(|| 0x70 + numero - 1) // VK_F1 = 0x70
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_combinacoes_e_recusa_tecla_sem_ctrl_alt_ou_win() {
        let a = Atalho::ler("Ctrl+H").unwrap();
        assert_eq!((a.modificadores, a.tecla, a.texto.as_str()), (MOD_CONTROL, 0x48, "Ctrl+H"));
        let b = Atalho::ler(" ctrl + shift + f9 ").unwrap();
        assert_eq!((b.modificadores, b.tecla), (MOD_CONTROL | MOD_SHIFT, 0x78));
        assert_eq!(Atalho::ler("Alt+1").unwrap().tecla, 0x31);
        // Desligado, sem modificador que proteja o chat, tecla desconhecida ou duas teclas.
        for invalido in ["", "H", "Shift+H", "Ctrl+Enter", "Ctrl+F25", "Ctrl+H+J", "Ctrl+"] {
            assert_eq!(Atalho::ler(invalido), None, "{invalido}");
        }
    }
}
