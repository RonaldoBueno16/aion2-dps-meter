//! Regra de entrada no Firewall do Windows para o próprio Axon. Sem ela, o firewall pode descartar
//! os pacotes que chegam do servidor antes de o raw socket vê-los: a captura só enxerga o que sai
//! e o medidor fica procurando o servidor. O aviso que o Windows mostra no primeiro uso pode ficar
//! atrás do jogo em tela cheia, e um "Cancelar" nele cria regras de bloqueio, que vencem as de
//! liberação. Antivírus com firewall próprio (Kaspersky, Avast...) ignoram estas regras; para eles
//! o rodapé avisa (ver `janela::procurando`).

use std::ffi::OsString;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const NOME: &str = "Axon (captura)";
/// CREATE_NO_WINDOW: sem ele cada netsh pisca um console na abertura.
const SEM_JANELA: u32 = 0x0800_0000;

/// Roda antes de abrir os sockets: a regra vale para o socket a partir do bind. Com a regra deste
/// exe já criada numa abertura anterior, só confere (uma chamada); senão apaga as regras de entrada
/// deste exe (inclusive bloqueios do aviso do Windows) e as "Axon (captura)" de outras pastas, e
/// cria a liberação. Só entrada e só para este exe. Falha calada: sem elevação (build de debug) ou
/// com o serviço do firewall parado não há o que fazer, e o rodapé avisa se a entrada continuar
/// barrada.
pub fn liberar() {
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.display().to_string();
    if let Some(saida) = netsh(&format!("show rule name=\"{NOME}\" verbose")) {
        // O caminho sai como foi gravado, em qualquer idioma do Windows. Com acento no caminho a
        // página de código do console pode não bater: aí só refaz a regra.
        let texto = String::from_utf8_lossy(&saida.stdout).to_lowercase();
        if saida.status.success() && texto.contains(&exe.to_lowercase()) {
            return;
        }
    }
    netsh(&format!("delete rule name=\"{NOME}\""));
    // O aviso do Windows pode gravar o caminho em minúsculas.
    let minusculo = exe.to_lowercase();
    for caminho in if minusculo == exe { vec![&exe] } else { vec![&exe, &minusculo] } {
        netsh(&format!("delete rule name=all dir=in program=\"{caminho}\""));
    }
    netsh(&format!("add rule name=\"{NOME}\" dir=in action=allow program=\"{exe}\" enable=yes profile=any"));
}

/// `raw_arg`: o netsh lê `program="C:\caminho com espaço"` do jeito digitado no prompt; o `arg`
/// do Rust poria aspas em volta do argumento inteiro. Caminho absoluto: o processo é elevado e não
/// deve achar um netsh.exe qualquer na pasta do Axon.
fn netsh(argumentos: &str) -> Option<Output> {
    let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| OsString::from(r"C:\Windows"));
    Command::new(PathBuf::from(windows).join(r"System32\netsh.exe"))
        .raw_arg("advfirewall firewall")
        .raw_arg(argumentos)
        .creation_flags(SEM_JANELA)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
}
