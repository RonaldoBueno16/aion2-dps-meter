//! Troca de servidor: a conexão nova assume sozinha e o começo dela (seu personagem) é lido.

mod comum;

use std::net::Ipv4Addr;

use comum::{T0, hex, segundos};
use nucleo::Hora;
use nucleo::captura::segmento::SegmentoTcp;
use nucleo::medicao::sessao::Sessao;

const CLIENTE: Ipv4Addr = Ipv4Addr::new(192, 168, 0, 2);

const HEARTBEAT: &str = "0E0036942EC2FAA0010000";
// 0x3633 real do login, com o id trocado para 11174 (o do golpe) e o tamanho ajustado para 39 bytes.
const LOGIN: &str = "2A3336A6575FA1C1283705596F73686961090F000000021F00000063010000630100001F000000";
const GOLPE: &str = "210438CBE7020400A657575FE1003002073E095801000000AC57E8010100"; // 11174 → mob 46027

struct Conexao {
    servidor: Ipv4Addr,
    porta_local: u16,
    porta_servidor: u16,
    seq: u32,
}

impl Conexao {
    fn nova(servidor: &str, porta_local: u16, porta_servidor: u16) -> Self {
        Self { servidor: servidor.parse().unwrap(), porta_local, porta_servidor, seq: 1000 }
    }

    fn chave(&self) -> String {
        format!("{}:{} > {CLIENTE}:{}", self.servidor, self.porta_servidor, self.porta_local)
    }

    fn syn(&self) -> SegmentoTcp {
        SegmentoTcp::novo(self.servidor, self.porta_servidor, CLIENTE, self.porta_local, self.seq - 1, true, Vec::new())
    }

    fn dados(&mut self, texto: &str) -> SegmentoTcp {
        let dados = hex(texto);
        let seq = self.seq;
        self.seq += dados.len() as u32;
        SegmentoTcp::novo(self.servidor, self.porta_servidor, CLIENTE, self.porta_local, seq, false, dados)
    }
}

/// Avança o relógio e devolve a hora nova.
fn passar(t: &mut Hora, milissegundos: i64) -> Hora {
    *t += milissegundos * 10_000;
    *t
}

#[test]
fn conexao_nova_assume_mesmo_com_a_antiga_viva_e_le_o_login() {
    let mut sessao = Sessao::default();
    let mut antiga = Conexao::nova("193.202.112.171", 62225, 13328);
    let mut nova = Conexao::nova("193.202.112.195", 50000, 13328);
    let mut t = T0;

    for _ in 0..3 {
        sessao.ao_segmento(&antiga.dados(HEARTBEAT), passar(&mut t, 50));
    }
    sessao.ao_segmento(&antiga.dados(GOLPE), passar(&mut t, 50)); // id da sessão velha
    assert_eq!(sessao.fluxo.as_deref(), Some(antiga.chave().as_str()));

    // O login chega antes de a conexão nova ser reconhecida, e a antiga segue mandando heartbeat.
    sessao.ao_segmento(&nova.syn(), passar(&mut t, 10));
    sessao.ao_segmento(&nova.dados(LOGIN), passar(&mut t, 10));
    for _ in 0..3 {
        sessao.ao_segmento(&antiga.dados(HEARTBEAT), passar(&mut t, 25));
        sessao.ao_segmento(&nova.dados(HEARTBEAT), passar(&mut t, 25));
    }
    sessao.ao_segmento(&nova.dados(GOLPE), passar(&mut t, 50));
    sessao.ao_segmento(&antiga.dados(HEARTBEAT), passar(&mut t, 25)); // não volta para a antiga

    assert_eq!(sessao.fluxo.as_deref(), Some(nova.chave().as_str()));
    let p = sessao.medidor.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1); // nada da sessão velha
    let yoshi = &p.dano.jogadores[0];
    assert_eq!(yoshi.nome, "Yoshi");
    assert!(yoshi.voce);
    assert_eq!(yoshi.nivel, 31);
    assert_eq!(yoshi.poder, 355);
    assert_eq!(yoshi.golpes, 1);
}

#[test]
fn conexao_nova_de_outro_programa_nao_rouba_o_fluxo() {
    let mut sessao = Sessao::default();
    let mut jogo = Conexao::nova("193.202.112.171", 62225, 13328);
    let mut outro_programa = Conexao::nova("10.0.0.9", 50001, 27015); // por acaso com 0E 00 36 nos dados
    let mut t = T0;

    for _ in 0..3 {
        sessao.ao_segmento(&jogo.dados(HEARTBEAT), passar(&mut t, 50));
    }
    sessao.ao_segmento(&jogo.dados(GOLPE), passar(&mut t, 50));
    sessao.ao_segmento(&outro_programa.syn(), passar(&mut t, 10));
    for _ in 0..3 {
        sessao.ao_segmento(&jogo.dados(HEARTBEAT), passar(&mut t, 25));
        sessao.ao_segmento(&outro_programa.dados(HEARTBEAT), passar(&mut t, 25));
    }

    assert_eq!(sessao.fluxo.as_deref(), Some(jogo.chave().as_str()));
    let p = sessao.medidor.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    assert_eq!(p.dano.jogadores[0].golpes, 1); // luta intacta
}

#[test]
fn fluxo_sem_syn_so_assume_depois_de_5s_de_silencio_do_atual() {
    let mut sessao = Sessao::default();
    let mut atual = Conexao::nova("193.202.112.171", 62225, 13328);
    let mut outra = Conexao::nova("193.202.112.195", 50000, 13328); // já existia quando o medidor abriu
    let mut t = T0;

    for _ in 0..3 {
        sessao.ao_segmento(&atual.dados(HEARTBEAT), passar(&mut t, 50));
    }
    for _ in 0..3 {
        sessao.ao_segmento(&outra.dados(HEARTBEAT), passar(&mut t, 50));
    }
    assert_eq!(sessao.fluxo.as_deref(), Some(atual.chave().as_str()));

    sessao.ao_segmento(&outra.dados(HEARTBEAT), t + segundos(6.0));
    assert_eq!(sessao.fluxo.as_deref(), Some(outra.chave().as_str()));
}

#[test]
fn download_https_com_heartbeat_por_acaso_nao_vira_o_servidor_do_jogo() {
    let mut sessao = Sessao::default();
    let mut download = Conexao::nova("140.82.112.4", 51000, 443);
    let mut jogo = Conexao::nova("193.202.112.171", 62225, 13328);
    let mut t = T0;

    // Patch baixado por HTTPS: 0E 00 36 aparece por acaso a cada ~16 MB.
    for _ in 0..5 {
        sessao.ao_segmento(&download.dados(HEARTBEAT), passar(&mut t, 100));
    }
    assert_eq!(sessao.fluxo, None);

    for _ in 0..3 {
        sessao.ao_segmento(&jogo.dados(HEARTBEAT), passar(&mut t, 50));
    }
    assert_eq!(sessao.fluxo.as_deref(), Some(jogo.chave().as_str()));
}
