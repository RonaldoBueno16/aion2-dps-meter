//! Opcodes do cliente Global, lidos em little-endian (bytes no fio: 04 38 = 0x3804).
//! Origem: RATmeter, commit de 2026-10-01 07:33 UTC; 0x8D21 e os buffs vieram do medidor do TK
//! (Aion2-Dps-Meter, cliente coreano) e foram conferidos nas capturas do Global de 2026-10-03.
//! Revalidar a cada patch do jogo.

use super::varint;

pub const HEARTBEAT: u16 = 0x3600;
pub const HORA_SERVIDOR: u16 = 0x3603;
pub const INFO_PERSONAGEM: u16 = 0x3633;
pub const VINCULO_SESSAO: u16 = 0x3620;
pub const SPAWN_MOB: u16 = 0x3641;
pub const INFO_OUTROS_JOGADORES: u16 = 0x3645;
pub const ATRIBUTOS_JOGADOR: u16 = 0x3649;
pub const DANO: u16 = 0x3804;
pub const DANO_PERIODICO: u16 = 0x3805;
pub const BUFF_NOVO: u16 = 0x382A;
pub const BUFF_RENOVADO: u16 = 0x382B;
pub const BUFF_REMOVIDO: u16 = 0x382C;
pub const COOLDOWN_SKILL: u16 = 0x3847;
pub const HP_RESTANTE: u16 = 0x8D00;
pub const ESTADO_COMBATE: u16 = 0x8D21;
pub const MORTE_ENTIDADE: u16 = 0x8D04;
pub const GRUPO: u16 = 0x9702;
/// Chefes de campo da região (vivo, hora de renascer): achado nas capturas do Axon, 2026-10-06.
pub const CHEFES_DE_CAMPO: u16 = 0x9101;
pub const PODER_JOGADOR: u16 = 0x561C;
pub const TICKETS: u16 = 0x610B;
pub const TICKET_MUDOU: u16 = 0x610C;
pub const COMPRIMIDO: u16 = 0xFFFF;

pub fn nome(opcode: u16) -> &'static str {
    match opcode {
        HEARTBEAT => "Heartbeat",
        HORA_SERVIDOR => "HoraServidor",
        INFO_PERSONAGEM => "InfoPersonagem",
        VINCULO_SESSAO => "VinculoSessao",
        SPAWN_MOB => "SpawnMob",
        INFO_OUTROS_JOGADORES => "InfoOutrosJogadores",
        ATRIBUTOS_JOGADOR => "AtributosJogador",
        DANO => "Dano",
        DANO_PERIODICO => "DanoPeriodico",
        BUFF_NOVO => "BuffNovo",
        BUFF_RENOVADO => "BuffRenovado",
        BUFF_REMOVIDO => "BuffRemovido",
        COOLDOWN_SKILL => "CooldownSkill",
        HP_RESTANTE => "HpRestante",
        ESTADO_COMBATE => "EstadoCombate",
        MORTE_ENTIDADE => "MorteEntidade",
        GRUPO => "Grupo",
        CHEFES_DE_CAMPO => "ChefesDeCampo",
        PODER_JOGADOR => "PoderJogador",
        COMPRIMIDO => "Comprimido",
        _ => "",
    }
}

/// Lê o opcode logo depois do varint de tamanho.
pub fn ler(pacote: &[u8]) -> Option<u16> {
    let (_, n) = varint::ler(pacote, 0)?;
    if pacote.len() < n + 2 {
        return None;
    }
    Some(u16::from_le_bytes([pacote[n], pacote[n + 1]]))
}
