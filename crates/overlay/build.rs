// Raw socket (SIO_RCVALL) só funciona elevado: o Windows pede UAC ao abrir o executável de
// release. O build de debug abre sem elevação, para testar a janela com --replay.
use std::path::PathBuf;

use embed_manifest::manifest::ExecutionLevel;
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    let nivel = if std::env::var("PROFILE").as_deref() == Ok("release") {
        ExecutionLevel::RequireAdministrator
    } else {
        ExecutionLevel::AsInvoker
    };
    embed_manifest(new_manifest("Axon").requested_execution_level(nivel)).expect("manifesto do Windows");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        embutir_icone();
    }
    println!("cargo:rerun-if-changed=build.rs");
}

/// Ícone do exe (Explorer, bandeja, Alt+Tab): o assets/axon.ico vira um .res que o link.exe
/// embute. No MSVC o manifesto entra pelo /MANIFEST:EMBED, então os dois não colidem.
fn embutir_icone() {
    let pasta = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ico = std::fs::read(pasta.join("assets").join("axon.ico")).expect("assets/axon.ico");
    let res = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("icone.res");
    std::fs::write(&res, recurso_de_icone(&ico)).expect("icone.res");
    println!("cargo:rustc-link-arg-bins={}", res.display());
    println!("cargo:rerun-if-changed=assets/axon.ico");
}

/// Cada imagem do .ico vira um RT_ICON (ids 1..n) e o diretório vira o RT_GROUP_ICON 1, com o
/// deslocamento no arquivo trocado pelo id da imagem.
fn recurso_de_icone(ico: &[u8]) -> Vec<u8> {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    let u32_em = |i: usize| u32::from_le_bytes([ico[i], ico[i + 1], ico[i + 2], ico[i + 3]]);
    let quantas = u16::from_le_bytes([ico[4], ico[5]]);

    let mut res = Vec::new();
    // Um .res começa com um recurso vazio.
    recurso(&mut res, 0, 0, 0, &[]);
    let mut grupo = ico[..6].to_vec();
    for k in 0..quantas {
        let entrada = 6 + 16 * usize::from(k);
        let (tamanho, inicio) = (u32_em(entrada + 8) as usize, u32_em(entrada + 12) as usize);
        recurso(&mut res, RT_ICON, k + 1, 0x1010, &ico[inicio..inicio + tamanho]);
        grupo.extend_from_slice(&ico[entrada..entrada + 12]);
        grupo.extend_from_slice(&(k + 1).to_le_bytes());
    }
    recurso(&mut res, RT_GROUP_ICON, 1, 0x1030, &grupo);
    res
}

/// Cabeçalho de 32 bytes (tipo e nome por número, idioma neutro) e os dados alinhados em 4.
fn recurso(res: &mut Vec<u8>, tipo: u16, nome: u16, opcoes: u16, dados: &[u8]) {
    res.extend_from_slice(&(dados.len() as u32).to_le_bytes());
    res.extend_from_slice(&32_u32.to_le_bytes());
    for numero in [tipo, nome] {
        res.extend_from_slice(&[0xFF, 0xFF]);
        res.extend_from_slice(&numero.to_le_bytes());
    }
    res.extend_from_slice(&0_u32.to_le_bytes()); // DataVersion
    res.extend_from_slice(&opcoes.to_le_bytes()); // MemoryFlags
    res.extend_from_slice(&0_u16.to_le_bytes()); // LanguageId
    res.extend_from_slice(&0_u32.to_le_bytes()); // Version
    res.extend_from_slice(&0_u32.to_le_bytes()); // Characteristics
    res.extend_from_slice(dados);
    res.resize(res.len().next_multiple_of(4), 0);
}
