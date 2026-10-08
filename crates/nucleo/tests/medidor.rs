mod comum;

use comum::{T0, quase_igual, segundos};
use nucleo::medicao::catalogo::InfoNpc;
use nucleo::medicao::medidor::{Groggy, LUTAS_GUARDADAS, Medidor, PerfilJogador, TicketVisto};
use nucleo::protocolo::combate::{BarraGroggy, Buff, ChefeDeCampo, ChefesDeCampo, EventoDano, TICKET_ODYLE, Ticket};

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
    quase_igual(350.0, yoshi.total);
    assert_eq!(yoshi.golpes, 3);
    assert_eq!(yoshi.criticos, 1);
    assert_eq!(yoshi.skills.len(), 3);
    assert_eq!(yoshi.skills[0].skill, 14170000);
    // Tabela da aba DPS: maior golpe do jogador e DPS de cada skill no tempo ativo dele (0 s → 2 s).
    quase_igual(200.0, yoshi.maximo);
    quase_igual(350.0 / 2.0, yoshi.por_segundo);
    quase_igual(200.0 / 2.0, yoshi.skills[0].por_segundo);
    let critica: Vec<_> = yoshi.skills.iter().filter(|s| s.golpes == 1 && s.criticos == 1).collect();
    assert_eq!(critica.len(), 1);
    assert_eq!(critica[0].skill, 14030000);

    let outro = dano.jogadores.iter().find(|j| j.id == 22222).unwrap();
    assert_eq!(outro.classe, "Gladiator");
    quase_igual(300.0 / 650.0, outro.porcentagem);
    // Um golpe só: 1 s mínimo.
    quase_igual(300.0 / 1.0, outro.por_segundo);

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
    quase_igual(166.0 + 191.0, tank.total);
    assert_eq!(tank.golpes, 2);
    assert_eq!(tank.aparos, 1);
    assert_eq!(tank.skills[0].aparos, 1); // coluna PARRY da skill expandida
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
    quase_igual(780.0, clerigo.total);
    assert_eq!(clerigo.skills.len(), 1);
    assert_eq!(clerigo.skills[0].skill, 17120000);

    quase_igual(50.0, p.dano.jogadores.iter().find(|j| j.id == 30303).unwrap().total);
    assert!(p.dano_recebido.jogadores.is_empty());
}

#[test]
fn morte_de_jogador_conta_no_tank_e_nome_do_matador_jogador_e_aproveitado() {
    let mut m = Medidor::default();
    m.registrar(golpe_em(55242, 11174, 14340000, 100), T0);
    m.registrar_morte(55242, 11174, 14020000, 2401, "Yoshi", T0 + segundos(1.0)); // Yoshi mata o mob
    m.registrar(golpe_em(11174, 60000, 1223340, 500), T0 + segundos(2.0));
    m.registrar_morte(11174, 60000, 1223340, 0, "Lobo", T0 + segundos(3.0)); // mob mata o Yoshi: "Lobo" não vira jogador

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    assert_eq!(p.dano.jogadores[0].nome, "Yoshi");
    assert_eq!(p.dano_recebido.jogadores.len(), 1);
    let tank = &p.dano_recebido.jogadores[0];
    assert_eq!(tank.mortes, 1);
    assert_eq!(tank.segurando_aggro, 0); // morto não segura aggro
}

#[test]
fn dot_de_mob_com_skill_de_jogador_nao_tira_do_dps_o_dano_no_mob() {
    // World boss de 2026-10-03: o Círculo de Proteção do Chanter chega como DoT do boss no jogador.
    const BOSS: u32 = 21799;
    let mut m = Medidor::default();
    m.registrar(golpe_em(BOSS, 11174, 14340000, 100), T0);
    m.registrar(EventoDano { periodico: true, ..golpe_em(11174, BOSS, 18730002, 700) }, T0 + segundos(1.0));
    m.registrar(golpe_em(BOSS, 11174, 14340000, 200), T0 + segundos(2.0));

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    quase_igual(300.0, p.dano.jogadores[0].total);
    assert_eq!(p.dano_recebido.jogadores.len(), 1);
    assert_eq!(p.dano_recebido.jogadores[0].id, 11174);
    quase_igual(700.0, p.dano_recebido.jogadores[0].total);
}

#[test]
fn abate_sem_servidor_nao_faz_mob_virar_jogador_nem_com_skill_de_classe() {
    const BOSS: u32 = 21799;
    let mut m = Medidor::default();
    m.registrar(golpe_em(BOSS, 11174, 14340000, 100), T0);
    m.registrar_morte(11174, BOSS, 18730002, 0, "Arma de Guerra", T0 + segundos(1.0));
    m.registrar(golpe_em(BOSS, 22222, 11010000, 200), T0 + segundos(2.0));

    let p = m.obter_placar();
    assert!(p.dano.jogadores.iter().all(|j| j.nome != "Arma de Guerra"));
    quase_igual(300.0, p.dano.total);
}

#[test]
fn dono_do_marcador_so_vale_quando_e_jogador_conhecido() {
    let mut m = Medidor::default();
    m.marcar_dono(39394, 512); // espírito da Ivy
    m.marcar_dono(38784, 1_918_044); // lixo no marcador de um mob
    m.registrar(golpe(512, 16010000, 100), T0); // Ivy vira jogador conhecido
    m.registrar(golpe(39394, 16020000, 50), T0 + segundos(1.0));
    m.registrar(golpe_em(512, 38784, 1223340, 30), T0 + segundos(2.0)); // o mob bate na Ivy

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    assert_eq!(p.dano.jogadores[0].id, 512);
    quase_igual(150.0, p.dano.jogadores[0].total);
    quase_igual(30.0, p.dano_recebido.total);
}

#[test]
fn golpe_da_invocacao_antes_do_spawn_vai_para_o_dono_quando_o_vinculo_chega() {
    let mut m = Medidor::default();
    m.registrar(golpe(512, 16010000, 100), T0); // Ivy
    m.registrar(golpe(39394, 16020000, 50), T0 + segundos(1.0)); // espírito antes do spawn: linha própria
    m.registrar(golpe_em(39394, 60000, 1223340, 20), T0 + segundos(1.5)); // mob bate no espírito
    m.marcar_dono(39394, 512); // o spawn chega

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    quase_igual(150.0, p.dano.jogadores[0].total);
    assert_eq!(p.dano.jogadores[0].golpes, 2);
    assert!(p.dano_recebido.jogadores.is_empty());
}

#[test]
fn armadilha_reconhecida_como_voce_antes_do_vinculo_devolve_o_voce_ao_dono() {
    let mut m = Medidor::default();
    m.carregar_memoria(Some("Yoshi".into()), []);
    m.registrar(golpe(11174, 14340000, 100), T0); // você, ainda sem nome nesta conexão
    m.registrar_morte(MOB, 57692, 14170001, 2401, "Yoshi", T0 + segundos(1.0)); // armadilha mata: traz o seu nome
    m.registrar(golpe(57692, 14170001, 40), T0 + segundos(2.0));
    m.marcar_dono(57692, 11174); // o spawn chega

    let p = m.obter_placar();
    assert_eq!(p.dano.jogadores.len(), 1);
    let eu = &p.dano.jogadores[0];
    assert_eq!(eu.id, 11174);
    assert_eq!(eu.nome, "Yoshi");
    assert!(eu.voce);
    quase_igual(140.0, eu.total);
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
    m.registrar_morte(MOB, 11174, 14020000, 2401, "Yoshi", T0 + segundos(1.0)); // nome chega pelo abate

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
fn esquecer_memoria_apaga_os_perfis_e_o_seu_nome_e_volta_a_guardar() {
    let mut m = Medidor::default();
    m.carregar_memoria(Some("Fulano".into()), [("Beltrano".to_string(), PerfilJogador { nivel: 44, poder: 1000 })]);
    m.registrar(golpe(16173, 17010000, 100), T0);
    m.definir_jogador(16173, "Beltrano", 0, false); // nome sem nível: o nível viria da memória

    m.esquecer_memoria();
    let (eu, perfis) = m.exportar_memoria();
    assert_eq!(eu, None);
    assert!(perfis.is_empty());
    assert_eq!(m.obter_placar().dano.jogadores[0].nivel, 0);

    m.definir_jogador(16173, "Beltrano", 45, false);
    assert_eq!(m.exportar_memoria().1["Beltrano"], PerfilJogador { nivel: 45, poder: 0 });
}

#[test]
fn lista_de_chefes_fica_guardada_por_regiao() {
    let lista = |regiao: u32, vivo| ChefesDeCampo {
        regiao,
        chefes: vec![ChefeDeCampo { id: regiao * 100 + 1, vivo, posicao: None, hora_ms: 0 }],
    };
    let mut m = Medidor::default();
    m.registrar_chefes_de_campo(lista(1110, false), T0);
    m.registrar_chefes_de_campo(lista(1120, true), T0 + segundos(5.0));
    m.registrar_chefes_de_campo(lista(1110, true), T0 + segundos(9.0));
    // É do mundo: troca de conexão e Zerar não apagam.
    m.nova_conexao();
    m.reiniciar();

    assert_eq!(m.chefes_de_campo.as_ref().map(|(l, _)| l.regiao), Some(1110));
    assert_eq!(m.chefes_por_regiao.len(), 2);
    assert!(m.chefes_por_regiao[&1110].0.chefes[0].vivo); // a mais nova da região
    assert_eq!(m.chefes_por_regiao[&1120].1, T0 + segundos(5.0));
}

#[test]
fn login_desta_conexao_nao_e_trocado_por_nome_guardado() {
    let mut m = Medidor::default();
    m.carregar_memoria(Some("Yoshi".into()), []);
    m.definir_jogador(500, "Yoshi", 31, true); // 0x3633
    m.registrar(golpe(500, 14340000, 100), T0);
    m.registrar(golpe(600, 14340000, 100), T0);
    m.registrar_morte(MOB, 600, 14020000, 2401, "Yoshi", T0 + segundos(1.0)); // outro id com o mesmo nome

    let p = m.obter_placar();
    assert!(p.dano.jogadores.iter().find(|j| j.id == 500).unwrap().voce);
    assert!(!p.dano.jogadores.iter().find(|j| j.id == 600).unwrap().voce);
}

#[test]
fn so_o_meu_dano_deixa_so_a_sua_linha_com_a_porcentagem_sobre_voce() {
    let mut m = Medidor::default();
    m.definir_jogador(11174, "Yoshi", 31, true);
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar(golpe(22222, 11010000, 300), T0 + segundos(1.0));

    let dano = m.obter_placar().dano;
    quase_igual(0.25, dano.jogadores.iter().find(|j| j.voce).unwrap().porcentagem);

    let so_voce = dano.so_voce();
    assert_eq!(so_voce.jogadores.len(), 1);
    assert_eq!(so_voce.jogadores[0].nome, "Yoshi");
    quase_igual(100.0, so_voce.total);
    quase_igual(1.0, so_voce.jogadores[0].porcentagem);

    // Sem você reconhecido (overlay aberto no meio da sessão, antes de um abate): tabela vazia.
    let mut sem_voce = Medidor::default();
    sem_voce.registrar(golpe(22222, 11010000, 300), T0);
    let vazia = sem_voce.obter_placar().dano.so_voce();
    assert!(vazia.jogadores.is_empty());
    assert_eq!(vazia.total, 0.0);
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

#[test]
fn luta_que_acaba_vai_para_o_historico_e_luta_vazia_nao() {
    let mut m = Medidor::default();
    m.inatividade = segundos(15.0);
    m.reiniciar(); // "Zerar" sem luta: nada a guardar
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar(golpe(11174, 14340000, 50), T0 + segundos(2.0));
    m.registrar(golpe(11174, 14340000, 70), T0 + segundos(20.0)); // inatividade: luta nova
    m.reiniciar(); // "Zerar"

    let lutas = m.lutas_passadas();
    assert_eq!(lutas.len(), 2);
    assert_eq!(lutas[0].numero, 2); // a mais nova primeiro
    quase_igual(70.0, lutas[0].placar.dano.total);
    assert_eq!(lutas[1].numero, 1);
    quase_igual(150.0, lutas[1].placar.dano.total);
    assert_eq!(lutas[1].fim - lutas[1].inicio, segundos(2.0));
    assert!(m.obter_placar().dano.jogadores.is_empty());
}

#[test]
fn historico_guarda_so_as_ultimas_lutas() {
    let mut m = Medidor::default();
    for i in 0..LUTAS_GUARDADAS + 5 {
        m.registrar(golpe(11174, 14340000, 100), T0 + segundos(i as f64));
        m.reiniciar();
    }
    let lutas = m.lutas_passadas();
    assert_eq!(lutas.len(), LUTAS_GUARDADAS);
    assert_eq!(lutas[0].numero, LUTAS_GUARDADAS as u64 + 5);
    assert_eq!(lutas[LUTAS_GUARDADAS - 1].numero, 6);
}

#[test]
fn luta_acaba_quando_todos_os_mobs_dela_saem_de_combate() {
    const OUTRO_MOB: u32 = 46028;
    let mut m = Medidor::default();
    m.definir_jogador(11174, "Yoshi", 31, true);
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar_estado_combate(MOB, true, T0 + segundos(0.05));
    m.registrar(golpe_em(OUTRO_MOB, 11174, 14340000, 200), T0 + segundos(1.0));
    m.registrar_estado_combate(11174, false, T0 + segundos(1.5)); // o seu estado não conta
    m.registrar_estado_combate(MOB, false, T0 + segundos(2.0)); // OUTRO_MOB, sem estado visto, segue em combate
    m.registrar(golpe_em(OUTRO_MOB, 11174, 14340000, 300), T0 + segundos(3.0));
    m.registrar_morte(OUTRO_MOB, 11174, 14020000, 2401, "Yoshi", T0 + segundos(4.0)); // o último mob morre
    m.registrar(golpe_em(OUTRO_MOB, 11174, 14340000, 40), T0 + segundos(4.3)); // golpe atrasado: ainda é dela
    m.registrar(golpe_em(11174, 11174, 17120000, 500), T0 + segundos(6.0)); // cura depois: não abre luta
    assert!(m.lutas_passadas().is_empty());
    quase_igual(640.0, m.obter_placar().dano.total);
    assert!(m.obter_placar().cura.jogadores.is_empty());

    m.registrar(golpe_em(50000, 11174, 14340000, 70), T0 + segundos(7.0)); // mob novo: outra luta
    assert_eq!(m.lutas_passadas().len(), 1);
    quase_igual(640.0, m.lutas_passadas()[0].placar.dano.total);
    quase_igual(70.0, m.obter_placar().dano.total);
}

#[test]
fn boss_que_volta_ao_combate_depois_de_morrer_e_mob_sumido_nao_seguram_a_luta() {
    // World boss de 2026-10-03: o 0x8D21 de "em combate" chegou 0,04 s depois da morte, e mobs que
    // entraram em combate e saíram da visão nunca mandaram o 0.
    const BOSS: u32 = 21799;
    const SUMIDO: u32 = 5382;
    let mut m = Medidor::default();
    m.registrar(golpe_em(SUMIDO, 11174, 14340000, 10), T0);
    m.registrar_estado_combate(SUMIDO, true, T0 + segundos(0.05));
    m.registrar(golpe_em(BOSS, 11174, 14340000, 100), T0 + segundos(1.0));
    m.registrar(golpe_em(BOSS, 11174, 14340000, 100), T0 + segundos(12.0)); // mais de 5 s sem nada no SUMIDO
    m.registrar(golpe_em(BOSS, 11174, 14340000, 100), T0 + segundos(24.0));
    m.registrar_morte(BOSS, 11174, 14020000, 2401, "Yoshi", T0 + segundos(24.5));
    m.registrar_estado_combate(BOSS, true, T0 + segundos(24.54));
    m.registrar(golpe_em(50000, 11174, 14340000, 70), T0 + segundos(27.0));

    assert_eq!(m.lutas_passadas().len(), 1);
    quase_igual(310.0, m.lutas_passadas()[0].placar.dano.total);
}

#[test]
fn prazo_para_matar_vale_ate_o_chefe_sair_de_combate_ou_morrer() {
    const CHEFE: u32 = 35518;
    let em_ms = |hora: i64| (hora / 10_000) as u64;
    let mut m = Medidor::default();
    m.inatividade = i64::MAX;
    m.fim_pelo_combate = false;
    m.registrar(golpe_em(CHEFE, 11174, 14340000, 100), T0);
    m.registrar_estado_combate(CHEFE, true, T0);
    m.registrar_prazo(CHEFE, em_ms(T0 + segundos(300.0)), T0);
    assert_eq!(m.obter_placar().alvo.and_then(|a| a.prazo), Some(T0 + segundos(300.0)));

    // Saiu de combate (largou a luta): o prazo some até o próximo.
    m.registrar_estado_combate(CHEFE, false, T0 + segundos(10.0));
    assert_eq!(m.obter_placar().alvo.and_then(|a| a.prazo), None);

    // Prazo vencido ou absurdo não entra.
    m.registrar_prazo(CHEFE, em_ms(T0 + segundos(5.0)), T0 + segundos(11.0));
    m.registrar_prazo(CHEFE, u64::MAX, T0 + segundos(11.0));
    m.registrar_prazo(CHEFE, em_ms(T0 + segundos(7200.0)), T0 + segundos(11.0));
    assert_eq!(m.obter_placar().alvo.and_then(|a| a.prazo), None);

    m.registrar_estado_combate(CHEFE, true, T0 + segundos(60.0));
    m.registrar_prazo(CHEFE, em_ms(T0 + segundos(360.0)), T0 + segundos(60.0));
    assert_eq!(m.obter_placar().alvo.and_then(|a| a.prazo), Some(T0 + segundos(360.0)));
    m.registrar_morte(CHEFE, 11174, 14020000, 2401, "Yoshi", T0 + segundos(90.0));
    assert_eq!(m.obter_placar().alvo.and_then(|a| a.prazo), None);
}

#[test]
fn mob_que_volta_ao_combate_mantem_a_luta() {
    let mut m = Medidor::default();
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar_estado_combate(MOB, false, T0 + segundos(1.0)); // largou a luta
    m.registrar_estado_combate(MOB, true, T0 + segundos(2.0)); // puxado de novo
    m.registrar(golpe(11174, 14340000, 100), T0 + segundos(3.5));
    assert!(m.lutas_passadas().is_empty());
    quase_igual(200.0, m.obter_placar().dano.total);

    // Sem o fim pelo combate (replay), só a inatividade fecha a luta.
    let mut replay = Medidor::default();
    replay.fim_pelo_combate = false;
    replay.registrar(golpe(11174, 14340000, 100), T0);
    replay.registrar_estado_combate(MOB, false, T0 + segundos(1.0));
    replay.registrar(golpe_em(50000, 11174, 14340000, 100), T0 + segundos(3.0));
    assert!(replay.lutas_passadas().is_empty());
    quase_igual(200.0, replay.obter_placar().dano.total);
}

#[test]
fn tempo_de_buff_ativo_na_luta() {
    let buff = |alvo, instancia, codigo, duracao_ms| Buff { alvo, instancia, codigo, duracao_ms };
    let mut m = Medidor::default();
    m.definir_jogador(11174, "Yoshi", 31, true);
    m.registrar(golpe(11174, 14340000, 100), T0);
    // Dois Chanters dando o mesmo buff com sobreposição: 2 a 6 s, junto, e não 8 s.
    m.registrar_buff(buff(11174, 1, 181900011, 2000), T0 + segundos(2.0));
    m.registrar_buff(buff(11174, 1, 181900011, 2000), T0 + segundos(3.0)); // renovação: até 5 s
    m.registrar_buff(buff(11174, 2, 181900011, 3000), T0 + segundos(3.0)); // até 6 s
    // Removido antes de vencer: 1 a 2 s.
    m.registrar_buff(buff(11174, 3, 174100011, 10000), T0 + segundos(1.0));
    m.remover_buffs(11174, &[3], T0 + segundos(2.0));
    // Fora do tempo ativo: permanente, instantâneo e efeito do sistema.
    m.registrar_buff(buff(11174, 4, 174200001, u32::MAX), T0);
    m.registrar_buff(buff(11174, 5, 174200101, 0), T0);
    m.registrar_buff(buff(11174, 6, 200, 5000), T0);
    m.registrar(golpe(11174, 14340000, 100), T0 + segundos(10.0));

    let buffs = &m.obter_placar().dano.jogadores[0].buffs;
    assert_eq!(buffs.len(), 2);
    assert_eq!(buffs[0].skill, 18190000);
    quase_igual(0.4, buffs[0].fracao);
    assert_eq!(buffs[1].skill, 17410000);
    quase_igual(0.1, buffs[1].fracao);
}

#[test]
fn golpes_pelas_costas_por_jogador_e_por_skill() {
    let mut m = Medidor::default();
    m.registrar(EventoDano { costas: true, ..golpe(11174, 14340000, 100) }, T0);
    m.registrar(golpe(11174, 14340000, 100), T0 + segundos(1.0));
    m.registrar(EventoDano { costas: true, ..golpe(11174, 14030000, 100) }, T0 + segundos(2.0));

    let yoshi = &m.obter_placar().dano.jogadores[0];
    assert_eq!((yoshi.golpes, yoshi.costas), (3, 2));
    let skill = yoshi.skills.iter().find(|s| s.skill == 14340000).unwrap();
    assert_eq!((skill.golpes, skill.costas), (2, 1));
}

#[test]
fn golpe_mostra_o_mesmo_numero_que_o_jogo() {
    // Na tela subiu 2.574; até a 0.5.1 o medidor mostrava 48,4K (× 18,82).
    let mut m = Medidor::default();
    m.definir_jogador(11174, "Yoshi", 32, true);
    m.registrar(golpe(11174, 14340000, 2574), T0);
    let yoshi = &m.obter_placar().dano.jogadores[0];
    quase_igual(2574.0, yoshi.total);
    quase_igual(2574.0, yoshi.maximo);
}

#[test]
fn alvo_da_luta_e_o_mob_que_mais_apanhou_com_hp_e_morte() {
    let mut m = Medidor::default();
    assert_eq!(m.obter_placar().alvo, None);

    m.registrar_npc(MOB, 2400425);
    m.registrar_npc(30000, 2400939);
    m.registrar(golpe_em(30000, 11174, 14340000, 300), T0);
    m.registrar(golpe(11174, 14340000, 500), T0 + segundos(1.0));
    m.registrar(golpe(22222, 11010000, 200), T0 + segundos(2.0));
    m.registrar_hp(MOB, 84_865, T0 + segundos(2.0));

    let alvo = m.obter_placar().alvo.expect("luta com alvo");
    assert_eq!((alvo.entidade, alvo.codigo), (MOB, 2400425));
    quase_igual(700.0, alvo.dano);
    assert_eq!((alvo.hp, alvo.morto), (Some(84_865), false));

    m.registrar_morte(MOB, 11174, 14340000, 2401, "Yoshi", T0 + segundos(3.0));
    assert!(m.obter_placar().alvo.unwrap().morto);
    // Id reaproveitado por outro mob: a morte e o HP do anterior não valem.
    m.registrar_npc(MOB, 2400031);
    let alvo = m.obter_placar().alvo.unwrap();
    assert_eq!((alvo.codigo, alvo.hp, alvo.morto), (2400031, None, false));
}

#[test]
fn hp_de_jogador_nao_entra_e_luta_nova_comeca_sem_alvo() {
    let mut m = Medidor::default();
    m.definir_jogador(11174, "Yoshi", 32, true);
    m.registrar_hp(11174, 3_300, T0);
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar_hp(MOB, 5_000, T0);
    assert_eq!(m.obter_placar().alvo.unwrap().hp, Some(5_000));

    m.reiniciar();
    assert_eq!(m.obter_placar().alvo, None);
    // A luta que acabou guarda o alvo dela.
    assert_eq!(m.lutas_passadas()[0].placar.alvo.as_ref().map(|a| a.entidade), Some(MOB));
}

#[test]
fn hp_maximo_do_spawn_e_derrota_pela_queda_do_hp() {
    let mut m = Medidor::default();
    m.registrar_npc(MOB, 2400425);
    m.registrar_hp_do_spawn(MOB, 150_000_000, 160_000_000);
    m.registrar(golpe(11174, 14340000, 500), T0);
    // Antes do primeiro 0x8D00, a barra usa o HP do spawn; sem leituras, sem estimativa.
    let alvo = m.obter_placar().alvo.unwrap();
    assert_eq!((alvo.hp, alvo.hp_maximo, alvo.derrota_em), (Some(150_000_000), Some(160_000_000), None));

    // Cai 1 milhão por segundo: com 4 s de leituras ainda não estima; com 10 s, 140 s até zerar.
    for s in 0..=4 {
        m.registrar_hp(MOB, 150_000_000 - s * 1_000_000, T0 + segundos(s as f64));
    }
    assert_eq!(m.obter_placar().alvo.unwrap().derrota_em, None);
    for s in 5..=10 {
        m.registrar_hp(MOB, 150_000_000 - s * 1_000_000, T0 + segundos(s as f64));
    }
    m.registrar(golpe(11174, 14340000, 500), T0 + segundos(10.0));
    let alvo = m.obter_placar().alvo.unwrap();
    assert_eq!(alvo.hp, Some(140_000_000));
    quase_igual(140.0, alvo.derrota_em.unwrap());

    // HP subindo (reset do chefe): sem estimativa. Morto: também.
    m.registrar_hp(MOB, 160_000_000, T0 + segundos(11.0));
    assert_eq!(m.obter_placar().alvo.unwrap().derrota_em, None);
    m.registrar_morte(MOB, 11174, 14340000, 2401, "Yoshi", T0 + segundos(12.0));
    assert_eq!(m.obter_placar().alvo.unwrap().derrota_em, None);

    // Id reaproveitado: o máximo do mob anterior não vale.
    m.registrar_npc(MOB, 2400031);
    assert_eq!(m.obter_placar().alvo.unwrap().hp_maximo, None);
}

#[test]
fn odyle_vem_do_login_e_muda_com_o_ticket_dela() {
    let mut m = Medidor::default();
    let ticket = |id, valor, extra| Ticket { id, valor: Some(valor), extra };
    m.registrar_tickets(vec![ticket(TICKET_ODYLE, 550, Some(300))], T0);
    // Outro ticket não mexe na Odyle; a essência OD muda a carregada.
    m.registrar_ticket(ticket(60_000_002, 6, None), T0);
    assert_eq!(m.odyle, Some((550, Some(300))));
    m.registrar_ticket(ticket(TICKET_ODYLE, 550, Some(310)), T0);
    assert_eq!(m.odyle, Some((550, Some(310))));
    // Troca de conexão (teleporte entre servidores) não apaga: o login manda de novo.
    m.nova_conexao();
    assert_eq!(m.odyle, Some((550, Some(310))));
}

#[test]
fn login_troca_a_lista_de_tickets_e_a_mudanca_troca_um() {
    let mut m = Medidor::default();
    let ticket = |id, valor| Ticket { id, valor: Some(valor), extra: None };
    m.registrar_tickets(vec![ticket(60_000_100, 7), ticket(60_001_200, 2)], T0);
    m.registrar_ticket(ticket(60_001_200, 1), T0 + segundos(60.0));
    assert_eq!(m.tickets[&60_000_100], TicketVisto { valor: Some(7), extra: None, chegou: T0 });
    assert_eq!(m.tickets[&60_001_200], TicketVisto { valor: Some(1), extra: None, chegou: T0 + segundos(60.0) });

    // Outro login (troca de personagem): a lista nova não mistura com a anterior.
    m.registrar_tickets(vec![ticket(60_000_200, 10)], T0 + segundos(120.0));
    // É do personagem: troca de conexão e Zerar não apagam.
    m.nova_conexao();
    m.reiniciar();
    assert_eq!(m.tickets.keys().copied().collect::<Vec<_>>(), [60_000_200]);
}

/// Questlog de mentira: 2400425 é o world boss (nomeado), o resto é mob comum.
fn npc_de_teste(codigo: u32) -> Option<InfoNpc> {
    let chefe = codigo == 2400425;
    Some(InfoNpc { nome: format!("NPC {codigo}"), nivel: 45, nomeado: chefe, tipo: String::new(), retrato: None })
}

#[test]
fn dps_de_cada_um_conta_do_primeiro_ao_ultimo_golpe_dele() {
    let mut m = Medidor::default();
    m.definir_invocacao(57692, 11174, "Yoshi");
    // Yoshi bate de 0 s a 10 s; a armadilha dele, em 12 s, estica o tempo do dono.
    m.registrar(golpe(11174, 14340000, 1_000), T0);
    m.registrar(golpe(11174, 14340000, 1_000), T0 + segundos(10.0));
    m.registrar(golpe(57692, 14170001, 400), T0 + segundos(12.0));
    // Quem chega no fim: 8 s a 12 s.
    m.registrar(golpe(22222, 11010000, 2_000), T0 + segundos(8.0));
    m.registrar(golpe(22222, 11010000, 2_000), T0 + segundos(12.0));

    let p = m.obter_placar();
    assert_eq!(p.duracao, segundos(12.0));
    let yoshi = p.dano.jogadores.iter().find(|j| j.id == 11174).unwrap();
    quase_igual(2_400.0 / 12.0, yoshi.por_segundo);
    let tarde = p.dano.jogadores.iter().find(|j| j.id == 22222).unwrap();
    quase_igual(4_000.0 / 4.0, tarde.por_segundo);
}

#[test]
fn guerra_com_chefe_conta_so_o_dano_no_chefe_e_o_alvo_e_ele() {
    const BOSS: u32 = 21799;
    const ADD: u32 = 30000;
    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.definir_jogador(11174, "Yoshi", 45, true);
    m.registrar_npc(BOSS, 2400425);
    m.registrar_npc(ADD, 2400939);

    // Mob em volta antes do boss, depois o boss, e você volta a bater no mob.
    m.registrar(golpe_em(ADD, 11174, 14340000, 1_000), T0);
    m.registrar(golpe_em(BOSS, 11174, 14340000, 300), T0 + segundos(2.0));
    m.registrar(golpe_em(BOSS, 22222, 11010000, 500), T0 + segundos(4.0));
    m.registrar(golpe_em(ADD, 11174, 14340000, 2_000), T0 + segundos(6.0));
    m.registrar(
        EventoDano { alvo_id: 11174, autor_id: ADD, skill: 1223380, dano: 70, tipo_dano: 2, ..Default::default() },
        T0 + segundos(6.0),
    );

    let p = m.obter_placar();
    assert!(p.so_chefe);
    // O DPS é só o dano no boss, e o relógio da luta conta do primeiro golpe nele (2 s → 6 s).
    quase_igual(800.0, p.dano.total);
    assert_eq!(p.duracao, segundos(4.0));
    let yoshi = p.dano.jogadores.iter().find(|j| j.voce).unwrap();
    quase_igual(300.0, yoshi.total);
    // Um golpe só no boss (1 s mínimo): os golpes no mob em 0 s e 6 s não esticam o tempo dele.
    quase_igual(300.0 / 1.0, yoshi.por_segundo);
    // O alvo é o boss, mesmo com o seu último golpe no mob.
    let alvo = p.alvo.unwrap();
    assert_eq!((alvo.entidade, alvo.chefe), (BOSS, true));
    quase_igual(800.0, alvo.dano);
    quase_igual(300.0, alvo.meu_dano);
    // O Tank não muda.
    quase_igual(70.0, p.dano_recebido.total);
}

#[test]
fn sem_chefe_soma_tudo_e_o_alvo_e_o_ultimo_mob_em_que_voce_bateu() {
    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.definir_jogador(11174, "Yoshi", 45, true);
    m.registrar(golpe_em(30000, 22222, 11010000, 5_000), T0); // outro jogador, o mais batido
    m.registrar(golpe_em(30001, 11174, 14340000, 100), T0 + segundos(1.0));
    m.registrar(golpe_em(30002, 11174, 14340000, 200), T0 + segundos(2.0));

    let p = m.obter_placar();
    assert!(!p.so_chefe);
    quase_igual(5_300.0, p.dano.total);
    let alvo = p.alvo.unwrap();
    assert_eq!(alvo.entidade, 30002);
    quase_igual(200.0, alvo.meu_dano);
}

#[test]
fn morte_do_chefe_fecha_a_luta_mesmo_com_mobs_em_volta_apanhando() {
    const BOSS: u32 = 21799;
    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.definir_jogador(11174, "Yoshi", 45, true);
    m.registrar_npc(BOSS, 2400425);
    m.registrar_npc(30000, 2400939);

    m.registrar(golpe_em(BOSS, 11174, 14340000, 300), T0);
    m.registrar(golpe_em(30000, 22222, 11010000, 50), T0 + segundos(5.0)); // mob em volta, em combate
    m.registrar_morte(BOSS, 11174, 14340000, 2401, "Yoshi", T0 + segundos(10.0));
    m.registrar(golpe_em(BOSS, 22222, 11010000, 40), T0 + segundos(10.5)); // golpe atrasado: conta
    m.registrar(golpe_em(30000, 22222, 11010000, 60), T0 + segundos(12.0)); // depois: luta nova

    let passada = &m.lutas_passadas()[0].placar;
    assert!(passada.so_chefe);
    quase_igual(340.0, passada.dano.total);
    assert!(passada.alvo.as_ref().unwrap().morto);
    let agora = m.obter_placar();
    assert!(!agora.so_chefe);
    quase_igual(60.0, agora.dano.total);
}

#[test]
fn mob_batido_por_multidao_e_chefe_mesmo_sem_nome() {
    // Boss que já estava na tela quando o Axon abriu: sem spawn, sem código, sem nome.
    const BOSS: u32 = 21799;
    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.registrar(golpe_em(30000, 1000, 11010000, 5_000), T0);
    for jogador in 1000..1029 {
        m.registrar(golpe_em(BOSS, jogador, 11010000, 10), T0 + segundos(1.0));
    }
    assert!(!m.obter_placar().so_chefe); // 29 jogadores: ainda não

    m.registrar(golpe_em(BOSS, 1029, 11010000, 10), T0 + segundos(2.0));
    let p = m.obter_placar();
    assert!(p.so_chefe);
    quase_igual(300.0, p.dano.total);
    let alvo = p.alvo.unwrap();
    assert_eq!((alvo.entidade, alvo.codigo, alvo.chefe), (BOSS, 0, true));
}

#[test]
fn groggy_do_chefe_quebra_conta_e_volta_com_a_barra_cheia() {
    const CHEFE: u32 = 35518;
    let mut m = Medidor::default();
    m.inatividade = i64::MAX;
    m.fim_pelo_combate = false;
    m.registrar(golpe_em(CHEFE, 11174, 14340000, 100), T0);
    m.registrar_estado_combate(CHEFE, true, T0);
    let valor = |atual| BarraGroggy::Valor { entidade: CHEFE, maximo: 1200, atual };
    let groggy = |m: &Medidor| m.obter_placar().alvo.and_then(|a| a.groggy);
    assert_eq!(groggy(&m), None); // chefe sem 0xE005: nada

    m.registrar_barra_groggy(valor(1200), T0);
    m.registrar_barra_groggy(valor(6), T0 + segundos(45.0));
    assert_eq!(groggy(&m), Some(Groggy { atual: 6, maximo: 1200, quebrou_em: None, fim: None, quebras: 0 }));

    // Quebrou; o buff do groggy chega 0,05 s depois e diz quanto dura.
    let t = T0 + segundos(46.0);
    m.registrar_barra_groggy(BarraGroggy::Quebrou { entidade: CHEFE }, t);
    m.registrar_fim_groggy(CHEFE, 5000, t + segundos(0.05));
    let quebrado = Groggy { atual: 0, maximo: 1200, quebrou_em: Some(t), fim: Some(t + segundos(5.05)), quebras: 1 };
    assert_eq!(groggy(&m), Some(quebrado));

    // Barra cheia: o groggy acabou; a conta de quebras fica.
    m.registrar_barra_groggy(valor(1200), t + segundos(5.15));
    assert_eq!(groggy(&m), Some(Groggy { atual: 1200, maximo: 1200, quebrou_em: None, fim: None, quebras: 1 }));

    // Saiu de combate: some até o próximo 0xE005, que começa a conta de novo.
    m.registrar_estado_combate(CHEFE, false, t + segundos(10.0));
    assert_eq!(groggy(&m), None);
    m.registrar_barra_groggy(valor(1200), t + segundos(10.05));
    assert_eq!(groggy(&m).map(|g| g.quebras), Some(0));

    // A morte apaga.
    m.registrar_morte(CHEFE, 11174, 14020000, 2401, "Fulano", t + segundos(20.0));
    assert_eq!(groggy(&m), None);

    // Conexão nova também (os ids mudam).
    m.registrar_barra_groggy(valor(900), t + segundos(30.0));
    m.nova_conexao();
    m.registrar(golpe_em(CHEFE, 11174, 14340000, 100), t + segundos(31.0));
    assert_eq!(groggy(&m), None);
}

#[test]
fn buff_do_groggy_sem_barra_e_barra_de_jogador_sao_ignorados() {
    let mut m = Medidor::default();
    m.definir_jogador(11174, "Fulano", 31, true);
    m.registrar(golpe(11174, 14340000, 100), T0);
    m.registrar_fim_groggy(MOB, 5000, T0); // mob sem 0xE005
    m.registrar_barra_groggy(BarraGroggy::Valor { entidade: 11174, maximo: 1200, atual: 600 }, T0);
    assert_eq!(m.obter_placar().alvo.and_then(|a| a.groggy), None);
}

#[test]
fn mob_com_barra_de_groggy_ou_prazo_conta_como_chefe() {
    // Como o 35518 de 2026-10-06: sem spawn e com poucos atacantes, mas com 0xE005 e prazo.
    const CHEFE: u32 = 35518;
    const OUTRO_CHEFE: u32 = 35600;
    let mut m = Medidor::default();
    m.inatividade = i64::MAX;
    m.fim_pelo_combate = false;
    m.definir_jogador(11174, "Fulano", 31, true);
    m.registrar(golpe_em(CHEFE, 22222, 11010000, 500), T0);
    m.registrar(golpe_em(MOB, 11174, 14340000, 100), T0 + segundos(1.0)); // você num add
    let alvo = |m: &Medidor| m.obter_placar().alvo.map(|a| (a.entidade, a.chefe));
    assert_eq!(alvo(&m), Some((MOB, false)));

    m.registrar_barra_groggy(BarraGroggy::Valor { entidade: CHEFE, maximo: 1200, atual: 1200 }, T0 + segundos(2.0));
    assert_eq!(alvo(&m), Some((CHEFE, true)));
    // Saiu de combate e morreu: a barra some, mas ele continua chefe desta luta.
    m.registrar_estado_combate(CHEFE, false, T0 + segundos(3.0));
    m.registrar_morte(CHEFE, 11174, 14020000, 2401, "Fulano", T0 + segundos(4.0));
    assert_eq!(alvo(&m), Some((CHEFE, true)));

    // O prazo do 0x8D21 também marca.
    let mut m2 = Medidor::default();
    m2.registrar(golpe_em(OUTRO_CHEFE, 22222, 11010000, 500), T0);
    m2.registrar_prazo(OUTRO_CHEFE, (T0 + segundos(300.0)) as u64 / 10_000, T0);
    assert_eq!(alvo(&m2), Some((OUTRO_CHEFE, true)));
    // Id reaproveitado por outro mob: a marca não passa para ele.
    m2.registrar_npc(OUTRO_CHEFE, 2400031);
    assert_eq!(alvo(&m2), Some((OUTRO_CHEFE, false)));
}

#[test]
fn morte_com_a_luta_fechada_gera_relatorio_e_golpe_depois_fica_de_fora() {
    const EU: u32 = 11174;
    let mut m = Medidor::default();
    m.definir_jogador(EU, "Fulano", 31, true);
    m.registrar(golpe_em(EU, MOB, 1235330, 700), T0);
    m.registrar_hp(EU, 100, T0);
    // O mob saiu de combate: a luta acabou, e a morte chega 2 s depois.
    m.registrar_estado_combate(MOB, false, T0 + segundos(1.0));
    m.registrar_morte(EU, MOB, 1235330, 0, "", T0 + segundos(3.0));
    m.registrar(golpe_em(EU, MOB, 1235330, 500), T0 + segundos(3.1)); // depois do 0x8D04

    let r = m.ultima_morte().expect("relatório");
    assert_eq!(r.linhas.len(), 1);
    assert_eq!((r.dano_final, r.recebido, r.linhas[0].hp_depois), (Some(700), 700, Some(100)));
    assert_eq!(r.numero, 1);
    // A luta já tinha fechado: a morte fica fora dela.
    assert!(m.lutas_passadas()[0].placar.mortes.is_empty());
}

#[test]
fn invocacao_morta_nao_gera_relatorio_e_conexao_nova_limpa_o_buffer() {
    const EU: u32 = 11174;
    let mut m = Medidor::default();
    m.definir_jogador(EU, "Fulano", 31, true);
    m.definir_invocacao(57692, EU, "Fulano");
    m.registrar(golpe_em(57692, MOB, 1235300, 900), T0);
    m.registrar_morte(57692, MOB, 1235300, 0, "", T0 + segundos(1.0));
    assert!(m.ultima_morte().is_none());

    m.registrar(golpe_em(EU, MOB, 1235300, 900), T0 + segundos(2.0));
    m.nova_conexao();
    m.definir_jogador(EU, "Fulano", 31, true);
    m.registrar_morte(EU, MOB, 1235300, 0, "", T0 + segundos(3.0));
    let r = m.ultima_morte().expect("relatório");
    assert!(r.linhas.is_empty());
    // Sem golpe no buffer: matador e skill do 0x8D04, sem o dano.
    assert_eq!((r.matador, r.skill_final, r.dano_final), (MOB, 1235300, None));
}

#[test]
fn morte_em_pvp_tem_matador_e_skill_sem_o_dano() {
    const EU: u32 = 11174;
    const OUTRO: u32 = 22222;
    let mut m = Medidor::default();
    m.definir_jogador(EU, "Fulano", 31, true);
    m.definir_jogador(OUTRO, "Beltrano", 31, false);
    m.registrar(golpe_em(EU, MOB, 1235300, 900), T0);
    m.registrar(golpe_em(EU, OUTRO, 11010000, 3000), T0 + segundos(1.0));
    m.registrar_morte(EU, OUTRO, 11010000, 2401, "Beltrano", T0 + segundos(1.1));

    let r = m.ultima_morte().expect("relatório");
    assert!(r.matador_jogador);
    assert_eq!((r.nome_matador.as_str(), r.dano_final), ("Beltrano", None));
    // O golpe do jogador vira efeito sem valor; o do mob, de antes, não é tomado como golpe final.
    assert_eq!(r.linhas.iter().map(|l| l.valor).collect::<Vec<_>>(), [Some(900), None]);
    assert!(r.linhas.iter().all(|l| !l.golpe_final));
    assert_eq!(r.recebido, 900);
}

#[test]
fn buffer_guarda_so_os_128_golpes_mais_novos() {
    const EU: u32 = 11174;
    let mut m = Medidor::default();
    m.definir_jogador(EU, "Fulano", 31, true);
    m.janela_morte = segundos(30.0);
    for i in 0..200 {
        m.registrar(golpe_em(EU, MOB, 1235300, 1 + i), T0 + segundos(i as f64 * 0.1));
    }
    m.registrar_morte(EU, MOB, 1235300, 0, "", T0 + segundos(20.0));
    let r = m.ultima_morte().expect("relatório");
    assert_eq!(r.linhas.len(), 128);
    assert_eq!(r.linhas[0].valor, Some(73)); // o 73º golpe (valor 1 + 72)
    assert_eq!(r.dano_final, Some(200));
}

#[test]
fn luta_que_acabou_com_a_morte_guarda_o_relatorio() {
    const EU: u32 = 11174;
    let mut m = Medidor::default();
    m.definir_jogador(EU, "Fulano", 31, true);
    m.registrar(golpe_em(EU, MOB, 1235300, 900), T0);
    m.registrar_morte(EU, MOB, 1235300, 0, "", T0 + segundos(0.5));
    m.reiniciar();
    let luta = &m.lutas_passadas()[0];
    assert_eq!(luta.placar.mortes.len(), 1);
    assert_eq!(luta.placar.mortes[0].dano_final, Some(900));
    // A luta nova começa sem a morte.
    assert!(m.obter_placar().mortes.is_empty());
}

#[test]
fn voce_forcado_volta_depois_da_conexao_nova() {
    const EU: u32 = 16201;
    let mut m = Medidor::default();
    m.forcar_voce(EU);
    m.nova_conexao();
    m.registrar(golpe_em(EU, MOB, 1235300, 900), T0);
    m.registrar_morte(EU, MOB, 1235300, 0, "", T0 + segundos(0.5));
    assert_eq!(m.ultima_morte().map(|r| (r.morto, r.dano_final)), Some((EU, Some(900))));
    assert!(m.obter_placar().voce_reconhecido);
}
