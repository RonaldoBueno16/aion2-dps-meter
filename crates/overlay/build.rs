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
        embutir_recursos();
    }
    println!("cargo:rerun-if-changed=build.rs");
}

/// Ícone do exe (Explorer, bandeja, Alt+Tab) e dados de versão (Propriedades > Detalhes, aviso do
/// UAC): o assets/axon.ico e a versão do Cargo.toml viram um .res que o link.exe embute. No MSVC o
/// manifesto entra pelo /MANIFEST:EMBED, então os dois não colidem.
fn embutir_recursos() {
    const RT_VERSION: u16 = 16;
    let pasta = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ico = std::fs::read(pasta.join("assets").join("axon.ico")).expect("assets/axon.ico");
    let mut recursos = recurso_de_icone(&ico);
    recurso(&mut recursos, RT_VERSION, 1, 0x0030, &recurso_de_versao());
    let res = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("recursos.res");
    std::fs::write(&res, recursos).expect("recursos.res");
    println!("cargo:rustc-link-arg-bins={}", res.display());
    println!("cargo:rerun-if-changed=assets/axon.ico");
}

/// VS_VERSIONINFO com os textos em pt-BR (0x0416, Unicode 0x04B0). Exe sem esses dados (empresa,
/// descrição, versão em branco) é um dos sinais que antivírus por heurística pesam contra.
fn recurso_de_versao() -> Vec<u8> {
    let numero = |nome: &str| std::env::var(nome).ok().and_then(|v| v.parse::<u32>().ok()).expect(nome);
    let (maior, menor, patch) =
        (numero("CARGO_PKG_VERSION_MAJOR"), numero("CARGO_PKG_VERSION_MINOR"), numero("CARGO_PKG_VERSION_PATCH"));
    let versao = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");

    // VS_FIXEDFILEINFO: assinatura, versão da estrutura, versão do arquivo e do produto (iguais),
    // máscara de flags, flags, VOS_NT_WINDOWS32, VFT_APP, subtipo e data (zerados).
    let fixo: Vec<u8> =
        [0xFEEF_04BD, 0x0001_0000, maior << 16 | menor, patch << 16, maior << 16 | menor, patch << 16, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0]
            .iter()
            .flat_map(|d: &u32| d.to_le_bytes())
            .collect();

    let textos = [
        ("CompanyName", "Ronaldo Bueno"),
        ("FileDescription", "Axon, medidor de DPS para AION 2"),
        ("FileVersion", versao.as_str()),
        ("InternalName", "Axon"),
        ("LegalCopyright", "© 2026 Ronaldo Bueno, licença MIT"),
        ("OriginalFilename", "Axon.exe"),
        ("ProductName", "Axon"),
        ("ProductVersion", versao.as_str()),
    ];
    let strings: Vec<Vec<u8>> = textos.iter().map(|(chave, valor)| bloco_de_texto(chave, valor)).collect();
    // A chave da tabela tem de bater com o par em Translation; senão o Windows mostra tudo vazio.
    let tabela = bloco("041604B0", 0, 1, &[], &strings);
    let string_file_info = bloco("StringFileInfo", 0, 1, &[], &[tabela]);
    let traducao: Vec<u8> = [0x0416_u16, 0x04B0].iter().flat_map(|p| p.to_le_bytes()).collect();
    let var = bloco("Translation", 4, 0, &traducao, &[]);
    let var_file_info = bloco("VarFileInfo", 0, 1, &[], &[var]);
    bloco("VS_VERSION_INFO", 52, 0, &fixo, &[string_file_info, var_file_info])
}

/// String: o valor é texto UTF-16 com o nulo, e o tamanho do valor conta em caracteres.
fn bloco_de_texto(chave: &str, valor: &str) -> Vec<u8> {
    let texto: Vec<u16> = valor.encode_utf16().chain([0]).collect();
    let bytes: Vec<u8> = texto.iter().flat_map(|u| u.to_le_bytes()).collect();
    bloco(chave, texto.len() as u16, 1, &bytes, &[])
}

/// Bloco do VS_VERSIONINFO: tamanho, tamanho do valor, tipo (0 binário, 1 texto), chave UTF-16 com
/// nulo, valor e filhos, cada um começando em múltiplo de 4. O tamanho não conta o preenchimento
/// depois do último item; quem acrescenta o próximo bloco é que alinha.
fn bloco(chave: &str, tamanho_do_valor: u16, tipo: u16, valor: &[u8], filhos: &[Vec<u8>]) -> Vec<u8> {
    let alinhar = |b: &mut Vec<u8>| b.resize(b.len().next_multiple_of(4), 0);
    let mut b = vec![0; 2];
    b.extend_from_slice(&tamanho_do_valor.to_le_bytes());
    b.extend_from_slice(&tipo.to_le_bytes());
    b.extend(chave.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    if !valor.is_empty() {
        alinhar(&mut b);
        b.extend_from_slice(valor);
    }
    for filho in filhos {
        alinhar(&mut b);
        b.extend_from_slice(filho);
    }
    let tamanho = b.len() as u16;
    b[..2].copy_from_slice(&tamanho.to_le_bytes());
    b
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
