//! Início junto com o Windows pela tarefa "Axon" do Agendador de Tarefas. O exe pede administrador
//! (a captura precisa), e o Windows ignora programa que pede administrador na chave Run; a tarefa
//! com o privilégio mais alto abre o Axon no logon sem perguntar de novo. Ela é criada quando você
//! liga a opção e apagada quando desliga. O estado vem da própria tarefa, não do config.json.

use std::ffi::OsString;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use nucleo::medicao::catalogo;
use windows_sys::Win32::Globalization::CP_OEMCP;

pub const TAREFA: &str = "Axon";
/// O argumento da tarefa: abre escondido, esperando o jogo.
pub const SEGUNDO_PLANO: &str = "--segundo-plano";
/// CREATE_NO_WINDOW: sem ele cada schtasks pisca um console.
const SEM_JANELA: u32 = 0x0800_0000;

#[derive(Clone, Debug, PartialEq)]
pub enum Estado {
    Desligado,
    /// A tarefa abre este exe.
    Ligado,
    /// A tarefa abre outro exe (outra pasta, outra cópia em Downloads).
    OutroExe(PathBuf),
}

/// Uma consulta ao schtasks, que só lê. Sem a tarefa (ou sem o schtasks), desligado.
pub fn estado() -> Estado {
    let Ok(exe) = std::env::current_exe() else { return Estado::Desligado };
    let Some(saida) = schtasks(&["/Query", "/TN", TAREFA, "/XML"]).filter(|s| s.status.success()) else {
        return Estado::Desligado;
    };
    match comando_da_tarefa(&crate::firewall::texto_do_netsh(&saida.stdout, CP_OEMCP)) {
        Some(comando) if mesmo_exe(&comando, &exe) => Estado::Ligado,
        Some(comando) => Estado::OutroExe(comando),
        None => Estado::Desligado,
    }
}

/// Cria (ou troca) a tarefa para este exe. O XML vai num arquivo temporário na pasta do Axon,
/// apagado logo depois. Err com o texto para a tela.
pub fn ligar() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("não achei o exe: {e}"))?;
    let usuario = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(dominio), Ok(nome)) => format!("{dominio}\\{nome}"),
        _ => return Err("não achei o seu usuário do Windows".into()),
    };
    let pasta = catalogo::pasta_dados();
    let _ = std::fs::create_dir_all(&pasta);
    let arquivo = pasta.join("tarefa-axon.xml");
    // UTF-16 com BOM: o schtasks lê o XML como o próprio cabeçalho diz.
    let bytes: Vec<u8> = [0xFEFF].into_iter().chain(xml(&exe, &usuario).encode_utf16()).flat_map(u16::to_le_bytes).collect();
    std::fs::write(&arquivo, bytes).map_err(|e| format!("não deu para gravar o XML: {e}"))?;
    let caminho = arquivo.display().to_string();
    let saida = schtasks(&["/Create", "/TN", TAREFA, "/XML", &caminho, "/F"]);
    let _ = std::fs::remove_file(&arquivo);
    resultado(saida)
}

pub fn desligar() -> Result<(), String> {
    resultado(schtasks(&["/Delete", "/TN", TAREFA, "/F"]))
}

fn resultado(saida: Option<Output>) -> Result<(), String> {
    match saida {
        Some(s) if s.status.success() => Ok(()),
        Some(s) => {
            let erro = crate::firewall::texto_do_netsh(&s.stderr, CP_OEMCP);
            let erro = erro.trim().trim_start_matches("ERRO:").trim_start_matches("ERROR:").trim();
            Err(if erro.is_empty() { format!("o schtasks saiu com {}", s.status) } else { erro.to_string() })
        }
        None => Err("o schtasks não abriu".into()),
    }
}

/// Caminho absoluto: o processo é elevado e não deve achar um schtasks.exe qualquer na pasta do Axon.
fn schtasks(argumentos: &[&str]) -> Option<Output> {
    let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| OsString::from(r"C:\Windows"));
    Command::new(PathBuf::from(windows).join(r"System32\schtasks.exe"))
        .args(argumentos)
        .creation_flags(SEM_JANELA)
        .stdin(Stdio::null())
        .output()
        .ok()
}

/// O caminho no Windows não diferencia maiúsculas.
fn mesmo_exe(comando: &Path, exe: &Path) -> bool {
    comando.to_string_lossy().to_lowercase() == exe.to_string_lossy().to_lowercase()
}

/// O `<Command>` da consulta `/XML`, sem o escape do XML.
fn comando_da_tarefa(xml: &str) -> Option<PathBuf> {
    let inicio = xml.find("<Command>")? + "<Command>".len();
    let fim = inicio + xml[inicio..].find("</Command>")?;
    let comando = desescapar(xml[inicio..fim].trim());
    (!comando.is_empty()).then(|| PathBuf::from(comando))
}

fn escapar(texto: &str) -> String {
    texto.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// `&amp;` por último: `&amp;lt;` é o texto "&lt;", não "<".
fn desescapar(texto: &str) -> String {
    texto.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// A tarefa: no logon deste usuário, 15 s depois; com o privilégio mais alto (sem UAC); sem o limite
/// de 72 h do padrão, que derrubaria o Axon; na bateria também; uma instância só; prioridade normal
/// (a do padrão, 7, é abaixo do normal).
fn xml(exe: &Path, usuario: &str) -> String {
    let comando = escapar(&exe.display().to_string());
    let pasta = escapar(&exe.parent().map(|p| p.display().to_string()).unwrap_or_default());
    let usuario = escapar(usuario);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Abre o Axon escondido no logon; ele aparece quando o AION 2 abre. Desligue em Configurações › Sistema do Axon.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{usuario}</UserId>
      <Delay>PT15S</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{usuario}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{comando}</Command>
      <Arguments>{SEGUNDO_PLANO}</Arguments>
      <WorkingDirectory>{pasta}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn xml_da_tarefa_sem_limite_de_tempo_no_logon_do_usuario_e_com_o_caminho_escapado() {
        let exe = Path::new(r"C:\Jogos & Cia\<Axon>\Axon.exe");
        let x = xml(exe, "PC-SALA\\Fulano");
        assert!(x.contains(r"<Command>C:\Jogos &amp; Cia\&lt;Axon&gt;\Axon.exe</Command>"));
        assert!(x.contains(r"<WorkingDirectory>C:\Jogos &amp; Cia\&lt;Axon&gt;</WorkingDirectory>"));
        assert!(x.contains("<Arguments>--segundo-plano</Arguments>"));
        assert_eq!(x.matches(r"<UserId>PC-SALA\Fulano</UserId>").count(), 2);
        assert!(x.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(x.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(x.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(x.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(x.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
        // O que o schtasks devolve na consulta volta ao caminho de antes.
        assert_eq!(comando_da_tarefa(&x).as_deref(), Some(exe));
    }

    #[test]
    fn comando_da_consulta_e_o_exe_certo_sem_diferenciar_maiusculas() {
        // Trecho do formato do `schtasks /Query /XML` (o CRLF duplo vem dele).
        let consulta = "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\r\n<Task>\r\r\n  <Actions Context=\"Author\">\r\r\n    \
                        <Exec>\r\r\n      <Command>C:\\Users\\Fulano\\Downloads\\Axon.exe</Command>\r\r\n      \
                        <Arguments>--segundo-plano</Arguments>\r\r\n    </Exec>\r\r\n  </Actions>\r\r\n</Task>";
        let comando = comando_da_tarefa(consulta).unwrap();
        assert!(mesmo_exe(&comando, Path::new(r"c:\users\fulano\downloads\AXON.exe")));
        assert!(!mesmo_exe(&comando, Path::new(r"C:\Users\Fulano\Downloads\Axon (1).exe")));
        assert_eq!(comando_da_tarefa("<Task><Command></Command></Task>"), None);
        assert_eq!(comando_da_tarefa("<Task></Task>"), None);
        assert_eq!(desescapar("&amp;lt;"), "&lt;");
    }
}
