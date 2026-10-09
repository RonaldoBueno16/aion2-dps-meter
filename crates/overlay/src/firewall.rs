//! Regra de entrada no Firewall do Windows para o próprio Axon. Sem ela, o firewall pode descartar
//! os pacotes que chegam do servidor antes de o raw socket vê-los: a captura só enxerga o que sai
//! e o medidor fica procurando o servidor. O aviso que o Windows mostra no primeiro uso pode ficar
//! atrás do jogo em tela cheia, e um "Cancelar" nele cria regras de bloqueio, que vencem as de
//! liberação. Antivírus com firewall próprio (Kaspersky, Avast...) ignoram estas regras; para eles
//! o rodapé avisa (ver `janela::procurando`). Desde a 0.6.0 a regra só é criada depois de o usuário
//! clicar em Liberar no aviso do overlay (ver `janela::aviso_firewall`).

use std::ffi::OsString;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use windows_sys::Win32::Globalization::{CP_OEMCP, MultiByteToWideChar};

const NOME: &str = "Axon (captura)";
/// CREATE_NO_WINDOW: sem ele cada netsh pisca um console na abertura.
const SEM_JANELA: u32 = 0x0800_0000;

/// A regra deste exe já existe (criada numa abertura anterior): uma chamada ao netsh, que só lê.
/// Sem netsh ou com o serviço do firewall parado, responde que não, e o aviso aparece de novo.
pub fn liberada() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    netsh(&format!("show rule name=\"{NOME}\" verbose")).is_some_and(|saida| {
        saida.status.success() && regra_cobre(&texto_do_netsh(&saida.stdout, CP_OEMCP), &exe.display().to_string())
    })
}

/// Saída do netsh em texto. No Windows 11 ela vem em UTF-8 quando vai para um pipe (conferido em
/// 2026-10-03 com o console em 850 e em 65001). Se não for UTF-8 válido, lê na página OEM (850 no
/// Windows em pt-BR): com o acento lido errado, um caminho como "Área de Trabalho" nunca bateria e
/// o aviso voltaria em toda abertura. O schtasks (`inicio`) responde do mesmo jeito.
pub(crate) fn texto_do_netsh(bytes: &[u8], pagina_oem: u32) -> String {
    if let Ok(texto) = std::str::from_utf8(bytes) {
        return texto.to_string();
    }
    let Ok(tamanho) = i32::try_from(bytes.len()) else { return String::from_utf8_lossy(bytes).into_owned() };
    // A primeira chamada só mede; a segunda escreve em `largo`, do tamanho medido.
    let largura = unsafe { MultiByteToWideChar(pagina_oem, 0, bytes.as_ptr(), tamanho, std::ptr::null_mut(), 0) };
    if largura <= 0 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut largo = vec![0u16; largura as usize];
    let escritos = unsafe { MultiByteToWideChar(pagina_oem, 0, bytes.as_ptr(), tamanho, largo.as_mut_ptr(), largura) };
    String::from_utf16_lossy(&largo[..escritos.max(0) as usize])
}

/// O caminho sai como foi gravado, em qualquer idioma do Windows (o aviso do Windows pode gravar em
/// minúsculas).
fn regra_cobre(saida_do_show: &str, exe: &str) -> bool {
    saida_do_show.to_lowercase().contains(&exe.to_lowercase())
}

/// Só depois do clique em Liberar, e antes de abrir os sockets: a regra vale para o socket a partir
/// do bind. Apaga as regras de entrada deste exe (inclusive bloqueios do aviso do Windows) e as
/// "Axon (captura)" de outras pastas, e cria a liberação. Só entrada e só para este exe. Falha
/// calada: sem elevação (build de debug) ou com o serviço do firewall parado não há o que fazer, e
/// o rodapé avisa se a entrada continuar barrada.
pub fn liberar() {
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.display().to_string();
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

#[cfg(test)]
mod testes {
    use super::*;

    /// Saída real do `show rule ... verbose` num Windows em pt-BR (2026-10-02).
    const SHOW: &str = r"Nome da Regra:                        Axon (captura)
----------------------------------------------------------------------
Habilitado:                           Sim
Direção:                              Entrada
Perfis:                               Domínio,Particular,Público
Agrupamento:
LocalIP:                              Qualquer
RemoteIP:                             Qualquer
Protocolo:                            Qualquer
Travessia da borda:                   Não
Programa:                             C:\Users\Ronaldo Bueno\aion2-dps-meter\target\teste-031\release\Axon.exe
Tipos de Interface:                   Qualquer
Segurança:                            NotRequired
Origem da Regra:                      Configuração Local
Ação:                                 Permitir
Ok.
";

    #[test]
    fn regra_vale_so_para_o_exe_da_mesma_pasta() {
        let exe = r"C:\Users\Ronaldo Bueno\aion2-dps-meter\target\teste-031\release\Axon.exe";
        assert!(regra_cobre(SHOW, exe));
        // O aviso do Windows pode gravar o caminho em minúsculas.
        assert!(regra_cobre(SHOW, &exe.to_uppercase()));
        // Axon movido para outra pasta: a regra antiga não serve, o aviso aparece de novo.
        assert!(!regra_cobre(SHOW, r"C:\Users\Ronaldo Bueno\Downloads\Axon\Axon.exe"));
        assert!(!regra_cobre("Nenhuma regra corresponde aos critérios especificados.", exe));
    }

    #[test]
    fn caminho_com_acento_bate_em_utf8_e_na_pagina_850() {
        let exe = r"C:\Users\João\OneDrive\Área de Trabalho\Axon.exe";
        let linha = format!("Programa:                             {exe}\r\nOk.\r\n");
        assert!(regra_cobre(&texto_do_netsh(linha.as_bytes(), 850), exe));
        // A mesma linha na página 850 (ã = 0xC6, Á = 0xB5) não é UTF-8 válido.
        let oem: Vec<u8> = linha.chars().map(|c| match c { 'ã' => 0xC6, 'Á' => 0xB5, c => c as u8 }).collect();
        assert!(std::str::from_utf8(&oem).is_err());
        assert!(regra_cobre(&texto_do_netsh(&oem, 850), exe));
    }
}
