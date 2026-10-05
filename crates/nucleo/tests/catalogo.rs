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
