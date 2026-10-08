//! Pipeline completo: segmento TCP → escolha do fluxo do servidor → remontagem →
//! enquadramento → descompressão → eventos no Medidor.
//! Não é thread-safe: alimentar de uma thread só (ou sob Mutex).

use std::collections::HashMap;

use super::medidor::Medidor;
use crate::captura::latencia::Latencia;
use crate::captura::montador::{Entrega, MontadorTcp};
use crate::captura::segmento::SegmentoTcp;
use crate::protocolo::desempacotador::Desempacotador;
use crate::protocolo::enquadrador::Enquadrador;
use crate::protocolo::{PADRAO_HEARTBEAT, combate, opcodes, procurar};
use crate::{Hora, TICKS_POR_SEGUNDO};

const ACERTOS_PARA_TRAVAR: i32 = 3;
const SILENCIO_PARA_TROCAR: i64 = 5 * TICKS_POR_SEGUNDO;

// O começo de uma conexão nova (seu personagem, jogadores por perto) chega antes de ela ser
// reconhecida como a do jogo: os segmentos dos outros fluxos ficam guardados para reprocessar.
const BYTES_GUARDADOS_POR_FLUXO: usize = 4 * 1024 * 1024;
const ESQUECER_FLUXO_PARADO: i64 = 30 * TICKS_POR_SEGUNDO;
const INTERVALO_FAXINA: i64 = 5 * TICKS_POR_SEGUNDO;

struct Guardado {
    segmentos: Vec<(SegmentoTcp, Hora)>,
    bytes: usize,
    cheio: bool,
    comecou_com_syn: bool,
    ultimo: Hora,
}

#[derive(Default)]
pub struct Sessao {
    candidatos: HashMap<String, (i32, Hora)>,
    guardados: HashMap<String, Guardado>,
    desempacotador: Desempacotador,
    montador: MontadorTcp,
    enquadrador: Enquadrador,
    ultima_faxina: Hora,
    porta_servidor: u16,
    /// O fluxo do PC para o servidor (o inverso de `fluxo`), que alimenta o ping.
    volta: Option<String>,
    latencia: Latencia,
    /// Hora do último segmento do jogo: o ping olha os 10 s antes dela (ao vivo e no replay).
    ultimo_segmento: Hora,

    pub medidor: Medidor,
    /// Fluxo servidor → cliente em uso, no formato "ip:porta > ip:porta".
    pub fluxo: Option<String>,
    pub pacotes: u64,
}

impl Sessao {
    pub fn lacunas(&self) -> u32 {
        self.montador.lacunas
    }

    pub fn dessincronizacoes(&self) -> u32 {
        self.enquadrador.dessincronizacoes
    }

    pub fn ao_segmento(&mut self, seg: &SegmentoTcp, hora: Hora) {
        // Antes do filtro abaixo: o ACK puro do servidor também mede o ping.
        if self.fluxo.as_deref() == Some(seg.chave.as_str()) {
            self.ultimo_segmento = hora;
            if let Some(ack) = seg.ack {
                self.latencia.chegou(ack, hora);
            }
        } else if self.volta.as_deref() == Some(seg.chave.as_str()) && !seg.dados.is_empty() {
            self.latencia.enviou(seg.seq, seg.dados.len(), hora);
        }
        // ACK sem dados não leva nada ao montador e, guardado, nunca chega ao limite de bytes: a
        // memória cresceria enquanto o fluxo durasse. HTTPS e HTTP nunca são o jogo, e um download
        // grande traz 0E 00 36 por acaso a cada ~16 MB: três bastariam para tomar o lugar do jogo.
        if (seg.dados.is_empty() && !seg.syn) || [seg.porta_origem, seg.porta_destino].iter().any(|p| matches!(p, 443 | 80)) {
            return;
        }
        if self.fluxo.as_deref() != Some(seg.chave.as_str()) {
            self.guardar(seg, hora);
        }
        if self.detectar_fluxo(seg, hora) {
            return; // trocou: este segmento já entrou no reprocessamento
        }
        if self.fluxo.as_deref() == Some(seg.chave.as_str()) {
            self.processar(seg, hora);
        }
    }

    fn processar(&mut self, seg: &SegmentoTcp, hora: Hora) {
        // Depois do SYN o stream começa na fronteira de um pacote: não precisa esperar o heartbeat.
        if seg.syn {
            self.enquadrador.iniciar_conexao();
        }
        let Sessao { montador, enquadrador, desempacotador, medidor, pacotes, .. } = self;
        montador.adicionar(seg.seq, seg.syn, &seg.dados, &mut |entrega| match entrega {
            Entrega::Dados(dados) => enquadrador.adicionar(dados, &mut |bruto| {
                desempacotador.expandir(bruto, &mut |pacote| {
                    *pacotes += 1;
                    ao_pacote(medidor, pacote, hora);
                });
            }),
            Entrega::Perda => enquadrador.reiniciar(),
        });
    }

    /// O servidor de jogo manda o heartbeat 0E 00 36 cerca de 20 vezes por segundo. Trava no fluxo
    /// com 3 heartbeats. Troca na hora quando o candidato é uma conexão aberta (SYN) depois que o
    /// medidor começou e na mesma porta de servidor do fluxo atual (troca de servidor: as duas
    /// conexões vistas usavam 13328). Qualquer outro candidato só troca com 5 s de silêncio do atual,
    /// senão um programa qualquer com 0E 00 36 nos dados zeraria a luta.
    fn detectar_fluxo(&mut self, seg: &SegmentoTcp, hora: Hora) -> bool {
        if procurar(&seg.dados, &PADRAO_HEARTBEAT).is_none() {
            return false;
        }

        let acertos = self.candidatos.get(&seg.chave).map_or(0, |c| c.0) + 1;
        self.candidatos.insert(seg.chave.clone(), (acertos, hora));
        if acertos < ACERTOS_PARA_TRAVAR || self.fluxo.as_deref() == Some(seg.chave.as_str()) {
            return false;
        }

        let conexao_nova = self.guardados.get(&seg.chave).is_some_and(|g| g.comecou_com_syn)
            && seg.porta_origem == self.porta_servidor;
        if let Some(fluxo) = &self.fluxo
            && !conexao_nova
            && hora - self.candidatos.get(fluxo).map_or(0, |c| c.1) <= SILENCIO_PARA_TROCAR
        {
            return false;
        }

        self.porta_servidor = seg.porta_origem;
        self.trocar_para(&seg.chave);
        true
    }

    /// Menor ida e volta TCP até o servidor nos últimos 10 s, em ticks (`captura::latencia`).
    pub fn ping(&self) -> Option<i64> {
        self.latencia.ping(self.ultimo_segmento)
    }

    fn trocar_para(&mut self, chave: &str) {
        self.fluxo = Some(chave.to_string());
        self.volta = chave.split_once(" > ").map(|(de, para)| format!("{para} > {de}"));
        self.latencia = Latencia::default();
        self.montador = MontadorTcp::default();
        self.enquadrador = Enquadrador::default();
        // Ids de entidade valem só dentro da conexão: o mesmo personagem volta com outro id.
        self.medidor.nova_conexao();

        let Some(g) = self.guardados.remove(chave) else { return };
        for (seg, hora) in &g.segmentos {
            self.processar(seg, *hora);
        }
    }

    fn guardar(&mut self, seg: &SegmentoTcp, hora: Hora) {
        let g = self.guardados.entry(seg.chave.clone()).or_insert_with(|| Guardado {
            segmentos: Vec::new(),
            bytes: 0,
            cheio: false,
            comecou_com_syn: seg.syn,
            ultimo: hora,
        });
        g.ultimo = hora;
        if !g.cheio {
            if g.bytes + seg.dados.len() > BYTES_GUARDADOS_POR_FLUXO {
                g.cheio = true;
            } else {
                g.segmentos.push((seg.clone(), hora));
                g.bytes += seg.dados.len();
            }
        }

        if hora - self.ultima_faxina < INTERVALO_FAXINA {
            return;
        }
        self.ultima_faxina = hora;
        self.guardados.retain(|_, g| hora - g.ultimo <= ESQUECER_FLUXO_PARADO);
    }
}

fn ao_pacote(medidor: &mut Medidor, pacote: &[u8], hora: Hora) {
    let Some(op) = opcodes::ler(pacote) else { return };

    match op {
        opcodes::DANO => {
            if let Ok(dano) = combate::dano(pacote) {
                medidor.registrar(dano, hora);
            }
        }
        opcodes::DANO_PERIODICO => {
            if let Ok(dot) = combate::dano_periodico(pacote) {
                medidor.registrar(dot, hora);
            }
        }
        opcodes::INFO_PERSONAGEM => {
            if let Some(eu) = combate::info_personagem(pacote) {
                medidor.definir_jogador(eu.entidade_id, &eu.nome, eu.nivel, true);
                medidor.definir_poder(eu.entidade_id, eu.poder);
            }
        }
        opcodes::SPAWN_MOB => {
            let spawn = combate::spawn_invocacao(pacote);
            if spawn.invocacao {
                medidor.definir_invocacao(spawn.entidade_id, spawn.dono_id, &spawn.nome_dono);
            } else if spawn.entidade_id != 0 {
                medidor.esquecer_invocacao(spawn.entidade_id);
                if spawn.codigo != 0 {
                    medidor.registrar_npc(spawn.entidade_id, spawn.codigo);
                }
                if let Some((atual, maximo)) = spawn.hp {
                    medidor.registrar_hp_do_spawn(spawn.entidade_id, atual, maximo);
                }
            }
            if spawn.dono_marcado != 0 {
                medidor.marcar_dono(spawn.entidade_id, spawn.dono_marcado);
            }
        }
        opcodes::HP_RESTANTE => {
            if let Some((entidade, hp)) = combate::hp_restante(pacote) {
                medidor.registrar_hp(entidade, hp, hora);
            }
        }
        opcodes::MORTE_ENTIDADE => {
            if let Some(m) = combate::morte(pacote) {
                medidor.registrar_morte(m.morto, m.matador, m.skill, m.servidor, &m.nome_matador, hora);
            }
        }
        opcodes::PODER_JOGADOR => {
            if let Some((quem, poder)) = combate::poder(pacote) {
                medidor.definir_poder(quem, poder);
            }
        }
        opcodes::INFO_OUTROS_JOGADORES => {
            if let Some(outro) = combate::info_jogador(pacote) {
                medidor.definir_jogador(outro.entidade_id, &outro.nome, outro.nivel, false);
                medidor.definir_poder(outro.entidade_id, outro.poder);
            }
        }
        opcodes::ESTADO_COMBATE => {
            if let Some(estado) = combate::estado_combate(pacote) {
                medidor.registrar_estado_combate(estado.entidade, estado.em_combate, hora);
                if let Some(prazo_ms) = estado.prazo_ms.filter(|_| estado.em_combate) {
                    medidor.registrar_prazo(estado.entidade, prazo_ms, hora);
                }
            }
        }
        opcodes::BUFF_NOVO | opcodes::BUFF_RENOVADO => {
            if let Some(buff) = combate::buff(pacote, op == opcodes::BUFF_NOVO) {
                medidor.registrar_buff(buff, hora);
            }
        }
        opcodes::TICKETS => {
            if let Some(lista) = combate::tickets(pacote) {
                medidor.registrar_tickets(lista, hora);
            }
        }
        opcodes::CHEFES_DE_CAMPO => {
            if let Some(lista) = combate::chefes_de_campo(pacote) {
                medidor.registrar_chefes_de_campo(lista, hora);
            }
        }
        opcodes::TICKET_MUDOU => {
            if let Some(ticket) = combate::ticket_mudou(pacote) {
                medidor.registrar_ticket(ticket, hora);
            }
        }
        opcodes::BUFF_REMOVIDO => {
            if let Some((alvo, instancias)) = combate::buffs_removidos(pacote) {
                medidor.remover_buffs(alvo, &instancias, hora);
            }
        }
        _ => {}
    }
}
