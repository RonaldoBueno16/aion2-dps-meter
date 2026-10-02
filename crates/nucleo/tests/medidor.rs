mod comum;

use comum::{T0, quase_igual, segundos};
use nucleo::medicao::medidor::{FATOR_ESCALA, FATOR_ESCALA_JOGADOR, Medidor, PerfilJogador};
use nucleo::protocolo::combate::EventoDano;

const MOB: u32 = 46027;

fn golpe(autor: u32, skill: u32, dano: u64) -> EventoDano {
    EventoDano { alvo_id: MOB, autor_id: autor, skill, dano, tipo_dano: 2, ..Default::default() }
}

fn golpe_em(alvo: u32, autor: u32, skill: u32, dano: u64) -> EventoDano {
    EventoDano { alvo_id: alvo, ..golpe(autor, skill, dano) }
}

#[test]
fn separa_por_jogador_e_por_skill_e_soma_invocacao_no_dono() {
    let mut m = Medidor::default();
    m.definir_invocacao(57692, 11174, "Yoshi");

    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar(EventoDano { critico: true, tipo_dano: 3, ..golpe(11174, 14030010, 50) }, T0 + segundos(1.0)); // variante de 14030000
    m.registrar(golpe(57692, 14170001, 200), T0 + segundos(2.0)); // armadilha do Yoshi
    m.registrar(golpe(22222, 11010000, 300), T0 + segundos(4.0)); // outro jogador
    m.registrar(golpe_em(44444, 33333, 1223340, 999), T0 + segundos(4.0)); // mob em mob: fora

    let p = m.obter_placar();
    let dano = &p.dano;

    assert_eq!(dano.jogadores.len(), 2);
    assert_eq!(p.duracao, segundos(4.0));

    let yoshi = dano.jogadores.iter().find(|j| j.id == 11174).unwrap();
    assert_eq!(yoshi.nome, "Yoshi");
    assert_eq!(yoshi.classe, "Ranger");
    quase_igual(350.0 * FATOR_ESCALA, yoshi.total);
    assert_eq!(yoshi.golpes, 3);
    assert_eq!(yoshi.criticos, 1);
    assert_eq!(yoshi.skills.len(), 3);
    assert_eq!(yoshi.skills[0].skill, 14170000);
    // Tabela da aba DPS: maior golpe do jogador e DPS de cada skill na mesma duração da luta.
    quase_igual(200.0 * FATOR_ESCALA, yoshi.maximo);
    quase_igual(200.0 * FATOR_ESCALA / 4.0, yoshi.skills[0].por_segundo);
    let critica: Vec<_> = yoshi.skills.iter().filter(|s| s.golpes == 1 && s.criticos == 1).collect();
    assert_eq!(critica.len(), 1);
    assert_eq!(critica[0].skill, 14030000);

    let outro = dano.jogadores.iter().find(|j| j.id == 22222).unwrap();
    assert_eq!(outro.classe, "Gladiator");
    quase_igual(300.0 / 650.0, outro.porcentagem);
    quase_igual(300.0 * FATOR_ESCALA / 4.0, outro.por_segundo);

    assert!(p.dano_recebido.jogadores.is_empty());
    assert!(p.cura.jogadores.is_empty());
}

#[test]
fn golpe_de_mob_em_jogador_vai_para_dano_recebido_e_marca_aggro() {
    let mut m = Medidor::default();
    m.registrar(golpe_em(55242, 11174, 14340000, 100), T0); // Yoshi bate no mob
    m.registrar(EventoDano { aparo: true, ..golpe_em(11174, 55242, 1223340, 166) }, T0 + segundos(1.0)); // mob revida
    m.registrar(golpe_em(11174, 55242, 1223340, 191), T0 + segundos(2.0));

    let p = m.obter_placar();
    assert_eq!(p.dano_recebido.jogadores.len(), 1);
    let tank = &p.dano_recebido.jogadores[0];
    assert_eq!(tank.id, 11174);
    quase_igual((166.0 + 191.0) * FATOR_ESCALA_JOGADOR, tank.total);
    assert_eq!(tank.golpes, 2);
    assert_eq!(tank.aparos, 1);
    assert_eq!(tank.segurando_aggro, 1);
}

#[test]
fn cura_em_aliado_e_em_si_mesmo_vai_para_healer_e_nao_para_dano() {
    let mut m = Medidor::default();
    m.registrar(golpe(11174, 14340000, 100), T0); // Yoshi vira jogador conhecido
    m.registrar(golpe(30303, 17010000, 50), T0 + segundos(1.0)); // Clérigo bate no mob
    m.registrar(golpe_em(11174, 30303, 17120000, 400), T0 + segundos(2.0)); // Fulgor Restaurador no Yoshi
    m.registrar(golpe_em(30303, 30303, 17120000, 380), T0 + segundos(3.0)); // e em si mesmo
    m.registrar(golpe_em(11174, 30303, 17990000, 999), T0 + segundos(3.0)); // jogador → jogador sem ser cura: fora

    let p = m.obter_placar();
    assert_eq!(p.cura.jogadores.len(), 1);
    let clerigo = &p.cura.jogadores[0];
    assert_eq!(clerigo.id, 30303);
    assert_eq!(clerigo.classe, "Cleric");
    quase_igual(780.0 * FATOR_ESCALA_JOGADOR, clerigo.total);
    assert_eq!(clerigo.skills.len(), 1);
    assert_eq!(clerigo.skills[0].skill, 17120000);

    quase_igual(50.0 * FATOR_ESCALA, p.dano.jogadores.iter().find(|j| j.id == 30303).unwrap().total);
    assert!(p.dano_recebido.jogadores.is_empty());
}

#[test]
fn morte_de_jogador_conta_no_tank_e_nome_do_matador_jogador_e_aproveitado() {
    let mut m = Medidor::default();
    m.registrar(golpe_em(55242, 11174, 14340000, 100), T0);
    m.registrar_morte(55242, 11174, 14020000, "Yoshi", T0 + segundos(1.0)); // Yoshi mata o mob
    m.registrar(golpe_em(11174, 60000, 1223340, 500), T0 + segundos(2.0));
    m.registrar_morte(11174, 60000, 1223340, "Lobo", T0 + segundos(3.0)); // mob mata o Yoshi: "Lobo" não vira jogador

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    assert_eq!(p.dano.jogadores[0].nome, "Yoshi");
    assert_eq!(p.dano_recebido.jogadores.len(), 1);
    let tank = &p.dano_recebido.jogadores[0];
    assert_eq!(tank.mortes, 1);
    assert_eq!(tank.segurando_aggro, 0); // morto não segura aggro
}

#[test]
fn level_e_power_aparecem_na_linha_e_zero_nao_apaga() {
    let mut m = Medidor::default();
    m.definir_jogador(11174, "Yoshi", 30, true);
    m.definir_jogador(11174, "Yoshi", 0, true); // pacote sem level: mantém o 30
    m.definir_poder(11174, 355);
    m.definir_poder(11174, 361); // 0x561C depois do login
    m.definir_poder(11174, 0);
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar(golpe(22222, 11010000, 100), T0);

    let p = m.obter_placar();
    let yoshi = p.dano.jogadores.iter().find(|j| j.id == 11174).unwrap();
    assert_eq!(yoshi.nivel, 30);
    assert_eq!(yoshi.poder, 361);
    assert!(yoshi.voce);
    let outro = p.dano.jogadores.iter().find(|j| j.id == 22222).unwrap();
    assert_eq!(outro.nivel, 0);
    assert_eq!(outro.poder, 0);
}

#[test]
fn overlay_aberto_no_meio_da_sessao_reconhece_voce_pelo_nome_guardado() {
    let mut m = Medidor::default();
    m.carregar_memoria(Some("Yoshi".into()), [("Yoshi".to_string(), PerfilJogador { nivel: 31, poder: 375 })]);
    m.nova_conexao(); // a memória sobrevive à troca de conexão

    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar_morte(MOB, 11174, 14020000, "Yoshi", T0 + segundos(1.0)); // nome chega pelo abate

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    let yoshi = &p.dano.jogadores[0];
    assert_eq!(yoshi.nome, "Yoshi");
    assert!(yoshi.voce);
    assert_eq!(yoshi.nivel, 31);
    assert!(yoshi.nivel_lembrado);
    assert_eq!(yoshi.poder, 375);
    assert!(yoshi.poder_lembrado);
}

#[test]
fn valor_desta_conexao_vence_a_memoria_e_atualiza_a_memoria() {
    let mut m = Medidor::default();
    m.carregar_memoria(None, [("Dacura".to_string(), PerfilJogador { nivel: 44, poder: 1000 })]);
    m.definir_jogador(16173, "Dacura", 45, false); // 0x3645 desta conexão
    m.registrar(golpe(16173, 17010000, 100), T0);

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    let dacura = &p.dano.jogadores[0];
    assert_eq!(dacura.nivel, 45);
    assert!(!dacura.nivel_lembrado);
    assert_eq!(dacura.poder, 1000); // power ainda não chegou nesta conexão
    assert!(dacura.poder_lembrado);
    assert_eq!(m.exportar_memoria().1["Dacura"], PerfilJogador { nivel: 45, poder: 1000 });
}

#[test]
fn login_desta_conexao_nao_e_trocado_por_nome_guardado() {
    let mut m = Medidor::default();
    m.carregar_memoria(Some("Yoshi".into()), []);
    m.definir_jogador(500, "Yoshi", 31, true); // 0x3633
    m.registrar(golpe(500, 14340000, 100), T0);
    m.registrar(golpe(600, 14340000, 100), T0);
    m.registrar_morte(MOB, 600, 14020000, "Yoshi", T0 + segundos(1.0)); // outro id com o mesmo nome

    let p = m.obter_placar();
    assert!(p.dano.jogadores.iter().find(|j| j.id == 500).unwrap().voce);
    assert!(!p.dano.jogadores.iter().find(|j| j.id == 600).unwrap().voce);
}

#[test]
fn luta_nova_depois_de_inatividade() {
    let mut m = Medidor::default();
    m.inatividade = segundos(15.0);
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar(golpe(11174, 14340000, 100), T0 + segundos(16.0));

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores[0].golpes, 1);
    assert_eq!(p.duracao, 0);
}
