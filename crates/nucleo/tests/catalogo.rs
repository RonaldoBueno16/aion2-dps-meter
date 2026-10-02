//! Usa a rede (questlog.gg e o CDN da NCSoft): fica fora do `cargo test` comum.
//! Rodar com `cargo test -p nucleo --test catalogo -- --ignored`.

use std::time::{Duration, Instant};

use nucleo::medicao::catalogo::CatalogoSkills;

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
