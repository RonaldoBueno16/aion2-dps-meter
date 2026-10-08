//! O `recordes.json` no disco. Lê com o tratamento de corrompido: o arquivo ilegível vai para
//! `.corrompido` antes de qualquer gravação, e nunca é sobrescrito. Grava por troca (`.tmp`,
//! `sync_all`, `rename`), relendo e mesclando o disco antes, para outra instância aberta não perder o
//! dela. Quem chama decide se pode gravar (nada no `--replay` nem com a opção desligada).

use std::ffi::OsString;
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nucleo::medicao::catalogo;
use nucleo::medicao::recordes::{self, Leitura, Recordes};

/// Sem conseguir gravar, a próxima tentativa sai depois disso (ou na próxima mudança).
pub const TENTAR_DE_NOVO: Duration = Duration::from_secs(30);

pub fn arquivo() -> PathBuf {
    catalogo::pasta_dados().join("recordes.json")
}

/// `recordes.json` → `recordes.json.tmp`.
fn ao_lado(caminho: &Path, sufixo: &str) -> PathBuf {
    let mut nome = OsString::from(caminho.as_os_str());
    nome.push(sufixo);
    PathBuf::from(nome)
}

pub struct Guardados {
    caminho: PathBuf,
    pub recordes: Recordes,
    /// Versão mais nova que esta, ou arquivo que não deu para ler nem mover: mostra, não grava.
    pub so_leitura: bool,
    /// Itens inválidos que saíram na leitura.
    pub removidos: usize,
    /// O arquivo estava ilegível e ficou guardado como `.corrompido`.
    pub corrompido: bool,
    /// Mudou na memória e não gravou: o erro e quando tentou.
    pub por_gravar: Option<(String, Instant)>,
}

/// O que o disco tem agora.
enum Disco {
    Ausente,
    Lido(Recordes, usize, bool),
    /// Ilegível, já movido para `.corrompido`.
    Movido,
}

impl Guardados {
    /// Arquivo ausente: vazio, sem criar nada (ele nasce no primeiro recorde).
    pub fn abrir(caminho: PathBuf) -> Self {
        let mut g = Guardados {
            caminho,
            recordes: Recordes::default(),
            so_leitura: false,
            removidos: 0,
            corrompido: false,
            por_gravar: None,
        };
        match g.ler_disco() {
            Ok(Disco::Ausente) => {}
            Ok(Disco::Lido(recordes, removidos, so_leitura)) => {
                g.recordes = recordes;
                g.removidos = removidos;
                g.so_leitura = so_leitura;
            }
            Ok(Disco::Movido) => g.corrompido = true,
            // Sem ler nem mover, gravar apagaria o que está lá.
            Err(erro) => {
                g.so_leitura = true;
                g.por_gravar = Some((erro, Instant::now()));
            }
        }
        g
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// A memória para o disco, com o que outra instância gravou no meio.
    pub fn gravar(&mut self) -> Result<(), String> {
        if !self.recarregar()? {
            return Ok(());
        }
        self.escrever()
    }

    /// Tira um (chefe, classe): relê e mescla antes, e só então tira (na ordem contrária, a mescla
    /// traria o apagado de volta).
    pub fn apagar(&mut self, npc: u32, classe: &str) -> Result<(), String> {
        if !self.recarregar()? {
            return Ok(());
        }
        self.recordes.apagar(npc, classe);
        self.escrever()
    }

    /// "Apagar todos": some o arquivo.
    pub fn apagar_todos(&mut self) -> Result<(), String> {
        if self.so_leitura {
            return Ok(());
        }
        match std::fs::remove_file(&self.caminho) {
            Ok(()) => {}
            Err(erro) if erro.kind() == ErrorKind::NotFound => {}
            Err(erro) => return Err(erro.to_string()),
        }
        self.recordes = Recordes::default();
        self.por_gravar = None;
        Ok(())
    }

    /// O que ficou por gravar, de `TENTAR_DE_NOVO` em `TENTAR_DE_NOVO`.
    pub fn tentar_de_novo(&mut self) {
        if !self.so_leitura && self.por_gravar.as_ref().is_some_and(|(_, quando)| quando.elapsed() >= TENTAR_DE_NOVO) {
            let _ = self.gravar();
        }
    }

    /// Mescla o disco na memória. false: não pode gravar (versão mais nova, agora ou antes).
    fn recarregar(&mut self) -> Result<bool, String> {
        if self.so_leitura {
            return Ok(false);
        }
        match self.ler_disco() {
            Ok(Disco::Ausente) => Ok(true),
            Ok(Disco::Lido(_, _, true)) => {
                self.so_leitura = true;
                Ok(false)
            }
            Ok(Disco::Lido(disco, _, false)) => {
                self.recordes.mesclar(&disco);
                Ok(true)
            }
            Ok(Disco::Movido) => {
                self.corrompido = true;
                Ok(true)
            }
            Err(erro) => {
                self.por_gravar = Some((erro.clone(), Instant::now()));
                Err(erro)
            }
        }
    }

    fn ler_disco(&self) -> Result<Disco, String> {
        let bytes = match std::fs::read(&self.caminho) {
            Ok(bytes) => bytes,
            Err(erro) if erro.kind() == ErrorKind::NotFound => return Ok(Disco::Ausente),
            Err(erro) => return Err(erro.to_string()),
        };
        match recordes::ler(&bytes) {
            Leitura::Lido { recordes, removidos, so_leitura } => Ok(Disco::Lido(recordes, removidos, so_leitura)),
            Leitura::Corrompido => {
                std::fs::rename(&self.caminho, ao_lado(&self.caminho, ".corrompido")).map_err(|e| e.to_string())?;
                Ok(Disco::Movido)
            }
        }
    }

    /// `.tmp` gravado e fechado antes do `rename` (no Windows, com o arquivo aberto a troca falha).
    fn escrever(&mut self) -> Result<(), String> {
        let tmp = ao_lado(&self.caminho, ".tmp");
        let resultado = (|| -> std::io::Result<()> {
            {
                let mut arquivo = File::create(&tmp)?;
                arquivo.write_all(recordes::texto(&self.recordes).as_bytes())?;
                arquivo.sync_all()?;
            }
            std::fs::rename(&tmp, &self.caminho)
        })();
        match resultado {
            Ok(()) => {
                self.por_gravar = None;
                Ok(())
            }
            Err(erro) => {
                self.por_gravar = Some((erro.to_string(), Instant::now()));
                Err(erro.to_string())
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use nucleo::medicao::recordes::{Candidato, MarcaDps, MarcaTempo};

    use super::*;

    /// Pasta própria por teste: os testes rodam em paralelo.
    fn pasta(nome: &str) -> PathBuf {
        let pasta = std::env::temp_dir().join(format!("axon-recordes-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    fn kill(npc: u32, dps: f64, ms: u64) -> Candidato {
        let data = 1_791_400_000;
        Candidato {
            npc,
            classe: "Ranger",
            data,
            jogadores: 4,
            dps: Ok(MarcaDps { valor: dps, data, ativo_ms: 100_000, jogadores: 4 }),
            tempo: Ok(MarcaTempo { ms, data, jogadores: 4, dps }),
        }
    }

    #[test]
    fn f1_ausente_fica_vazio_e_o_arquivo_nasce_no_primeiro_recorde() {
        let pasta = pasta("f1");
        let caminho = pasta.join("recordes.json");
        let mut g = Guardados::abrir(caminho.clone());
        assert!(g.recordes.lista.is_empty() && !g.corrompido && !g.so_leitura);
        assert!(!caminho.exists());
        g.recordes.registrar(&kill(2400425, 2000.0, 150_000));
        g.gravar().unwrap();
        assert_eq!(Guardados::abrir(caminho.clone()).recordes, g.recordes);
        assert!(!ao_lado(&caminho, ".tmp").exists());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn f2_f3_ilegivel_vai_para_corrompido_byte_a_byte_e_comeca_vazio() {
        let pasta = pasta("f2");
        let caminho = pasta.join("recordes.json");
        let mut grande = br#"{"versao":1,"recordes":[]}"#.to_vec();
        grande.resize(1024 * 1024 + 1, b' ');
        let ruins: [&[u8]; 5] = [br#"{"versao":1,"recordes":[{"npc":24"#, &[0xD1, 0x00, 0x7F, 0xFF], b"", "\u{feff}".as_bytes(), &grande];
        for ruim in ruins {
            std::fs::write(&caminho, ruim).unwrap();
            let mut g = Guardados::abrir(caminho.clone());
            assert!(g.corrompido && g.recordes.lista.is_empty() && !g.so_leitura);
            assert!(!caminho.exists());
            assert_eq!(std::fs::read(ao_lado(&caminho, ".corrompido")).unwrap(), ruim);
            // Gravar depois não toca no .corrompido.
            g.recordes.registrar(&kill(2400425, 2000.0, 150_000));
            g.gravar().unwrap();
            assert_eq!(std::fs::read(ao_lado(&caminho, ".corrompido")).unwrap(), ruim);
            std::fs::remove_file(&caminho).unwrap();
        }
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn f4_versao_mais_nova_nao_grava_nem_apaga() {
        let pasta = pasta("f4");
        let caminho = pasta.join("recordes.json");
        let futuro = r#"{"versao":7,"recordes":[{"npc":2400425,"classe":"Ranger","kills":1,"tempo":{"ms":151870,"data":1791487600,"jogadores":4,"dps":2204.8}}],"novo":true}"#;
        std::fs::write(&caminho, futuro).unwrap();
        let mut g = Guardados::abrir(caminho.clone());
        assert!(g.so_leitura);
        assert_eq!(g.recordes.lista.len(), 1);
        g.recordes.registrar(&kill(2400424, 10.0, 5_000));
        g.gravar().unwrap();
        g.apagar(2400425, "Ranger").unwrap();
        g.apagar_todos().unwrap();
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), futuro);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn f9_falha_de_gravacao_guarda_na_memoria_e_a_proxima_grava() {
        let pasta = pasta("f9");
        let caminho = pasta.join("recordes.json");
        let mut g = Guardados::abrir(caminho.clone());
        g.recordes.registrar(&kill(2400425, 2000.0, 150_000));
        g.gravar().unwrap();
        // Só leitura: a troca falha.
        let mut permissoes = std::fs::metadata(&caminho).unwrap().permissions();
        permissoes.set_readonly(true);
        std::fs::set_permissions(&caminho, permissoes.clone()).unwrap();
        g.recordes.registrar(&kill(2400424, 10.0, 5_000));
        assert!(g.gravar().is_err());
        assert!(g.por_gravar.is_some());
        assert!(g.recordes.melhor(2400424, "Ranger").is_some());

        #[allow(clippy::permissions_set_readonly_false)]
        permissoes.set_readonly(false);
        std::fs::set_permissions(&caminho, permissoes).unwrap();
        g.gravar().unwrap();
        assert!(g.por_gravar.is_none());
        assert!(Guardados::abrir(caminho.clone()).recordes.melhor(2400424, "Ranger").is_some());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn f10_tmp_pela_metade_e_ignorado_e_sobrescrito() {
        let pasta = pasta("f10");
        let caminho = pasta.join("recordes.json");
        let mut g = Guardados::abrir(caminho.clone());
        g.recordes.registrar(&kill(2400425, 2000.0, 150_000));
        g.gravar().unwrap();
        std::fs::write(ao_lado(&caminho, ".tmp"), br#"{"versao":1,"recor"#).unwrap();
        let mut g = Guardados::abrir(caminho.clone());
        assert_eq!(g.recordes.lista.len(), 1);
        g.recordes.registrar(&kill(2400424, 10.0, 5_000));
        g.gravar().unwrap();
        assert!(!ao_lado(&caminho, ".tmp").exists());
        assert_eq!(Guardados::abrir(caminho.clone()).recordes.lista.len(), 2);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn f12_duas_instancias_nao_perdem_o_recorde_uma_da_outra_e_apagar_nao_volta() {
        let pasta = pasta("f12");
        let caminho = pasta.join("recordes.json");
        let mut a = Guardados::abrir(caminho.clone());
        let mut b = Guardados::abrir(caminho.clone());
        b.recordes.registrar(&kill(2400425, 2000.0, 150_000));
        b.gravar().unwrap();
        a.recordes.registrar(&kill(2400424, 10.0, 5_000));
        a.gravar().unwrap();
        let disco = Guardados::abrir(caminho.clone()).recordes;
        assert!(disco.melhor(2400425, "Ranger").is_some() && disco.melhor(2400424, "Ranger").is_some());

        a.apagar(2400424, "Ranger").unwrap();
        a.gravar().unwrap();
        let disco = Guardados::abrir(caminho.clone()).recordes;
        assert!(disco.melhor(2400424, "Ranger").is_none() && disco.melhor(2400425, "Ranger").is_some());

        a.apagar_todos().unwrap();
        assert!(!caminho.exists() && a.recordes.lista.is_empty());
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
