//! Pacotes reais da captura de 2026-10-01 (cliente Global, depois do patch das 19:10 UTC).

mod comum;

use comum::hex;
use nucleo::protocolo::combate;

#[test]
fn dano_seletor_4() {
    let e = combate::dano(&hex("210438CBE7020400A657575FE1003002073E095801000000AC57E8010100")).unwrap();
    assert_eq!(e.alvo_id, 46027);
    assert_eq!(e.autor_id, 11174);
    assert_eq!(e.skill, 14770007);
    assert_eq!(e.dano, 232);
    assert!(!e.critico);
}

#[test]
fn dano_seletor_6_com_direcao() {
    let e = combate::dano(&hex("240438CBE7021600A657304DD7003102800002CB261A5401000000AC57EC050100")).unwrap();
    assert_eq!(e.skill, 14110000);
    assert_eq!(e.dano, 748);
    assert!(e.frente);
    assert!(!e.costas);
}

#[test]
fn golpe_sem_dano_seletor_0_e_descartado() {
    let motivo = combate::dano(&hex("1F0438CBE7020000A657509BD7002E024CAB385401000000AC570100")).unwrap_err();
    assert!(motivo.contains("seletor"));
}

#[test]
fn hp_de_mob() {
    assert_eq!(combate::hp_restante(&hex("14008DCBE7020201004E9A010000000000")), Some((46027, 105038)));
}

#[test]
fn spawn_de_armadilha_traz_dono_e_nome_do_dono() {
    let pacote = hex(concat!(
        "C7014136DCC2035F000105596F736869AC902C000002007C12C600F88EC500F1",
        "14477FAE5543F49701D922D9225B0700005B0700000000000000000000000000",
        "008493010064000000F04902000100000000000000A08601000000000090D003",
        "000201110181969800FFFFFFFFFFFFFFFF8075D52ABB030000DCC2030102007C",
        "12C600F88EC500F11447070206A62B00006C0000000000610907547562617A61",
        "6902000200000000000000000000000000000002CD0006040000D0002F010000",
        "1800000000",
    ));

    let s = combate::spawn_invocacao(&pacote);
    assert!(s.invocacao);
    assert_eq!(s.entidade_id, 57692);
    assert_eq!(s.dono_id, 11174);
    assert_eq!(s.nome_dono, "Yoshi");
    assert_eq!(s.dono_marcado, 11174);
}

#[test]
fn spawn_de_espirito_traz_dono_so_no_marcador() {
    // Espírito da Ivy (#512, Elementalist), máscara 0x101F, captura de login de 2026-10-02.
    let pacote = hex(concat!(
        "A8014136E2B3021F1000AE8E2C000002AD709847A342ECC700041046818EEE40",
        "4D0501EA2DEA2D710600007106000000000000000000000000000018D7010064",
        "000000F04902000100000000000000A08601000000000090D003000201110144",
        "AA9809FFFFFFFFFFFFFFFF8075D52ABB03000080040A02EDFB9B478896C3C700",
        "A84E460702010002000003CD009C040000D0003D010000D6003CF6FFFF1E0000",
        "00DD1D030000",
    ));

    let s = combate::spawn_invocacao(&pacote);
    assert_eq!(s.entidade_id, 39394);
    assert!(!s.invocacao); // não é 0x5F: o vínculo depende do Medidor conferir o dono
    assert_eq!(s.dono_marcado, 512);
}

#[test]
fn morte_de_mob_traz_skill_matador_e_nome() {
    let m = combate::morte(&hex("28048DCBE702A0EDD500A657610905596F73686907547562617A6169020000000000000100")).unwrap();
    assert_eq!(m.morto, 46027);
    assert_eq!(m.matador, 11174);
    assert_eq!(m.skill, 14020000);
    assert_eq!(m.servidor, 2401);
    assert_eq!(m.nome_matador, "Yoshi");
}

#[test]
fn morte_de_armadilha_que_expirou_vem_sem_matador() {
    let m = combate::morte(&hex("1B048DDCC203000000000000000000000000800600000000")).unwrap();
    assert_eq!(m.morto, 57692);
    assert_eq!(m.matador, 0);
    assert_eq!(m.nome_matador, "");
}

#[test]
fn golpe_de_mob_no_jogador() {
    let e = combate::dano(&hex("240438A6570600CAAF03ACAA120002020000023BAB4A0701000000904EA6010100")).unwrap();
    assert_eq!(e.alvo_id, 11174);
    assert_eq!(e.autor_id, 55242);
    assert_eq!(e.skill, 1223340);
    assert_eq!(e.dano, 166); // o HP do jogador caiu exatamente 166 (PROTOCOLO.md §6)
}

// 0x3645 de 2026-10-02: cabeçalho real + trecho real do bloco do level (os ~1.000 bytes de
// equipamento do meio foram cortados). Conferidos no jogo: Dacura level 45 (o 32 logo depois do
// nome não é o level), Nxhunter level 43 e power 909. O 1.040 do Dacura e o 366 do Auril vêm do
// mesmo campo, sem conferência própria; o Auril cobre o bloco que começa pela tag 0xCE.
fn conferir_info_de_outro_jogador(cabecalho: &str, bloco: &str, id: u32, nome: &str, nivel: i32, poder: i32) {
    let info = combate::info_jogador(&hex(&format!("{cabecalho}{bloco}"))).unwrap();
    assert_eq!(info.entidade_id, id);
    assert_eq!(info.nome, nome);
    assert_eq!(info.nivel, nivel);
    assert_eq!(info.poder, poder);
}

#[test]
fn info_de_outro_jogador_dacura() {
    conferir_info_de_outro_jogador(
        "D60A4536AD7E0520A00107064461637572612000000002024096C06A8947",
        "0000000E0101CE5704000000610904CD008C000000CE0060F0FFFFD00036010000270248F4FFFF2D0000000000000010040000DE020C",
        16173,
        "Dacura",
        45,
        1040,
    );
}

#[test]
fn info_de_outro_jogador_auril() {
    conferir_info_de_outro_jogador(
        "BA0A4536CB6B15B0A4010705417572696C240000000202009227D39C46",
        "0000000E01019E8404000000610903CE0048F4FFFFD00043010000270248F4FFFF20000000000000006E0100002A0400",
        13771,
        "Auril",
        32,
        366,
    );
}

#[test]
fn info_de_outro_jogador_nxhunter() {
    conferir_info_de_outro_jogador(
        "E50A4536E64B15B0A40107084E7868756E74657210000000020200D2",
        "0000000E01019D6404000000610904CD00AA000000CE0060F0FFFFD00033010000270248F4FFFF2B000000000000008D03000068040000",
        9702,
        "Nxhunter",
        43,
        909,
    );
}

#[test]
fn info_de_outro_jogador_sem_bloco_fica_sem_level() {
    let info = combate::info_jogador(&hex("D60A4536AD7E0520A00107064461637572612000000002024096C06A8947")).unwrap();
    assert_eq!(info.nome, "Dacura");
    assert_eq!(info.nivel, 0);
    assert_eq!(info.poder, 0);
}

#[test]
fn seu_personagem_no_login_traz_level_e_power() {
    let eu = combate::info_personagem(&hex(
        "8D0F3336AB575FA1C1283705596F73686961090F000000021F00000063010000630100001F000000",
    ))
    .unwrap();
    assert_eq!(eu.entidade_id, 11179);
    assert_eq!(eu.nome, "Yoshi");
    assert_eq!(eu.nivel, 31);
    assert_eq!(eu.poder, 355);
}

#[test]
fn atualizacao_de_power() {
    // 0x561C de 2026-10-02 03:58:20: o power do Yoshi foi de 355 para 361 (o jogo mostrava 361).
    assert_eq!(combate::poder(&hex("511C56AB576901000001018955950600010000000000000000EE00")), Some((11179, 361)));
}

#[test]
fn spawn_de_mob_nao_e_invocacao() {
    let s = combate::spawn_invocacao(&hex("6D413687D1030C20001D38290000020000B1C5008438C600680C4700809D4300E00194A70794A707"));
    assert!(!s.invocacao);
    assert_ne!(s.entidade_id, 0);
    assert_eq!(s.dono_id, 0);
    assert_eq!(s.dono_marcado, 0);
}
