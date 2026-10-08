mod comum;

use comum::{T0, segundos};
use nucleo::TICKS_POR_SEGUNDO;
use nucleo::medicao::catalogo::InfoNpc;
use nucleo::medicao::medidor::Medidor;
use nucleo::medicao::recordes::{
    self, Candidato, Leitura, MarcaDps, MarcaTempo, RECORDES_MAX, Recorde, Recordes, Recusa, SemDps, SemTempo,
    TAMANHO_MAX,
};
use nucleo::protocolo::combate::EventoDano;

const EU: u32 = 11174;
const BOSS: u32 = 21799;
const ADD: u32 = 30000;
const CODIGO: u32 = 2400425;
/// Skill de Ranger: o autor vira jogador e a classe sai dela.
const FLECHA: u32 = 14340000;

fn npc_de_teste(codigo: u32) -> Option<InfoNpc> {
    let chefe = codigo == CODIGO || codigo == 2400424;
    Some(InfoNpc { nome: format!("NPC {codigo}"), nivel: 45, nomeado: chefe, tipo: String::new(), retrato: None })
}

fn golpe(autor: u32, alvo: u32, dano: u64) -> EventoDano {
    EventoDano { alvo_id: alvo, autor_id: autor, skill: FLECHA, dano, tipo_dano: 2, ..Default::default() }
}

/// Medidor com você (Ranger) e o chefe com spawn de `hp` de 160.000.000.
fn com_chefe(hp: u64) -> Medidor {
    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.definir_jogador(EU, "Fulano", 45, true);
    m.registrar_npc(BOSS, CODIGO);
    m.registrar_hp_do_spawn(BOSS, hp, 160_000_000);
    m
}

/// Você bate de 0 s a 25 s (100.000 de dano; nenhum buraco chega aos 15 s da inatividade); o chefe
/// morre aos 30 s.
fn kill_inteiro(m: &mut Medidor) {
    for s in [0.0, 10.0, 20.0, 25.0] {
        m.registrar(golpe(EU, BOSS, 25_000), T0 + segundos(s));
    }
    m.registrar(golpe(22222, BOSS, 5_000), T0 + segundos(29.0));
    m.registrar_morte(BOSS, EU, FLECHA, 2401, "Fulano", T0 + segundos(30.0));
}

fn candidato_da_luta_aberta(m: &Medidor) -> Result<Candidato, Recusa> {
    recordes::candidato(&m.abate().expect("chefe na luta"))
}

#[test]
fn k1_kill_inteiro_tem_tempo_da_morte_e_golpe_atrasado_nao_muda() {
    let mut m = com_chefe(160_000_000);
    kill_inteiro(&mut m);
    // A luta ainda está aberta: o kill já vale.
    let c = candidato_da_luta_aberta(&m).expect("kill válido");
    assert_eq!((c.npc, c.classe, c.jogadores), (CODIGO, "Ranger", 2));
    assert_eq!(c.data, T0 / TICKS_POR_SEGUNDO + 30);
    let tempo = c.tempo.clone().expect("tempo vale");
    assert_eq!(tempo.ms, 30_000);
    let dps = c.dps.clone().expect("DPS vale");
    assert_eq!((dps.valor, dps.ativo_ms), (100_000.0 / 25.0, 25_000));

    // Golpe 0,5 s depois da morte: a duração da luta anda, o tempo do kill não.
    m.registrar(golpe(22222, BOSS, 1_000), T0 + segundos(30.5));
    m.reiniciar();
    let luta = &m.lutas_passadas()[0];
    assert_eq!(luta.placar.duracao, segundos(30.5));
    let passada = recordes::candidato(luta.abate.as_ref().expect("abate")).expect("kill válido");
    assert_eq!(passada.tempo.expect("tempo").ms, 30_000);
}

#[test]
fn k2_chefe_visto_com_86_por_cento_vale_so_o_dps() {
    let mut m = com_chefe(137_600_000);
    kill_inteiro(&mut m);
    let c = candidato_da_luta_aberta(&m).expect("kill válido");
    assert!(c.dps.is_ok());
    match c.tempo {
        Err(SemTempo::Parcial(Some(fracao))) => assert!((fracao - 0.86).abs() < 1e-9),
        outro => panic!("esperava parcial, veio {outro:?}"),
    }
}

#[test]
fn k3_chefe_sem_codigo_nao_tem_candidato() {
    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.definir_jogador(EU, "Fulano", 45, true);
    // Sem spawn: chefe só pelo prazo do 0x8D21 (o caso do #35518).
    m.registrar(golpe(EU, BOSS, 40_000), T0);
    m.registrar_prazo(BOSS, ((T0 + segundos(300.0)) / 10_000) as u64, T0);
    m.registrar(golpe(EU, BOSS, 60_000), T0 + segundos(25.0));
    m.registrar_morte(BOSS, EU, FLECHA, 2401, "Fulano", T0 + segundos(30.0));
    assert_eq!(candidato_da_luta_aberta(&m), Err(Recusa::SemCodigo));
}

#[test]
fn k4_zerar_antes_da_morte_nao_tem_candidato() {
    let mut m = com_chefe(160_000_000);
    m.registrar(golpe(EU, BOSS, 40_000), T0);
    m.registrar(golpe(EU, BOSS, 60_000), T0 + segundos(25.0));
    m.reiniciar();
    let abate = m.lutas_passadas()[0].abate.clone().expect("abate");
    assert_eq!(abate.morte, None);
    assert_eq!(recordes::candidato(&abate), Err(Recusa::SemMorte));
}

#[test]
fn k5_chefe_que_saiu_de_combate_e_voltou_nao_vale_o_tempo() {
    let mut m = com_chefe(160_000_000);
    m.registrar_npc(ADD, 2400939);
    m.registrar(golpe(EU, BOSS, 40_000), T0);
    m.registrar(golpe(EU, ADD, 1_000), T0 + segundos(1.0));
    // O chefe reseta; o add continua em combate e segura a luta.
    m.registrar_estado_combate(BOSS, false, T0 + segundos(5.0));
    m.registrar(golpe(EU, ADD, 1_000), T0 + segundos(6.0));
    m.registrar_estado_combate(BOSS, true, T0 + segundos(8.0));
    for s in [12.0, 20.0, 25.0] {
        m.registrar(golpe(EU, BOSS, 20_000), T0 + segundos(s));
    }
    m.registrar_morte(BOSS, EU, FLECHA, 2401, "Fulano", T0 + segundos(30.0));
    let c = candidato_da_luta_aberta(&m).expect("kill válido");
    assert_eq!(c.tempo, Err(SemTempo::SaiuDeCombate));
    assert!(c.dps.is_ok());

    // A saída de combate na hora da morte não conta.
    let mut m = com_chefe(160_000_000);
    for s in [0.0, 10.0, 20.0, 29.0] {
        m.registrar(golpe(EU, BOSS, 25_000), T0 + segundos(s));
    }
    m.registrar_estado_combate(BOSS, false, T0 + segundos(29.99));
    m.registrar_morte(BOSS, EU, FLECHA, 2401, "Fulano", T0 + segundos(30.0));
    assert!(candidato_da_luta_aberta(&m).expect("kill válido").tempo.is_ok());
}

#[test]
fn k6_voce_reconhecido_so_depois_da_luta_nao_vale_e_no_meio_vale() {
    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.registrar_npc(BOSS, CODIGO);
    m.registrar_hp_do_spawn(BOSS, 160_000_000, 160_000_000);
    kill_inteiro(&mut m);
    m.reiniciar();
    m.definir_jogador(EU, "Fulano", 45, true); // o login chegou depois (o caso do Axios)
    let abate = m.lutas_passadas()[0].abate.clone().expect("abate");
    assert_eq!(recordes::candidato(&abate), Err(Recusa::VoceNaoReconhecido));

    let mut m = Medidor::default();
    m.consultar_npc = npc_de_teste;
    m.registrar_npc(BOSS, CODIGO);
    m.registrar_hp_do_spawn(BOSS, 160_000_000, 160_000_000);
    m.registrar(golpe(EU, BOSS, 40_000), T0);
    m.definir_jogador(EU, "Fulano", 45, true);
    for s in [10.0, 20.0, 25.0] {
        m.registrar(golpe(EU, BOSS, 20_000), T0 + segundos(s));
    }
    m.registrar_morte(BOSS, EU, FLECHA, 2401, "Fulano", T0 + segundos(30.0));
    assert!(candidato_da_luta_aberta(&m).is_ok());
}

#[test]
fn k7_sem_golpe_seu_no_chefe_ou_pouco_tempo_ativo() {
    let mut m = com_chefe(160_000_000);
    m.registrar_npc(ADD, 2400939);
    for s in [0.0, 12.0, 24.0] {
        m.registrar(golpe(22222, BOSS, 40_000), T0 + segundos(s));
        m.registrar(golpe(EU, ADD, 1_000), T0 + segundos(s + 1.0));
    }
    m.registrar_morte(BOSS, 22222, FLECHA, 2401, "Beltrano", T0 + segundos(30.0));
    assert_eq!(candidato_da_luta_aberta(&m), Err(Recusa::SemSeuDano));

    // 10 s ativos no chefe: o tempo vale, o DPS não.
    let mut m = com_chefe(160_000_000);
    m.registrar(golpe(22222, BOSS, 40_000), T0);
    m.registrar(golpe(22222, BOSS, 40_000), T0 + segundos(10.0));
    m.registrar(golpe(EU, BOSS, 40_000), T0 + segundos(15.0));
    m.registrar(golpe(EU, BOSS, 40_000), T0 + segundos(25.0));
    m.registrar_morte(BOSS, EU, FLECHA, 2401, "Fulano", T0 + segundos(30.0));
    let c = candidato_da_luta_aberta(&m).expect("kill válido");
    assert_eq!(c.dps, Err(SemDps::AtivoCurto(10_000)));
    assert!(c.tempo.is_ok());
}

#[test]
fn k8_trinta_jogadores_no_chefe_vale_so_o_dps() {
    let mut m = com_chefe(160_000_000);
    for i in 0..28 {
        m.registrar(golpe(40_000 + i, BOSS, 100), T0 - segundos(1.0));
    }
    kill_inteiro(&mut m);
    let c = candidato_da_luta_aberta(&m).expect("kill válido");
    assert_eq!(c.jogadores, 30);
    assert_eq!(c.tempo, Err(SemTempo::Multidao(30)));
    assert!(c.dps.is_ok());
}

#[test]
fn k9_dois_chefes_na_luta_nao_tem_candidato() {
    let mut m = com_chefe(160_000_000);
    m.registrar_npc(21524, 2400424);
    m.registrar(golpe(EU, 21524, 1_000), T0 - segundos(1.0));
    kill_inteiro(&mut m);
    assert_eq!(candidato_da_luta_aberta(&m), Err(Recusa::MaisDeUmChefe(2)));
}

fn candidato(dps: f64, ms: u64, data: i64) -> Candidato {
    Candidato {
        npc: CODIGO,
        classe: "Ranger",
        data,
        jogadores: 4,
        dps: Ok(MarcaDps { valor: dps, data, ativo_ms: 100_000, jogadores: 4 }),
        tempo: Ok(MarcaTempo { ms, data, jogadores: 4, dps }),
    }
}

const DIA: i64 = 1_791_400_000;

#[test]
fn k10_primeiro_kill_vira_recorde_empate_nao_troca_e_o_resultado_guarda_o_de_antes() {
    let mut r = Recordes::default();
    let primeiro = r.registrar(&candidato(2000.0, 160_000, DIA));
    assert!(primeiro.novo_dps && primeiro.novo_tempo && primeiro.mudou);
    assert_eq!((primeiro.dps_antes.clone(), primeiro.tempo_antes.clone()), (None, None));

    let empate = r.registrar(&candidato(2000.0, 160_000, DIA + 10));
    assert!(!empate.novo_dps && !empate.novo_tempo);
    assert!(empate.mudou); // o kill conta
    assert_eq!(r.melhor(CODIGO, "Ranger").expect("recorde").dps.as_ref().map(|d| d.data), Some(DIA));

    // DPS maior e tempo pior: troca só o DPS; o resultado guarda o melhor de antes.
    let melhor = r.registrar(&candidato(2100.0, 170_000, DIA + 20));
    assert!(melhor.novo_dps && !melhor.novo_tempo);
    assert_eq!(melhor.dps_antes.map(|d| d.valor), Some(2000.0));
    assert_eq!(melhor.tempo_antes.map(|t| t.ms), Some(160_000));
    let guardado = r.melhor(CODIGO, "Ranger").expect("recorde");
    assert_eq!((guardado.kills, guardado.dps.as_ref().map(|d| d.valor)), (3, Some(2100.0)));
    assert_eq!(guardado.tempo.as_ref().map(|t| t.ms), Some(160_000));
    // Outra classe é outro recorde.
    assert!(r.melhor(CODIGO, "Cleric").is_none());

    // Nada válido: o arquivo não muda.
    let antes = r.clone();
    let nada = Candidato { dps: Err(SemDps::AtivoCurto(5_000)), tempo: Err(SemTempo::SaiuDeCombate), ..candidato(9.0, 1, DIA) };
    assert!(!r.registrar(&nada).mudou);
    assert_eq!(r, antes);
}

fn lido(texto: &str) -> (Recordes, usize, bool) {
    match recordes::ler(texto.as_bytes()) {
        Leitura::Lido { recordes, removidos, so_leitura } => (recordes, removidos, so_leitura),
        Leitura::Corrompido => panic!("lido como corrompido: {texto}"),
    }
}

#[test]
fn f5_versao_ausente_migra_e_ida_e_volta_da_o_mesmo() {
    let (r, removidos, so_leitura) = lido(
        r#"{"recordes":[{"npc":2400425,"classe":"Ranger","dps":{"valor":2310.5,"data":1791401200,"ativo_ms":148200,"jogadores":4}}]}"#,
    );
    assert_eq!((removidos, so_leitura), (0, false));
    assert_eq!(r.lista[0].kills, 0);
    let texto = recordes::texto(&r);
    assert!(texto.contains("\"versao\": 1"));
    assert_eq!(lido(&texto).0, r);
}

#[test]
fn f4_versao_mais_nova_abre_so_para_leitura() {
    let (r, _, so_leitura) =
        lido(r#"{"versao":2,"recordes":[{"npc":2400425,"classe":"Ranger","kills":1,"tempo":{"ms":151870,"data":1791487600,"jogadores":4,"dps":2204.8}}]}"#);
    assert!(so_leitura);
    assert_eq!(r.lista.len(), 1);
}

#[test]
fn f2_f3_ilegivel_ou_grande_demais_e_corrompido() {
    let inteiro = r#"{"versao":1,"recordes":[]}"#;
    for ruim in [&inteiro[..15], "", "\u{feff}", "\u{1}\u{2}lixo", "[]", r#"{"versao":"um","recordes":[]}"#] {
        assert_eq!(recordes::ler(ruim.as_bytes()), Leitura::Corrompido, "{ruim:?}");
    }
    assert_eq!(recordes::ler(&[0xFF, 0xFE, 0x00]), Leitura::Corrompido);
    let mut grande = inteiro.as_bytes().to_vec();
    grande.resize(TAMANHO_MAX + 1, b' ');
    assert_eq!(recordes::ler(&grande), Leitura::Corrompido);
    // Com BOM, como o config.json.
    assert_eq!(lido(&format!("\u{feff}{inteiro}")).0, Recordes::default());
}

#[test]
fn f6_item_invalido_sai_os_outros_ficam_e_chave_repetida_fica_o_melhor() {
    let (r, removidos, _) = lido(
        r#"{"versao":1,"recordes":[
        {"npc":2400425,"classe":"Ranger","kills":2,"dps":{"valor":2000,"data":1791401200,"ativo_ms":100000,"jogadores":4}},
        {"npc":2400425,"classe":"Ranger","kills":5,"dps":{"valor":2500,"data":1791401300,"ativo_ms":100000,"jogadores":4},
         "tempo":{"ms":150000,"data":1791401300,"jogadores":4,"dps":2500}},
        {"npc":123,"classe":"Ranger","kills":1,"dps":{"valor":1,"data":1791401200,"ativo_ms":100000,"jogadores":4}},
        {"npc":2400425,"classe":"Spirit","kills":1,"dps":{"valor":1,"data":1791401200,"ativo_ms":100000,"jogadores":4}},
        {"npc":2400425,"classe":"Cleric","kills":-1,"dps":{"valor":1,"data":1791401200,"ativo_ms":100000,"jogadores":4}},
        {"npc":2400425,"classe":"Cleric","kills":1,"dps":{"valor":-5,"data":1791401200,"ativo_ms":100000,"jogadores":4}},
        {"npc":2400425,"classe":"Cleric","kills":1,"tempo":{"ms":0,"data":1791401200,"jogadores":4,"dps":1}},
        {"npc":2400425,"classe":"Cleric","kills":1,"tempo":{"ms":90000000,"data":1791401200,"jogadores":4,"dps":1}},
        {"npc":2400425,"classe":"Cleric","kills":1,"tempo":{"ms":1000,"data":1791401200,"jogadores":0,"dps":1}},
        {"npc":2400425,"classe":"Cleric","kills":1},
        "lixo",
        {"npc":2400424,"classe":"Templar","kills":1,"tempo":{"ms":200000,"data":99,"jogadores":2,"dps":10}}
    ]}"#,
    );
    assert_eq!(removidos, 9);
    assert_eq!(r.lista.len(), 2);
    let ranger = r.melhor(2400425, "Ranger").expect("ranger");
    assert_eq!((ranger.kills, ranger.dps.as_ref().map(|d| d.valor)), (5, Some(2500.0)));
    // Data estranha não derruba o item: aparece como desconhecida.
    let templar = r.melhor(2400424, "Templar").expect("templar");
    assert!(!recordes::data_valida(templar.tempo.as_ref().expect("tempo").data, DIA));
    assert!(recordes::data_valida(DIA - 100, DIA));
    assert!(!recordes::data_valida(DIA + 100, DIA));
}

#[test]
fn f7_f8_ida_e_volta_so_com_os_campos_e_sem_nomes() {
    let mut m = com_chefe(160_000_000);
    m.definir_jogador(22222, "Beltrano", 45, false);
    kill_inteiro(&mut m);
    let mut r = Recordes::default();
    r.registrar(&candidato_da_luta_aberta(&m).expect("kill"));
    let texto = recordes::texto(&r);
    for nome in ["Fulano", "Beltrano", "11174", "21799", "22222"] {
        assert!(!texto.contains(nome), "{nome} no arquivo");
    }
    let valor: serde_json::Value = serde_json::from_str(&texto).unwrap();
    let chaves = |v: &serde_json::Value| {
        let mut chaves: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        chaves.sort();
        chaves.join(",")
    };
    assert_eq!(chaves(&valor), "recordes,versao");
    let item = &valor["recordes"][0];
    assert_eq!(chaves(item), "classe,dps,kills,npc,tempo");
    assert_eq!(chaves(&item["dps"]), "ativo_ms,data,jogadores,valor");
    assert_eq!(chaves(&item["tempo"]), "data,dps,jogadores,ms");
    assert_eq!(lido(&texto).0, r);
}

fn recorde(npc: u32, data: i64) -> Recorde {
    Recorde {
        npc,
        classe: "Ranger".into(),
        kills: 1,
        dps: Some(MarcaDps { valor: 100.0, data, ativo_ms: 30_000, jogadores: 1 }),
        tempo: None,
    }
}

#[test]
fn f11_passou_de_500_sai_o_de_data_mais_antiga() {
    let mut r = Recordes { lista: (0..RECORDES_MAX as u32).map(|i| recorde(2_000_000 + i, DIA + i64::from(i))).collect() };
    r.lista[7].dps.as_mut().unwrap().data = DIA - 1_000;
    let mut c = candidato(50.0, 1_000, DIA + 9_999);
    c.npc = 3_000_000;
    r.registrar(&c);
    assert_eq!(r.lista.len(), RECORDES_MAX);
    assert!(r.melhor(2_000_007, "Ranger").is_none());
    assert!(r.melhor(3_000_000, "Ranger").is_some());
}

#[test]
fn f12_mescla_fica_com_o_melhor_de_cada_um_e_e_idempotente() {
    let mut a = Recordes::default();
    a.registrar(&candidato(2000.0, 160_000, DIA));
    let mut b = a.clone();
    b.registrar(&candidato(1900.0, 150_000, DIA + 50)); // a outra instância: tempo melhor, DPS pior
    let mut c = candidato(10.0, 99_000, DIA);
    c.npc = 2400424;
    b.registrar(&c);

    let mut juntos = a.clone();
    juntos.mesclar(&b);
    let r = juntos.melhor(CODIGO, "Ranger").expect("recorde");
    assert_eq!((r.dps.as_ref().map(|d| d.valor), r.tempo.as_ref().map(|t| t.ms)), (Some(2000.0), Some(150_000)));
    assert_eq!(r.kills, 2);
    assert!(juntos.melhor(2400424, "Ranger").is_some());

    // Mesclar com o mesmo arquivo (cada gravação relê o disco) não muda nada.
    let antes = juntos.clone();
    juntos.mesclar(&antes);
    assert_eq!(juntos, antes);
    // Apagar tira só a chave.
    assert!(juntos.apagar(2400424, "Ranger"));
    assert!(juntos.melhor(2400424, "Ranger").is_none() && juntos.melhor(CODIGO, "Ranger").is_some());
}
