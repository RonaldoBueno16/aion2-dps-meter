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
    // Depois do nome: "Armadilha de Explosão" no questlog.
    assert_eq!(s.codigo, 2920620);
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
    assert_eq!(s.codigo, 2920110); // "Espírito do Fogo"
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

#[test]
fn golpe_de_mob_com_flag_0x20_le_o_dano_depois_do_varint_do_bloco() {
    // World boss de 2026-10-03: flags 0x30 e o 2º campo do bloco em 2 bytes (B4 03 = 436). Lido como
    // u8, o dano saía 10.000 (o varint fixo 90 4E logo antes dele).
    let e = combate::dano(&hex("250438AF400600A7AA0182D91200410230B40302D3F65C0701000000904E86110100")).unwrap();
    assert_eq!(e.alvo_id, 8239);
    assert_eq!(e.autor_id, 21799);
    assert_eq!(e.skill, 1235330);
    assert_eq!(e.dano, 2182);
    assert!(e.frente);
}

#[test]
fn golpe_pelas_costas() {
    // Byte de direção 01 (world boss de 2026-10-03, seletor 0x26).
    let e = combate::dano(&hex("2D0438A7AA012600C6498C75A8003802000001BBEACD41010000009E55F91804BF02BF02BF02BF020100"))
        .unwrap();
    assert_eq!(e.alvo_id, 21799);
    assert_eq!(e.dano, 3193);
    assert!(e.costas);
    assert!(!e.frente);
}

#[test]
fn estado_de_combate() {
    assert_eq!(combate::estado_combate(&hex("0B218D97DE010000")), Some((28439, false)));
    assert_eq!(combate::estado_combate(&hex("0A218DE1590001")), Some((11489, true)));
}

#[test]
fn buff_novo_renovado_e_removido() {
    // 0x382A de classe (163300001, da skill 16330000) e o 0x382C da mesma instância 0,15 s depois.
    let novo = combate::buff(
        &hex("352A38A7AA010113B10AA1C2BB09640000000000000092999A02A1010000A576012E2DF90000FBEF2148A4257CC78ADE2A46"),
        true,
    )
    .unwrap();
    assert_eq!(novo, combate::Buff { alvo: 21799, instancia: 1329, codigo: 163300001, duracao_ms: 100 });
    assert_eq!(combate::buffs_removidos(&hex("122C38A7AA010200C6030100B10A01")), Some((21799, vec![454, 1329])));

    // 0x382B (sem o u8 01) com a instância em 1, 2 e 3 bytes de varint.
    let renovado = |h: &str| combate::buff(&hex(h), false).unwrap();
    assert_eq!(
        renovado("332B38A7AA01135827285D098813000000000000FAA99A02A1010000922D0137B7EF000270F71F481E7373C700D82946"),
        combate::Buff { alvo: 21799, instancia: 88, codigo: 157100071, duracao_ms: 5000 }
    );
    assert_eq!(
        renovado("332B38881A13C007EB92D70A600900000000000091679A02A1010000881A01B18E15010223B6F947702C6EC7008C4146"),
        combate::Buff { alvo: 3336, instancia: 960, codigo: 181900011, duracao_ms: 2400 }
    );
    assert_eq!(
        renovado("342B389A5213FDC001ABF8290BCF1800000000000074D09A02A10100009A520D11CC1D0100488920483EBA7BC7CBE72946"),
        combate::Buff { alvo: 10522, instancia: 24701, codigo: 187300011, duracao_ms: 6351 }
    );
}

#[test]
fn remocao_de_buffs_com_entradas_tipo_0_e_tipo_7() {
    // Tipo 7 traz 10 bytes a mais depois do motivo.
    let pacote = hex(concat!(
        "402C38A7AA01060083070100B00A0100EC0A0107EB0A0BFF211059C80062CA424E078E0C0BFF21",
        "1059C80062CA424E07AF0B0BFF211059C80062CA424E",
    ));
    assert_eq!(combate::buffs_removidos(&pacote), Some((21799, vec![899, 1328, 1388, 1387, 1550, 1455])));
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
    assert_eq!(s.codigo, 2701341); // "Seguidor de Zikel", Nv 20
    assert_eq!(s.hp, Some((119_700, 119_700)));
}

#[test]
fn tickets_do_login_trazem_a_odyle() {
    // Login de 2026-10-05, com a tela mostrando 550(+270)/840: lista de 73 tickets.
    let login = "F2030B614904010000000A01030000008085800100000000040400000006000600000004070000002004080000000301\
                 090000008085800100000000040A0000000E040B00000002040C0000000E010D0000008085800100000000040E000000\
                 0E040F0000000E04650000000E04660000000504670000000501C9000000808580010000000001CA0000008085800100\
                 00000001CB000000808580010000000001CC000000808580010000000001CD000000808580010000000001CE00000080\
                 858001000000000081969800008296980000839698000084969800008596980000869698000087969800008896980000\
                 89969800008A969800008B969800008C9698000C01879303A6048E0204028793030604038793030A04048793030A0405\
                 8793030A04068793030904078793032304088793031C04098793030A040A8793030704658793030A04C987930307044D\
                 8B93030704B18B93030A04358F93030704998F93030A041D9393030704819393030A04059793030704699793030704B5\
                 9B930304049D9F93030404BDA29303070421A393030A048DAA93030704F1AA93030A0445B693030704A9B6930307042D\
                 BA9303070415BE93030704FDC193030704811D2C04030401B4C4040304814A5D050404824A5D050104834A5D05040484\
                 4A5D050204854A5D050404864A5D0502";
    let lista = combate::tickets(&hex(login)).unwrap();
    assert_eq!(lista.len(), 73);
    // Cortada no último byte: nada (a leitura tem de fechar no fim).
    assert_eq!(combate::tickets(&hex(&login[..login.len() - 2])), None);
    let odyle = |lista: &[combate::Ticket]| lista.iter().copied().find(|t| t.id == combate::TICKET_ODYLE);
    let id = combate::TICKET_ODYLE;
    assert_eq!(odyle(&lista), Some(combate::Ticket { id, valor: Some(550), extra: Some(270) }));
    // Ticket comum: só o valor (entrada tipo 04).
    let comum = lista.iter().find(|t| t.id == 60_000_002);
    assert_eq!(comum, Some(&combate::Ticket { id: 60_000_002, valor: Some(6), extra: None }));

    // Login de 2026-10-02: a Odyle sem carregada (tipo 04, sem o extra).
    let lista = combate::tickets(&hex(
        "F0030B614904010000000201030000008085800100000000040400000002000600000004070000000404080000000101\
         090000008085800100000000040A0000000E040B00000002040C00000002010D0000008085800100000000040E000000\
         0E040F0000000E04650000000E04660000000104670000000101C9000000808580010000000001CA0000008085800100\
         00000001CB000000808580010000000001CC000000808580010000000001CD000000808580010000000001CE00000080\
         858001000000000081969800008296980000839698000084969800008596980000869698000087969800008896980000\
         89969800008A969800008B969800008C96980004018793039B0104028793030204038793030204048793030204058793\
         030204068793030304078793032304088793031C04098793030A040A8793030704658793030204C987930307044D8B93\
         030704B18B93030204358F93030704998F930302041D9393030704819393030204059793030704699793030704B59B93\
         0302049D9F93030204BDA29303070421A3930302048DAA93030704F1AA9303020445B693030704A9B6930307042DBA93\
         03070415BE93030704FDC193030704811D2C04030401B4C4040304814A5D050404824A5D050104834A5D050404844A5D\
         050204854A5D050404864A5D0502",
    ))
    .unwrap();
    assert_eq!(odyle(&lista), Some(combate::Ticket { id, valor: Some(155), extra: None }));

    // 0x610C de 2026-10-03: o ticket 10 com valor 14. Lido como lista, sobra byte: nada.
    let mudou = hex("0E0C6100040A0000000E02");
    assert_eq!(combate::ticket_mudou(&mudou), Some(combate::Ticket { id: 10, valor: Some(14), extra: None }));
    assert_eq!(combate::tickets(&mudou), None);

    // 0x610C de 2026-10-05 no uso de uma essência OD: carregada 300 → 310 (primeiro byte 01).
    let essencia = hex("150C61010C01879303A604B602010A000000");
    assert_eq!(combate::ticket_mudou(&essencia), Some(combate::Ticket { id, valor: Some(550), extra: Some(310) }));
}

#[test]
fn spawn_traz_hp_atual_e_maximo() {
    // Começo de spawns reais das capturas de 2026-10-02 e 03, um de cada flag depois do código.
    let casos = [
        // 0x40: world boss Axios, já apanhando (o 1º 0x8D00 depois dele traz 138.076.470).
        ("FF0D4136A7AA01042000A9A024004002E7A62048C6047BC700042A466CE89A4350DC01B6C2EB4180D0A54C64000000", 21799, 2400425, Some((138_076_470, 160_000_000))),
        // 0x00: mob comum inteiro.
        ("6D4136ACAA0105200032A024000002C54F7246263781C700E06A466EAA2243AC730194A70794A70764000000", 21804, 2400306, Some((119_700, 119_700))),
        // 0x00: mob que já veio morto.
        ("6C4136CA8F010D20009BA024000002687C1F48CB3D79C7004C2946F1FC1B43ED6E000094A70764000000", 18378, 2400411, Some((0, 119_700))),
        // 0x08 e 0x48: 3 floats a mais antes do HP.
        ("AD01413682B9031D1000D48E2C000802703F1E4841CB7CC797CB2A460DC8AA43E4F2949C6C44B8C29DC37DE688C201E729E729E8060000", 56450, 2920148, Some((5_351, 5_351))),
        ("AE014136E3EE021D1000AF8E2C004802A2F31F489C8E71C7A3612B4602E1914379CF951D8A4378062DC4079EA942018D6E8D6EC80A0000", 46947, 2920111, Some((14_093, 14_093))),
    ];
    for (pacote, entidade, codigo, hp) in casos {
        let s = combate::spawn_invocacao(&hex(pacote));
        assert_eq!((s.entidade_id, s.codigo, s.hp), (entidade, codigo, hp), "{pacote}");
    }

    // Flag nunca vista (0x01 no lugar do 0x40 do Axios): layout desconhecido, sem HP.
    let s = combate::spawn_invocacao(&hex("FF0D4136A7AA01042000A9A024000102E7A62048C6047BC700042A466CE89A4350DC01B6C2EB4180D0A54C"));
    assert_eq!((s.codigo, s.hp), (2400425, None));
    // Atual acima do máximo: leitura errada, descartada.
    let s = combate::spawn_invocacao(&hex("6D4136ACAA0105200032A024000002C54F7246263781C700E06A466EAA2243AC7301A0A70794A707"));
    assert_eq!(s.hp, None);
}

// 0x9101 de 2026-10-06 17:25 (545 bytes): Altgard, 24 chefes, o Gartua Imortal (111021) morto. O jogo
// mostrava "Tempo restante 4h 58min 5s" às 17:22:36, ou seja, renasce às 22:20:41 (±2 s).
const CHEFES_GARTUA: &str = concat!(
        "A304019100005604000018019CE3061DBC2FC8D56A96C50002C546EF5D72500F",
        "A1010000019AE306325C15C89ED7DCC700F90947D12F380FA10100000199E306",
        "91939BC7645092C700ECDD46482B300FA1010000019BE306544326C84E308AC6",
        "00F4BC46FF81300FA101000000AEE3067BFB0414A1010000019DE306DA1467C5",
        "1152204700FC6446FA2A790FA101000001A4E306413F42482DC5E5C700B69746",
        "7BD23B0FA1010000019EE30600D004C7005A204700207E460CDE630FA1010000",
        "019FE3069C2A94466B3F02C7001C3146FF3BD2730FA101000001A0E30600D8AE",
        "4600A28AC700C869465ABA6B0FA101000001A1E30600A57447800A0DC800F0DB",
        "458366630FA101000001A2E306000C7E47005230C80058BD45C83E9E0FA10100",
        "0001A3E306EE710B48DD27D9C700409546B2DE8F0FA101000001A5E306BDB820",
        "48E4BB79C7005C2A463DABC30FA101000001A7E306BC66F147A584BC4700B885",
        "45953BBD0FA101000001A6E306AFE0F7471D71FB460040E4445F7A890FA10100",
        "0001A8E306B38D3048612DE54700EC55461F2EAE2710A101000001A9E306E0D4",
        "D34713801D4800C8D145ACFE3C12A101000001AAE306B5688847A87A19480014",
        "0B46A3FE6612A101000001ABE30638429FC511181A480088A9C56DABCA12A101",
        "000001ACE306CCF91048968E2D480094A7464D782410A101000000ADE306B3C8",
        "F213A101000000AFE306323A1E14A101000000B0E3069B720814A10100000000",
        "00",
);

// 0x9101 de 2026-10-03 (461 bytes): a entrada 16, que traz o byte da máscara, morta.
const CHEFES_BOSS: &str = concat!(
        "CF03019100005604000018019AE306325C15C89ED7DCC700F90947BF9F8D1000",
        "A10100000199E30691939BC7645092C700ECDD46D1741000A1010000019CE306",
        "1DBC2FC8D56A96C50002C54638700700A1010000019BE306544326C84E308AC6",
        "00F4BC4638A31200A1010000019DE306DA1467C51152204700FC644679205800",
        "A101000001A0E30600D8AE4600A28AC700C86946C17B4B00A101000000ADE306",
        "94760A05A1010000019EE30600D004C7005A204700207E469E042B00A1010000",
        "019FE3069C2A94466B3F02C7001C31466F96DF5600A101000001A1E30600A574",
        "47800A0DC800F0DB45EFD55100A101000001A2E306000C7E47005230C80058BD",
        "45300B0D00A101000001A3E306EE710B48DD27D9C700409546A4C6DF00A10100",
        "0000ACE306F198DA03A101000001A4E306413F42482DC5E5C700B697464B7137",
        "00A101000001A5E306BDB82048E4BB79C7005C2A469E753400A101000000A6E3",
        "064D5A3E03A101000000A7E30600E4997203A101000000A8E3063BA4DD03A101",
        "000000A9E30681C33203A101000000AAE306D23D6603A101000000ABE306D198",
        "C603A101000000AEE30602EC1905A101000000AFE306B63FB703A101000000B0",
        "E3065D59AA03A1010000000000",
);

#[test]
fn chefes_de_campo_trazem_quem_esta_vivo_e_a_hora_de_renascer() {
    let lista = combate::chefes_de_campo(&hex(CHEFES_GARTUA)).unwrap();
    assert_eq!(lista.regiao, 1110);
    assert_eq!(lista.chefes.len(), 24);
    assert_eq!(lista.chefes.iter().filter(|c| c.vivo).count(), 20);
    let gartua = lista.chefes.iter().find(|c| c.id == 111_021).unwrap();
    assert!(!gartua.vivo);
    assert_eq!(gartua.posicao, None);
    assert_eq!(gartua.hora_ms, 1_791_336_040_627); // 22:20:40,627 de Brasília
    // Axios vivo: a posição é a mesma do 0x3641 dele (captura de 2026-10-03), e a hora é a de quando nasceu.
    let axios = lista.chefes.iter().find(|c| c.id == 111_013).unwrap();
    assert!(axios.vivo);
    assert_eq!(axios.posicao, Some([164_578.95, -63_931.89, 10_903.0]));
    assert_eq!(axios.hora_ms, 1_791_265_844_029);
}

#[test]
fn chefes_de_campo_com_o_dono_da_mascara_morto() {
    let lista = combate::chefes_de_campo(&hex(CHEFES_BOSS)).unwrap();
    assert_eq!((lista.regiao, lista.chefes.len()), (1110, 24));
    assert_eq!(lista.chefes.iter().filter(|c| c.vivo).count(), 13);
    let decimo_sexto = &lista.chefes[16];
    assert_eq!((decimo_sexto.id, decimo_sexto.vivo, decimo_sexto.hora_ms), (111_015, false, 1_791_059_204_580));
}

#[test]
fn chefes_de_campo_com_mascara_que_nao_bate_sao_recusados() {
    // A primeira máscara (byte 27, 0xEF: o 5º chefe do grupo morto) passa a dizer que os 8 estão vivos.
    let mut pacote = hex(CHEFES_GARTUA);
    assert_eq!(pacote[27], 0xEF);
    pacote[27] = 0xFF;
    assert_eq!(combate::chefes_de_campo(&pacote), None);
}
