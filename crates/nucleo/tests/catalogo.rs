//! Usa a rede (questlog.gg e o CDN da NCSoft): fica fora do `cargo test` comum.
//! Rodar com `cargo test -p nucleo --test catalogo -- --ignored`.

use std::time::{Duration, Instant};

use nucleo::medicao::catalogo::{self, CatalogoSkills};

#[test]
#[ignore = "usa a rede"]
fn baixa_nomes_em_portugues_e_icone_sem_cache() {
    let pasta = std::env::temp_dir().join(format!("aion2meter-teste-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    let catalogo = CatalogoSkills::novo(Some(pasta.clone()));
    catalogo.esperar_pronto(Duration::from_secs(120));
    assert!(catalogo.quantidade() > 100, "listagem inicial trouxe {}", catalogo.quantidade());
    assert!(pasta.join("skills-pt.json").is_file());

    // Disparo Rápido (Ranger): nome em português e ícone baixado sob demanda.
    let info = catalogo.obter(14340000).expect("skill da listagem");
    assert_eq!(info.nome, "Disparo Rápido");
    assert!(catalogo.caminho_icone(info.icone.as_deref()).is_none(), "ícone ainda não baixado");
    let limite = Instant::now() + Duration::from_secs(30);
    while catalogo.caminho_icone(info.icone.as_deref()).is_none() && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(200));
    }
    let icone = catalogo.caminho_icone(info.icone.as_deref()).expect("ícone baixado");
    assert!(std::fs::read(icone).unwrap().starts_with(b"\x89PNG"));
    let _ = std::fs::remove_dir_all(&pasta);
}

#[test]
fn npc_do_questlog_traz_nome_level_chefe_e_retrato() {
    // Resposta real do getNpc (2026-10-05), sem a lista de drops.
    let boss: serde_json::Value = serde_json::from_str(
        r#"{"id":"2400425","name":"Arconte da Alma Perdida Axios","icon":"/assets/Game/UI/Resource/Texture/Portrait/Portrait_256/UT_256_MOB_DstrArchonE_01.UT_256_MOB_DstrArchonE_01","language":"pt","dbType":"npc","mainCategory":"monster","subDescription":"Altgard Único","level":45,"npcType":"monster","npcSubType":"heromonster","creatureType":"intellect","isNamed":true}"#,
    )
    .unwrap();
    let (codigo, info) = catalogo::ler_npc(&boss).unwrap();
    assert_eq!((codigo, info.nome.as_str(), info.nivel), (2400425, "Arconte da Alma Perdida Axios", 45));
    assert_eq!(info.retrato.as_deref(), Some("UT_256_MOB_DstrArchonE_01"));
    assert!(info.chefe());

    let mob: serde_json::Value = serde_json::from_str(
        r#"{"id":"2400939","name":"Aranha do Galho Seco","level":33,"npcType":"monster","npcSubType":"normalmonster","isNamed":false}"#,
    )
    .unwrap();
    let (_, info) = catalogo::ler_npc(&mob).unwrap();
    assert!(!info.chefe());
    assert_eq!(info.retrato, None);

    // Chefe de dungeon vem igual ao world boss: herói e nomeado.
    let dungeon: serde_json::Value = serde_json::from_str(
        r#"{"id":"2300371","name":"Desejo de Kromede","level":63,"npcType":"monster","npcSubType":"heromonster","isNamed":true}"#,
    )
    .unwrap();
    assert!(catalogo::ler_npc(&dungeon).unwrap().1.chefe());

    // Invocação também vem nomeada no questlog, e não é chefe.
    let totem: serde_json::Value = serde_json::from_str(
        r#"{"id":"2920205","name":"Totem de Recuperação","level":10,"npcType":"monster","npcSubType":"normalsummon","isNamed":true}"#,
    )
    .unwrap();
    assert!(!catalogo::ler_npc(&totem).unwrap().1.chefe());
}

#[test]
fn regiao_do_questlog_traz_os_chefes_em_ordem_de_codigo() {
    // Trecho do getRegion de Altgard (2026-10-06), fora de ordem como veio, com 3 dos 24 NPCs; o
    // "icon" do Newbold foi trocado por null para testar o chefe sem retrato.
    let regiao: serde_json::Value = serde_json::from_str(
        r#"{"id":"1110","name":"Altgard","mainCategory":"dark","regionHasNpcs":[{"id":"2400800","icon":"/assets/Game/UI/Resource/Texture/Portrait/Portrait_256/UT_256_MOB_Gartua_01_V02.UT_256_MOB_Gartua_01_V02","name":"Gartua Imortal","count":1,"level":51,"dbType":"npc"},{"id":"2400425","icon":"/assets/Game/UI/Resource/Texture/Portrait/Portrait_256/UT_256_MOB_DstrArchonE_01.UT_256_MOB_DstrArchonE_01","name":"Arconte da Alma Perdida Axios","count":1,"level":45,"dbType":"npc"},{"id":"2400424","icon":null,"name":"Profanador Newbold","count":1,"level":45,"dbType":"npc"}]}"#,
    )
    .unwrap();
    let (codigo, info) = catalogo::ler_regiao(&regiao).unwrap();
    assert_eq!((codigo, info.nome.as_str()), (1110, "Altgard"));
    let nomes: Vec<_> = info.chefes.iter().map(|c| (c.codigo, c.nome.as_str(), c.nivel)).collect();
    assert_eq!(
        nomes,
        [
            (2400424, "Profanador Newbold", 45),
            (2400425, "Arconte da Alma Perdida Axios", 45),
            (2400800, "Gartua Imortal", 51),
        ]
    );
    assert_eq!(info.chefes[2].retrato.as_deref(), Some("UT_256_MOB_Gartua_01_V02"));
    assert_eq!(info.chefes[0].retrato, None);
}

#[test]
fn itens_do_questlog_sem_repetir_e_com_a_chance_quando_vem() {
    // Trecho real do itemContainsItems do Baú de Saque de Newbold (2026-10-06): a lasca vem duas vezes,
    // sem chance e com 100%.
    let bau: serde_json::Value = serde_json::from_str(
        r#"[{"id":"632520098","icon":"/assets/Game/UI/Resource/Texture/Item/ETC/Icon_Item_AD_Pic_A_c_002.Icon_Item_AD_Pic_A_c_002","name":"Lasca da Obra-prima: Chefe de Campo I (Elyseano) (Vinculado)","grade":11,"chance":null,"mainCategory":"misc","subCategory":"conversionresource"},{"id":"210540129","icon":"/assets/Game/UI/Resource/Texture/Item/Armor/Icon_Equip_AR_L_0013_T03_Gloves.Icon_Equip_AR_L_0013_T03_Gloves","name":"Luvas de Newbold","grade":31,"chance":0.035714285,"mainCategory":"armor","subCategory":"gloves"},{"id":"632520098","icon":"/assets/Game/UI/Resource/Texture/Item/ETC/Icon_Item_AD_Pic_A_c_002.Icon_Item_AD_Pic_A_c_002","name":"Lasca da Obra-prima: Chefe de Campo I (Elyseano) (Vinculado)","grade":11,"chance":1,"mainCategory":"misc","subCategory":"conversionresource"}]"#,
    )
    .unwrap();
    let itens = catalogo::ler_itens(Some(&bau));
    let resumo: Vec<_> = itens.iter().map(|i| (i.codigo, i.chance, i.raridade)).collect();
    assert_eq!(resumo, [(632520098, Some(1.0), 11), (210540129, Some(0.035714285), 31)]);
    assert_eq!(itens[1].icone.as_deref(), Some("Icon_Equip_AR_L_0013_T03_Gloves"));
    assert_eq!(itens[1].quantidade, None);

    // npcDropsItems: com a quantidade; a Pedra de Mana vem sem chance.
    let npc: serde_json::Value = serde_json::from_str(
        r#"[{"id":"511360001","icon":"/assets/Game/UI/Resource/Texture/Item/ETC/Icon_Usable_MagicStone_a_001.Icon_Usable_MagicStone_a_001","name":"Pedra de Mana Inferior","grade":11,"chance":null,"countMin":1,"countMax":1,"mainCategory":"usable","subCategory":"magicstone"}]"#,
    )
    .unwrap();
    let itens = catalogo::ler_itens(Some(&npc));
    assert_eq!((itens[0].chance, itens[0].quantidade, itens[0].tipo.as_str()), (None, Some((1, 1)), "magicstone"));
    assert!(catalogo::ler_itens(None).is_empty());
}
