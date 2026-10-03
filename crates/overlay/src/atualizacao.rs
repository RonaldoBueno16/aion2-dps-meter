//! Atualização pelo GitHub. Na abertura, uma consulta à última release (a API pública aceita 60 por
//! hora sem login), repetida quando o usuário clica em "Verificar atualização" no rodapé; com versão
//! nova, o rodapé mostra "v0.5.0 → v0.5.1" e o botão Atualizar. O
//! clique baixa o `Axon.exe` solto da release, confere tamanho, SHA-256 (o `digest` que a API
//! publica para cada asset) e o cabeçalho "MZ", e troca o exe em uso: o Windows deixa renomear um
//! exe rodando, então o atual vira `.old` e o novo toma o nome. O overlay então abre o novo e fecha;
//! o `.old` é apagado na abertura seguinte.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde_json::Value;
use windows_sys::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};

const ULTIMA: &str = "https://api.github.com/repos/RonaldoBueno16/aion2-dps-meter/releases/latest";
/// Só baixa daqui: a URL vem da resposta da API, e o processo é elevado.
const DOWNLOADS: &str = "https://github.com/RonaldoBueno16/aion2-dps-meter/releases/download/";
/// Asset com o exe solto (o zip continua para quem baixa pelo navegador).
const ASSET: &str = "Axon.exe";
const TAMANHO_MAXIMO: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Novidade {
    /// Sem o "v": "0.5.1".
    pub versao: String,
    url: String,
    tamanho: u64,
    sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq)]
pub enum Estado {
    /// Consulta da abertura em andamento, em dia, ou sem internet: o rodapé só mostra a versão.
    Nada,
    /// Consulta pedida pelo botão em andamento.
    Consultando,
    /// O botão consultou e esta já é a última versão.
    EmDia,
    /// O botão consultou e o GitHub não respondeu (sem internet, API fora do ar, limite por hora).
    SemResposta,
    Disponivel(Novidade),
    Baixando(Novidade),
    Falhou(Novidade, String),
    /// Exe novo no lugar: falta o overlay abrir o novo e fechar.
    Pronta(Novidade),
}

pub struct Atualizacao(Arc<Mutex<Estado>>);

impl Atualizacao {
    /// Apaga o exe trocado na atualização anterior e consulta o GitHub em segundo plano.
    pub fn iniciar() -> Self {
        if let Ok(exe) = std::env::current_exe() {
            // Ainda preso se o processo antigo não terminou de fechar: fica para a próxima.
            let _ = std::fs::remove_file(com_sufixo(&exe, ".old"));
        }
        let atualizacao = Self(Arc::new(Mutex::new(Estado::Nada)));
        atualizacao.consultar(false);
        atualizacao
    }

    /// Botão "Verificar atualização": a mesma consulta da abertura, sem reabrir o Axon.
    pub fn verificar(&self) {
        if matches!(self.estado(), Estado::Nada | Estado::EmDia | Estado::SemResposta) {
            *travar(&self.0) = Estado::Consultando;
            self.consultar(true);
        }
    }

    fn consultar(&self, pelo_botao: bool) {
        let compartilhado = self.0.clone();
        let fio = std::thread::Builder::new()
            .name("atualizacao".into())
            .spawn(move || *travar(&compartilhado) = depois_da_consulta(ultima_release(), pelo_botao));
        if fio.is_err() {
            // Sem a thread, o botão voltaria a aparecer só se o estado sair de Consultando.
            *travar(&self.0) = depois_da_consulta(Err(()), pelo_botao);
        }
    }

    /// Só no debug (--nova-versao): finge uma versão nova, para ver o rodapé; o download dá 404.
    pub fn falsa() -> Self {
        Self(Arc::new(Mutex::new(Estado::Disponivel(Novidade {
            versao: "9.9.9".into(),
            url: format!("{DOWNLOADS}v9.9.9/{ASSET}"),
            tamanho: 0,
            sha256: [0; 32],
        }))))
    }

    pub fn estado(&self) -> Estado {
        travar(&self.0).clone()
    }

    /// Baixa, confere e troca o exe numa thread; o rodapé acompanha pelo estado.
    pub fn atualizar(&self) {
        let novidade = match self.estado() {
            Estado::Disponivel(n) | Estado::Falhou(n, _) => n,
            _ => return,
        };
        *travar(&self.0) = Estado::Baixando(novidade.clone());
        let compartilhado = self.0.clone();
        let _ = std::thread::Builder::new().name("atualizacao".into()).spawn(move || {
            let fim = match instalar(&novidade) {
                Ok(()) => Estado::Pronta(novidade),
                Err(erro) => Estado::Falhou(novidade, erro),
            };
            *travar(&compartilhado) = fim;
        });
    }
}

/// Abre o exe novo, que já está no caminho do atual. O processo é elevado, então o filho também
/// nasce elevado, sem outro pedido de administrador.
pub fn abrir_novo() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).spawn().map(drop).map_err(|e| format!("abrir a versão nova: {e}"))
}

fn travar(estado: &Mutex<Estado>) -> MutexGuard<'_, Estado> {
    estado.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ok(None): a última release não é mais nova que esta (ou não tem o exe solto). Err: sem internet,
/// API fora do ar ou limite de consultas por hora.
fn ultima_release() -> Result<Option<Novidade>, ()> {
    let mut resposta = nucleo::medicao::catalogo::cliente_http()
        .get(ULTIMA)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(drop)?;
    let json = resposta.body_mut().read_to_string().map_err(drop)?;
    Ok(novidade(&json, env!("CARGO_PKG_VERSION")))
}

/// Na abertura, em dia ou sem internet não há o que mostrar: o rodapé fica só com a versão. Pelo
/// botão, o rodapé diz o resultado, para o clique não parecer ignorado.
fn depois_da_consulta(consulta: Result<Option<Novidade>, ()>, pelo_botao: bool) -> Estado {
    match consulta {
        Ok(Some(novidade)) => Estado::Disponivel(novidade),
        Ok(None) if pelo_botao => Estado::EmDia,
        Err(()) if pelo_botao => Estado::SemResposta,
        _ => Estado::Nada,
    }
}

/// A release, se for mais nova que `atual` e tiver o exe solto com o digest SHA-256.
fn novidade(json: &str, atual: &str) -> Option<Novidade> {
    let release: Value = serde_json::from_str(json).ok()?;
    let versao = release["tag_name"].as_str()?.trim_start_matches('v');
    if numeros(versao)? <= numeros(atual)? {
        return None;
    }
    let asset = release["assets"].as_array()?.iter().find(|a| a["name"] == ASSET)?;
    let url = asset["browser_download_url"].as_str()?;
    if !url.starts_with(DOWNLOADS) {
        return None;
    }
    Some(Novidade {
        versao: versao.to_string(),
        url: url.to_string(),
        tamanho: asset["size"].as_u64()?,
        sha256: hexadecimal(asset["digest"].as_str()?.strip_prefix("sha256:")?)?,
    })
}

/// "0.4.10" → (0, 4, 10). Sufixo de pré-release ("0.5.0-beta") não conta como versão nova.
fn numeros(versao: &str) -> Option<(u32, u32, u32)> {
    let mut partes = versao.split('.').map(|p| p.parse::<u32>().ok());
    let numeros = (partes.next()??, partes.next()??, partes.next()??);
    partes.next().is_none().then_some(numeros)
}

fn hexadecimal(texto: &str) -> Option<[u8; 32]> {
    if texto.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(texto.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(bytes)
}

fn instalar(novidade: &Novidade) -> Result<(), String> {
    let mut resposta = nucleo::medicao::catalogo::cliente_http()
        .get(&novidade.url)
        .call()
        .map_err(|e| format!("baixar: {e}"))?;
    let bytes = resposta
        .body_mut()
        .with_config()
        .limit(TAMANHO_MAXIMO)
        .read_to_vec()
        .map_err(|e| format!("baixar: {e}"))?;
    conferir(&bytes, novidade)?;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let novo = com_sufixo(&exe, ".novo");
    std::fs::write(&novo, &bytes).map_err(|e| format!("gravar a versão nova: {e}"))?;
    if cfg!(debug_assertions) {
        // Trocar poria a release no lugar do build de teste: o debug para no arquivo conferido.
        return Err(format!("debug: baixado e conferido em {}, sem troca", novo.display()));
    }
    trocar(&exe, &novo)
}

fn conferir(bytes: &[u8], novidade: &Novidade) -> Result<(), String> {
    if bytes.len() as u64 != novidade.tamanho {
        return Err(format!("download incompleto ({} de {} bytes)", bytes.len(), novidade.tamanho));
    }
    if sha256(bytes)? != novidade.sha256 {
        return Err("o arquivo baixado não confere com o da release".into());
    }
    if !bytes.starts_with(b"MZ") {
        return Err("o arquivo baixado não é um executável".into());
    }
    Ok(())
}

/// O atual vira `.old` e o novo toma o nome dele. Se o segundo passo falhar, o atual volta.
fn trocar(exe: &Path, novo: &Path) -> Result<(), String> {
    let antigo = com_sufixo(exe, ".old");
    let _ = std::fs::remove_file(&antigo);
    std::fs::rename(exe, &antigo).map_err(|e| format!("renomear o exe atual: {e}"))?;
    if let Err(erro) = std::fs::rename(novo, exe) {
        let _ = std::fs::rename(&antigo, exe);
        return Err(format!("pôr a versão nova no lugar: {erro}"));
    }
    Ok(())
}

fn com_sufixo(exe: &Path, sufixo: &str) -> PathBuf {
    let mut caminho = exe.as_os_str().to_owned();
    caminho.push(sufixo);
    PathBuf::from(caminho)
}

fn sha256(dados: &[u8]) -> Result<[u8; 32], String> {
    let tamanho = u32::try_from(dados.len()).map_err(|_| "arquivo grande demais".to_string())?;
    let mut saida = [0u8; 32];
    // BCRYPT_SHA256_ALG_HANDLE: pseudo-handle do Windows 10+, sem abrir provedor.
    let status = unsafe {
        BCryptHash(BCRYPT_SHA256_ALG_HANDLE, std::ptr::null(), 0, dados.as_ptr(), tamanho, saida.as_mut_ptr(), 32)
    };
    if status != 0 {
        return Err(format!("SHA-256 falhou ({status:#x})"));
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Trecho da resposta real de /releases/latest (v0.4.0), com um asset Axon.exe como os das
    /// próximas releases.
    const RELEASE: &str = r#"{"tag_name":"v0.4.10","assets":[
        {"name":"Aion2Meter-0.4.10-win-x64.zip","size":1244321,
         "digest":"sha256:674dcfe130c70408797b5892269728fc38dafc8a6a6ce252c6056accc890d6e1",
         "browser_download_url":"https://github.com/RonaldoBueno16/aion2-dps-meter/releases/download/v0.4.10/Aion2Meter-0.4.10-win-x64.zip"},
        {"name":"Axon.exe","size":2243584,
         "digest":"sha256:9375ad11ca25d6caacb6c2e863927db0f62031d162aed924e365bf8fe4c512f1",
         "browser_download_url":"https://github.com/RonaldoBueno16/aion2-dps-meter/releases/download/v0.4.10/Axon.exe"}]}"#;

    #[test]
    fn versao_nova_so_quando_maior_e_com_o_exe_solto() {
        let n = novidade(RELEASE, "0.4.0").expect("0.4.10 é mais nova que 0.4.0");
        assert_eq!(n.versao, "0.4.10");
        assert_eq!(n.tamanho, 2243584);
        assert_eq!(n.sha256[..2], [0x93, 0x75]);
        // 0.4.10 > 0.4.9 (número, não texto); igual ou mais velha: nada.
        assert!(novidade(RELEASE, "0.4.9").is_some());
        assert!(novidade(RELEASE, "0.4.10").is_none());
        assert!(novidade(RELEASE, "0.5.0").is_none());
        // Release sem o Axon.exe solto (as de até a 0.4.0): nada.
        assert!(novidade(&RELEASE.replace("\"Axon.exe\"", "\"outro.exe\""), "0.4.0").is_none());
        // URL fora das releases do repositório: nada.
        assert!(novidade(&RELEASE.replace("https://github.com/RonaldoBueno16", "https://exemplo.com/x"), "0.4.0").is_none());
        assert_eq!(numeros("0.5.0-beta"), None);
    }

    #[test]
    fn so_o_botao_mostra_em_dia_ou_sem_resposta() {
        let nova = novidade(RELEASE, "0.4.0").unwrap();
        for pelo_botao in [false, true] {
            assert_eq!(depois_da_consulta(Ok(Some(nova.clone())), pelo_botao), Estado::Disponivel(nova.clone()));
        }
        // Na abertura, em dia ou sem internet: só a versão no rodapé, como antes do botão.
        assert_eq!(depois_da_consulta(Ok(None), false), Estado::Nada);
        assert_eq!(depois_da_consulta(Err(()), false), Estado::Nada);
        assert_eq!(depois_da_consulta(Ok(None), true), Estado::EmDia);
        assert_eq!(depois_da_consulta(Err(()), true), Estado::SemResposta);
    }

    #[test]
    fn sha256_e_conferencia_do_download() {
        let abc = sha256(b"abc").unwrap();
        assert_eq!(abc, hexadecimal("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").unwrap());
        let exe = b"MZ resto do exe";
        let certo = Novidade { versao: "1.0.0".into(), url: String::new(), tamanho: exe.len() as u64, sha256: sha256(exe).unwrap() };
        assert_eq!(conferir(exe, &certo), Ok(()));
        assert!(conferir(&exe[..5], &certo).unwrap_err().contains("incompleto"));
        let trocado = b"MZ resto do exf";
        assert!(conferir(trocado, &certo).unwrap_err().contains("não confere"));
    }

    #[test]
    fn troca_o_exe_e_volta_o_antigo_se_falhar() {
        let pasta = std::env::temp_dir().join(format!("axon-troca-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        let exe = pasta.join("Axon.exe");
        let novo = com_sufixo(&exe, ".novo");

        std::fs::write(&exe, "velho").unwrap();
        std::fs::write(&novo, "novo").unwrap();
        trocar(&exe, &novo).unwrap();
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "novo");
        assert_eq!(std::fs::read_to_string(com_sufixo(&exe, ".old")).unwrap(), "velho");

        // Sem o .novo, o segundo passo falha e o exe atual volta para o lugar.
        assert!(trocar(&exe, &novo).is_err());
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "novo");
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// Com rede: consulta a API de verdade e confere o zip da última release pelo digest.
    #[test]
    #[ignore]
    fn baixa_e_confere_o_asset_da_ultima_release() {
        let mut resposta = nucleo::medicao::catalogo::cliente_http().get(ULTIMA).call().unwrap();
        let release: Value = serde_json::from_str(&resposta.body_mut().read_to_string().unwrap()).unwrap();
        let asset = &release["assets"][0];
        let alvo = Novidade {
            versao: String::new(),
            url: asset["browser_download_url"].as_str().unwrap().to_string(),
            tamanho: asset["size"].as_u64().unwrap(),
            sha256: hexadecimal(asset["digest"].as_str().unwrap().strip_prefix("sha256:").unwrap()).unwrap(),
        };
        let bytes = nucleo::medicao::catalogo::cliente_http()
            .get(&alvo.url)
            .call()
            .unwrap()
            .body_mut()
            .with_config()
            .limit(TAMANHO_MAXIMO)
            .read_to_vec()
            .unwrap();
        assert_eq!(bytes.len() as u64, alvo.tamanho);
        assert_eq!(sha256(&bytes).unwrap(), alvo.sha256);
    }
}
