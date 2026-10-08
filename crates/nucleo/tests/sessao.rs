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

#[test]
fn ping_pelo_ack_do_servidor_no_fluxo_travado() {
    let mut sessao = Sessao::default();
    let mut jogo = Conexao::nova("193.202.112.171", 62225, 13328);
    let mut t = T0;
    for _ in 0..3 {
        sessao.ao_segmento(&jogo.dados(HEARTBEAT), passar(&mut t, 50));
    }
    assert_eq!(sessao.ping(), None);

    // O PC manda 20 bytes; o servidor confirma com um ACK puro (sem dados) 15 ms depois.
    let envio = SegmentoTcp::novo(CLIENTE, 62225, jogo.servidor, 13328, 7_000, false, vec![0; 20]);
    sessao.ao_segmento(&envio, passar(&mut t, 50));
    let mut ack = SegmentoTcp::novo(jogo.servidor, 13328, CLIENTE, 62225, jogo.seq, false, Vec::new());
    ack.ack = Some(7_020);
    sessao.ao_segmento(&ack, passar(&mut t, 15));
    assert_eq!(sessao.ping(), Some(15 * 10_000));

    // Envio de outro programa para o mesmo servidor não entra.
    let outro = SegmentoTcp::novo(CLIENTE, 50123, jogo.servidor, 13328, 9_000, false, vec![0; 20]);
    sessao.ao_segmento(&outro, passar(&mut t, 1));
    let mut ack = SegmentoTcp::novo(jogo.servidor, 13328, CLIENTE, 62225, jogo.seq, false, Vec::new());
    ack.ack = Some(9_020);
    sessao.ao_segmento(&ack, passar(&mut t, 1));
    assert_eq!(sessao.ping(), Some(15 * 10_000));
}

/// A única morte de jogador das capturas (world boss de 2026-10-03): o #16201 morto pelo #21799
/// (NPC 2400425), com o #16201 no papel de você. Os 19 pacotes de -4,65 s a 0 s, cada um na sua hora.
const MORTE_16201: [(f64, &str); 19] = [
    (-4.650, "13008dc97e0201008613000000000000"),
    (-4.451, "210438c97e0400a7aa0164d912005e021beb5c0701000000904ed2120100"),
    (-4.451, "0a218dc97e0001"),
    (-4.451, "13008dc97e020100340a000000000000"),
    (-4.201, "200438c97e0400c97e9040150148024b384d6c01000000a0519a0a0100"),
    (-4.201, "13008dc97e0201004e0f000000000000"),
    (-3.501, "190538c97e0bc97ed3095fb2fc0bcf02cd02ddaf1e00"),
    (-3.501, "170538c97e0bc97ed30960b2fc0b7e7bddaf1e00"),
    (-3.501, "13008dc97e0201001611000000000000"),
    (-1.502, "180538c97e0bc97ed3095fb2fc0b02cd02ddaf1e00"),
    (-1.502, "170538c97e0bc97ed30960b2fc0b037bddaf1e00"),
    (-1.502, "13008dc97e020100de12000000000000"),
    (-1.450, "0a218dc97e0000"),
    (-0.451, "13008dc97e020100f212000000000000"),
    (-0.040, "210438c97e0400a7aa0182d912005f02d3f65c0701000000904ebe260100"),
    (-0.040, "0a218dc97e0001"),
    (-0.040, "13008dc97e0201000000000000000000"),
    (0.000, "1f048dc97e82d91200a7aa01000000000000a9c19201000000000102"),
    (0.000, "0a218dc97e0000"),
];

fn npc_de_teste(codigo: u32) -> Option<nucleo::medicao::catalogo::InfoNpc> {
    Some(nucleo::medicao::catalogo::InfoNpc {
        nome: format!("NPC {codigo}"),
        nivel: 45,
        nomeado: codigo == 2400425,
        tipo: String::new(),
        retrato: None,
    })
}

#[test]
fn morte_real_vira_relatorio_com_golpe_final_hp_e_cura() {
    use nucleo::medicao::medidor::TipoRecebido::{Cura, Efeito, Golpe};

    let mut sessao = Sessao::default();
    let mut jogo = Conexao::nova("193.202.112.171", 62225, 13328);
    let mut t = T0;
    for _ in 0..3 {
        sessao.ao_segmento(&jogo.dados(HEARTBEAT), passar(&mut t, 50));
    }
    sessao.medidor.consultar_npc = npc_de_teste;
    sessao.medidor.definir_jogador(16201, "Fulano", 0, true);
    let morte = t + segundos(5.0);
    for (antes, pacote) in MORTE_16201 {
        sessao.ao_segmento(&jogo.dados(pacote), morte + segundos(antes));
    }

    let r = sessao.medidor.ultima_morte().expect("relatório da morte");
    assert_eq!((r.morto, r.matador, r.npc_matador, r.skill_final), (16201, 21799, 2400425, 1235330));
    assert!(!r.matador_jogador);
    // O spawn do #21799 não está nos pacotes: o nome vem do código que o 0x8D04 traz.
    assert_eq!(r.nome_matador, "NPC 2400425");
    assert_eq!((r.dano_final, r.hp_antes_final), (Some(4926), Some(4850)));
    let tipos: Vec<_> = r.linhas.iter().map(|l| l.tipo).collect();
    assert_eq!(tipos, [Golpe, Cura, Efeito, Efeito, Efeito, Efeito, Golpe]);
    let valores: Vec<_> = r.linhas.iter().map(|l| l.valor).collect();
    assert_eq!(valores, [Some(2386), Some(1306), None, None, None, None, Some(4926)]);
    let hp: Vec<_> = r.linhas.iter().map(|l| l.hp_depois).collect();
    assert_eq!(hp, [2612, 3918, 4374, 4374, 4830, 4830, 0].map(Some));
    let finais: Vec<_> = r.linhas.iter().map(|l| l.golpe_final).collect();
    assert_eq!(finais, [false, false, false, false, false, false, true]);
    assert_eq!((r.linhas[0].quem.as_str(), r.linhas[1].quem.as_str()), ("NPC 2400425", "você"));
    assert!((r.linhas[0].antes_s + 4.451).abs() < 0.001);
    assert_eq!((r.recebido, r.maior, r.monstros, r.curado), (7312, 4926, 1, 1306));
    // A luta em andamento leva a morte no placar.
    assert_eq!(sessao.medidor.obter_placar().mortes.len(), 1);
}
