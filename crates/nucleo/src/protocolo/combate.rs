//! Leitura dos pacotes de combate. Layout descrito em PROTOCOLO.md, seção 3.

use super::leitor::{LeitorPacote, Resultado};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EventoDano {
    pub alvo_id: u32,
    pub autor_id: u32,
    pub skill: u32,
    pub dano: u64,
    pub tipo_dano: i32,
    pub critico: bool,
    pub costas: bool,
    pub frente: bool,
    pub aparo: bool,
    pub perfeito: bool,
    pub duplo: bool,
    pub periodico: bool,
}

const TIPO_CRITICO: i32 = 3;

/// Dano direto, opcode 0x3804. Err traz o motivo do descarte.
pub fn dano(pacote: &[u8]) -> Result<EventoDano, String> {
    let mut r = abrir_corpo(pacote)?;
    let alvo = r.ler_varint()? as u32;

    let seletor = r.ler_varint()?;
    let variante = seletor & 0x0F;
    if seletor > 255 || !(4..=7).contains(&variante) {
        return Err(format!("seletor {seletor}"));
    }

    r.ler_varint()?; // desconhecido
    // autor == alvo vale: cura em si mesmo chega assim (quem decide é o Medidor).
    let autor = r.ler_varint()? as u32;

    let skill = r.ler_u32()?;
    r.ler_u8()?; // desconhecido
    let tipo = r.ler_varint()? as i32;

    let (mut flags, mut direcao) = (0u8, 0u8);
    if variante != 4 {
        flags = r.ler_u8()?;
        r.ler_u8()?; // desconhecido
        direcao = r.ler_u8()?;
    }
    r.pular(8)?; // desconhecido

    r.ler_varint()?; // desconhecido
    let dano = r.ler_varint()?;

    Ok(EventoDano {
        alvo_id: alvo,
        autor_id: autor,
        skill,
        dano,
        tipo_dano: tipo,
        critico: tipo == TIPO_CRITICO,
        costas: direcao & 0x01 != 0,
        frente: direcao & 0x02 != 0,
        aparo: flags & 0x02 != 0,
        perfeito: flags & 0x04 != 0,
        duplo: flags & 0x08 != 0,
        periodico: false,
    })
}

/// Dano periódico (DoT), opcode 0x3805.
pub fn dano_periodico(pacote: &[u8]) -> Result<EventoDano, String> {
    let mut r = abrir_corpo(pacote)?;
    let alvo = r.ler_varint()? as u32;
    let efeito = r.ler_u8()?;
    if efeito & 0x02 == 0 {
        return Err(format!("efeito 0x{efeito:02X} sem bit de dano"));
    }

    let autor = r.ler_varint()? as u32;
    r.ler_varint()?; // desconhecido
    let skill = r.ler_u32()? / 100;
    let dano = r.ler_varint()?;

    Ok(EventoDano { alvo_id: alvo, autor_id: autor, skill, dano, periodico: true, ..Default::default() })
}

/// HP atual de uma entidade, opcode 0x8D00: (entidade, hp).
pub fn hp_restante(pacote: &[u8]) -> Option<(u32, u64)> {
    let ler = || -> Resultado<(u32, u64)> {
        let mut r = abrir_corpo(pacote)?;
        let entidade = r.ler_varint()? as u32;
        r.ler_varint()?; // desconhecido
        r.ler_varint()?; // desconhecido
        r.ler_varint()?; // desconhecido
        let hp = u64::from(r.ler_u32()?) | u64::from(r.ler_u32()?) << 32;
        Ok((entidade, hp))
    };
    ler().ok()
}

/// Jogador lido de 0x3633 ou 0x3645. Nível e poder 0 = desconhecido.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfoJogador {
    pub entidade_id: u32,
    pub nome: String,
    pub nivel: i32,
    pub poder: i32,
}

/// Outro jogador entrando no campo de visão, opcode 0x3645. Cabeçalho como em
/// [`cabecalho_jogador`]; o level fica ~1.000 bytes adiante, depois do equipamento, no bloco
/// [u16 servidor][u8 n][n × (u16 tag, u32 valor)][u32 level][u32 0][u32 power].
/// O u32 logo depois do nome NÃO é o level (deu 32 no Dacura, que é 45, e 12 na Vallaina, que é 32).
/// Power conferido no jogo: Nxhunter 909 (level 43) e LaReini 579 (level 31).
/// None quando não há entidade.
pub fn info_jogador(pacote: &[u8]) -> Option<InfoJogador> {
    let (entidade_id, nome, fim_do_nome) = cabecalho_jogador(pacote);
    let (nivel, poder) = if fim_do_nome > 0 { procurar_nivel_e_poder(pacote, fim_do_nome) } else { (0, 0) };
    (entidade_id > 0).then_some(InfoJogador { entidade_id, nome, nivel, poder })
}

/// O seu personagem, opcode 0x3633 (chega no login; não veio no teleporte). Depois do nome:
/// [u16 servidor][u32 desconhecido][u8 desconhecido][u32 level][u32 power]. Conferido uma vez
/// (level 31 e power 355 no login de 2026-10-02; o power subiu para 361 pelo 0x561C).
pub fn info_personagem(pacote: &[u8]) -> Option<InfoJogador> {
    let (entidade_id, nome, fim_do_nome) = cabecalho_jogador(pacote);
    let (mut nivel, mut poder) = (0, 0);
    if fim_do_nome > 0 && fim_do_nome + 15 <= pacote.len() {
        nivel = nivel_valido(u32_em(pacote, fim_do_nome + 7));
        poder = u32_em(pacote, fim_do_nome + 11).min(i32::MAX as u32) as i32;
    }
    (entidade_id > 0).then_some(InfoJogador { entidade_id, nome, nivel, poder })
}

/// Power mudou, opcode 0x561C: [varint entidade][u32 power]... Visto uma vez, com o seu
/// personagem (355 → 361 junto com o 0x561D, que traz o mesmo valor duas vezes sem a entidade).
/// Devolve (entidade, power).
pub fn poder(pacote: &[u8]) -> Option<(u32, i32)> {
    let ler = || -> Resultado<(u32, i32)> {
        let mut r = abrir_corpo(pacote)?;
        let entidade = r.ler_varint()? as u32;
        let poder = r.ler_u32()?.min(i32::MAX as u32) as i32;
        Ok((entidade, poder))
    };
    ler().ok().filter(|&(entidade, poder)| entidade > 0 && poder > 0)
}

/// [varint entidade][4 bytes][u8 flags (bit 0 = tem nome)][varint tamanho][nome UTF-8].
/// Devolve (entidade, nome, fim do nome), com fim = posição logo depois do nome ou 0 sem nome.
fn cabecalho_jogador(pacote: &[u8]) -> (u32, String, usize) {
    let Ok(mut r) = abrir_corpo(pacote) else { return (0, String::new(), 0) };
    let Ok(entidade) = r.ler_varint() else { return (0, String::new(), 0) };
    let entidade = entidade as u32;
    let mut ler_nome = || -> Resultado<Option<(String, usize)>> {
        r.pular(4)?; // desconhecido
        if r.ler_u8()? & 0x01 == 0 {
            return Ok(None);
        }
        let tamanho = r.ler_varint()?;
        if !(1..=72).contains(&tamanho) {
            return Ok(None);
        }
        let bruto = r.ler_bytes(tamanho as usize)?;
        Ok(Some((sem_controle(bruto), r.posicao)))
    };
    match ler_nome() {
        Ok(Some((nome, fim))) => (entidade, nome, fim),
        _ => (entidade, String::new(), 0),
    }
}

/// Varredura pelo bloco [u16 servidor 1000..9999][u8 n 1..8][n × (u16 tag 1..0xFFF, u32)][u32 level 1..99],
/// com o power 8 bytes depois do level. Achou um, e só um, em cada um dos 81 pacotes 0x3645 de
/// 2026-10-02; a 1ª tag varia (0xCD ou 0xCE).
fn procurar_nivel_e_poder(p: &[u8], desde: usize) -> (i32, i32) {
    let mut i = desde;
    while i + 3 <= p.len() {
        let servidor = u16_em(p, i);
        let n = usize::from(p[i + 2]);
        let candidato = (1000..=9999).contains(&servidor) && (1..=8).contains(&n);
        let fim = i + 3 + n * 6;
        if candidato
            && fim + 4 <= p.len()
            && (0..n).all(|k| (1..0x1000).contains(&u16_em(p, i + 3 + k * 6)))
        {
            let nivel = nivel_valido(u32_em(p, fim));
            if nivel != 0 {
                let poder = if fim + 12 <= p.len() { u32_em(p, fim + 8).min(i32::MAX as u32) as i32 } else { 0 };
                return (nivel, poder);
            }
        }
        i += 1;
    }
    (0, 0)
}

fn nivel_valido(valor: u32) -> i32 {
    if (1..=99).contains(&valor) { valor as i32 } else { 0 }
}

/// Morte de entidade, opcode 0x8D04.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Morte {
    pub morto: u32,
    pub matador: u32,
    pub skill: u32,
    /// Servidor do matador: 1000..9999 em jogador, 0 em mob (o world boss de 2026-10-03).
    pub servidor: u16,
    pub nome_matador: String,
}

/// [varint morto][u32 skill que matou][varint matador][u16 servidor][u8 tam][nome do matador][u8 tam][legião]...
/// Sem matador (invocação que expirou) vem tudo zerado. None quando não há morto.
pub fn morte(pacote: &[u8]) -> Option<Morte> {
    let mut m = Morte::default();
    let mut ler = || -> Resultado<()> {
        let mut r = abrir_corpo(pacote)?;
        m.morto = r.ler_varint()? as u32;
        m.skill = r.ler_u32()?;
        m.matador = r.ler_varint()? as u32;
        if m.matador != 0 && r.restante() >= 3 {
            m.servidor = r.ler_u16()?;
            let tamanho = usize::from(r.ler_u8()?);
            if (1..=72).contains(&tamanho) && r.restante() >= tamanho {
                m.nome_matador = sem_controle(r.ler_bytes(tamanho)?);
            }
        }
        Ok(())
    };
    let _ = ler();
    (m.morto != 0).then_some(m)
}

const TIPO_INVOCACAO: u16 = 0x5F;

/// Spawn lido de 0x3641. `invocacao` diz se é invocação, pet ou armadilha (tipo 0x5F).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spawn {
    pub entidade_id: u32,
    pub invocacao: bool,
    pub dono_id: u32,
    pub nome_dono: String,
    /// Dono do marcador `FF×8 … 07 02 01|06 [u32]`, em qualquer tipo de spawn. Ainda não conferido:
    /// o Medidor só aceita se for jogador conhecido.
    pub dono_marcado: u32,
}

/// Spawn de invocação, pet ou armadilha (0x3641 com tipo 0x5F no byte baixo da máscara).
/// A invocação dá dano com id próprio; o dono vem no bloco
/// [u32 dono][u32 legião][u16 0][u16 servidor][u8 tamanho][nome da legião UTF-8],
/// achado por varredura com todas as validações juntas (formato documentado pelo
/// A2Tools em docs/summon-attribution.md). O nome logo depois da máscara é o do dono.
/// Sem legião o bloco vem zerado e só sobra o nome do dono.
pub fn spawn_invocacao(pacote: &[u8]) -> Spawn {
    let mut s = Spawn::default();
    let mut ler = || -> Resultado<()> {
        let mut r = abrir_corpo(pacote)?;
        s.entidade_id = r.ler_varint()? as u32;
        let mascara = r.ler_u16()?;
        s.dono_marcado = procurar_marcador_dono(pacote, r.posicao, s.entidade_id);
        if mascara & 0xFF != TIPO_INVOCACAO {
            return Ok(());
        }

        if r.ler_u8()? & 0x01 != 0 {
            let tamanho = r.ler_varint()?;
            if (1..=72).contains(&tamanho) {
                s.nome_dono = sem_controle(r.ler_bytes(tamanho as usize)?);
            }
        }

        s.dono_id = procurar_bloco_dono(pacote, r.posicao, s.entidade_id);
        s.invocacao = s.dono_id != 0 || !s.nome_dono.is_empty();
        Ok(())
    };
    if ler().is_err() {
        s.invocacao = false;
    }
    s
}

fn procurar_bloco_dono(p: &[u8], desde: usize, propria_entidade: u32) -> u32 {
    let mut i = desde;
    while i + 13 <= p.len() {
        let dono = u32_em(p, i);
        let servidor = u16_em(p, i + 10);
        let tamanho = usize::from(p[i + 12]);
        let valido = dono != 0
            && dono < 1 << 24
            && dono != propria_entidade
            && u16_em(p, i + 8) == 0
            && (1000..=9999).contains(&servidor)
            && (1..=48).contains(&tamanho)
            && i + 13 + tamanho <= p.len()
            && std::str::from_utf8(&p[i + 13..i + 13 + tamanho]).is_ok_and(|legiao| !legiao.chars().any(char::is_control));
        if valido {
            return dono;
        }
        i += 1;
    }
    0
}

/// Primeiro `07 02` depois de `FF×8`, com o 3º byte 01 ou 06, seguido do u32 do dono. Armadilha do
/// Ranger (tipo 0x5F) traz 06; espírito do Elementalist (0x1F), 01. Na captura do world boss de
/// 2026-10-03, a classe da invocação bateu com a do dono em 444 de 445; outros tipos trazem lixo no
/// lugar do dono (ex.: 1.918.044.167), por isso quem confere é o Medidor.
fn procurar_marcador_dono(p: &[u8], desde: usize, propria_entidade: u32) -> u32 {
    let achar = |de: usize, alvo: &[u8]| p.get(de..)?.windows(alvo.len()).position(|w| w == alvo).map(|i| de + i);
    let Some(ffs) = achar(desde, &[0xFF; 8]) else { return 0 };
    let Some(i) = achar(ffs + 8, &[0x07, 0x02]) else { return 0 };
    if i + 7 > p.len() || !matches!(p[i + 2], 0x01 | 0x06) {
        return 0;
    }
    let dono = u32_em(p, i + 3);
    if dono == 0 || dono >= 1 << 24 || dono == propria_entidade { 0 } else { dono }
}

/// UTF-8 sem os caracteres de controle (bytes inválidos viram U+FFFD).
fn sem_controle(bruto: &[u8]) -> String {
    String::from_utf8_lossy(bruto).chars().filter(|c| !c.is_control()).collect()
}

fn u16_em(p: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([p[i], p[i + 1]])
}

fn u32_em(p: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([p[i], p[i + 1], p[i + 2], p[i + 3]])
}

/// Posiciona o leitor depois do varint de tamanho e do opcode.
fn abrir_corpo(pacote: &[u8]) -> Resultado<LeitorPacote<'_>> {
    let mut r = LeitorPacote::novo(pacote, 0);
    r.ler_varint()?;
    r.ler_u16()?;
    Ok(r)
}
