//! Dois modos com o mesmo núcleo do medidor:
//!   captura.pcapng [--hex N] [--op 3804] [--procurar-quedas] [--entidade ID]
//!       reprocessa uma captura: sincronização, LZ4, opcodes, conferência com HP e placar.
//!   ao-vivo [segundos]
//!       captura por raw socket (precisa de administrador) e imprime o placar.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use indexmap::IndexMap;
use nucleo::captura::montador::{Entrega, MontadorTcp};
use nucleo::captura::pcapng;
use nucleo::captura::segmento::SegmentoTcp;
use nucleo::formato::{f, hex, n, n_int, p};
use nucleo::medicao::catalogo::CatalogoSkills;
use nucleo::medicao::dados_jogo;
use nucleo::medicao::medidor::{Placar, Tabela};
use nucleo::medicao::sessao::Sessao;
use nucleo::protocolo::combate::{self, EventoDano};
use nucleo::protocolo::desempacotador::Desempacotador;
use nucleo::protocolo::enquadrador::Enquadrador;
use nucleo::protocolo::leitor::LeitorPacote;
use nucleo::protocolo::{opcodes, procurar};
use nucleo::{Hora, segundos};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arquivo_nomes = dados_jogo::carregar_nomes_padrao();
    let catalogo = dados_jogo::definir_catalogo(CatalogoSkills::novo(None));
    catalogo.esperar_pronto(Duration::from_secs(30));
    println!(
        "Nomes de skills: {} em português, {} em inglês de reserva ({})",
        catalogo.quantidade(),
        dados_jogo::nomes_carregados(),
        arquivo_nomes.map_or_else(|| "dados/skills.json não encontrado".to_string(), |c| c.display().to_string())
    );

    if args.first().map(String::as_str) == Some("ao-vivo") {
        ao_vivo(args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60));
        return;
    }

    let valor_de = |opcao: &str| args.iter().skip_while(|a| *a != opcao).nth(1).cloned();
    let (hex_opcao, op_opcao, entidade_opcao) = (valor_de("--hex"), valor_de("--op"), valor_de("--entidade"));
    let caminho = args
        .iter()
        .find(|a| {
            !a.starts_with("--")
                && Some(*a) != hex_opcao.as_ref()
                && Some(*a) != op_opcao.as_ref()
                && Some(*a) != entidade_opcao.as_ref()
        })
        .cloned()
        .unwrap_or_else(|| "captura.pcapng".into());
    let config = Config {
        op_amostra: u16::from_str_radix(op_opcao.as_deref().unwrap_or("3804"), 16).expect("--op em hexadecimal"),
        procurar_quedas: args.iter().any(|a| a == "--procurar-quedas"),
        entidade: entidade_opcao.map(|e| e.parse().expect("--entidade numérica")),
    };
    let amostras_hex: usize = hex_opcao.map_or(0, |h| h.parse().expect("--hex numérico"));

    let quadros = match pcapng::ler(Path::new(&caminho)) {
        Ok(q) => q,
        Err(erro) => {
            eprintln!("{erro}");
            std::process::exit(1);
        }
    };

    let mut streams: IndexMap<String, FluxoTcp> = IndexMap::new();
    let mut tcp = 0;
    for quadro in &quadros {
        let Some(seg) = SegmentoTcp::extrair(&quadro.dados, quadro.tipo_enlace) else { continue };
        tcp += 1;
        let s = streams.entry(seg.chave.clone()).or_insert_with(|| FluxoTcp::novo(seg.chave.clone()));
        s.analise.hora = quadro.hora;
        s.adicionar(&seg, &config);
    }

    let absoluto = std::path::absolute(&caminho).unwrap_or_else(|_| caminho.clone().into());
    println!("Arquivo: {}", absoluto.display());
    println!("Quadros: {} | TCP/IPv4: {tcp} | streams: {}", quadros.len(), streams.len());

    let mut ordem: Vec<&FluxoTcp> = streams.values().collect();
    ordem.sort_by(|a, b| b.montador.bytes_entregues.cmp(&a.montador.bytes_entregues));
    for s in ordem {
        s.relatar(&config, amostras_hex);
    }

    // Mesmo arquivo pelo caminho do medidor ao vivo (Sessao + Medidor), sem fim de luta por inatividade.
    let mut replay = Sessao::default();
    replay.medidor.inatividade = i64::MAX;
    for quadro in &quadros {
        if let Some(seg) = SegmentoTcp::extrair(&quadro.dados, quadro.tipo_enlace) {
            replay.ao_segmento(&seg, quadro.hora);
        }
    }
    println!();
    println!("=== Placar pelo pipeline do medidor (fluxo {})", replay.fluxo.as_deref().unwrap_or(""));
    imprimir_placar(&replay.medidor.obter_placar());
}

fn ao_vivo(duracao: u64) {
    let sessao = Arc::new(Mutex::new(Sessao::default()));
    let alimentar = sessao.clone();
    let captura = nucleo::captura::socket_bruto::CapturaSocketBruto::iniciar(Arc::new(move |seg, hora| {
        alimentar.lock().unwrap().ao_segmento(&seg, hora);
    }));
    let captura = match captura {
        Ok(c) => c,
        Err(erro) => {
            eprintln!("{erro}");
            std::process::exit(1);
        }
    };
    let enderecos: Vec<String> = captura.enderecos.iter().map(ToString::to_string).collect();
    println!("Capturando em {} por {duracao} s", enderecos.join(", "));

    let fim = std::time::Instant::now() + Duration::from_secs(duracao);
    while std::time::Instant::now() < fim {
        std::thread::sleep(Duration::from_secs(5));
        let s = sessao.lock().unwrap();
        let contadores = &captura.contadores;
        println!(
            "[{}] IP recebidos {}, TCP {}, fluxo {}, pacotes {}, lacunas {}, dessinc. {}, jogadores {}",
            hora_local(),
            contadores.recebidos.load(std::sync::atomic::Ordering::Relaxed),
            contadores.segmentos_tcp.load(std::sync::atomic::Ordering::Relaxed),
            s.fluxo.as_deref().unwrap_or("(procurando)"),
            s.pacotes,
            s.lacunas(),
            s.dessincronizacoes(),
            s.medidor.obter_placar().dano.jogadores.len()
        );
    }
    imprimir_placar(&sessao.lock().unwrap().medidor.obter_placar());
}

/// HH:mm:ss no fuso do Windows.
fn hora_local() -> String {
    let mut agora = unsafe { std::mem::zeroed::<windows_sys::Win32::Foundation::SYSTEMTIME>() };
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut agora) };
    format!("{:02}:{:02}:{:02}", agora.wHour, agora.wMinute, agora.wSecond)
}

fn imprimir_placar(placar: &Placar) {
    println!("Luta de {} s", n(segundos(placar.duracao), 1));
    imprimir_tabela("DPS (dano causado)", "DPS", &placar.dano);
    imprimir_tabela("Tank (dano recebido)", "DTPS", &placar.dano_recebido);
    imprimir_tabela("Healer (cura)", "HPS", &placar.cura);
}

fn imprimir_tabela(titulo: &str, por_segundo: &str, tabela: &Tabela) {
    println!("--- {titulo}: total {}", n(tabela.total, 0));
    for j in &tabela.jogadores {
        let mut perfil = j.classe.to_string();
        if j.nivel > 0 {
            perfil += &format!(" Nv {}", j.nivel);
        }
        if j.poder > 0 {
            perfil += &format!(" Power {}", j.poder);
        }
        println!(
            "{:<22} {:<12} {:>14} {:>10} {por_segundo} {:>6}  golpes {}, crítico {}{}{}{}{}",
            j.nome,
            perfil.trim(),
            n(j.total, 0),
            n(j.por_segundo, 0),
            p(j.porcentagem, 1),
            j.golpes,
            p(if j.golpes > 0 { f64::from(j.criticos) / f64::from(j.golpes) } else { 0.0 }, 0),
            if j.aparos > 0 { format!(", aparos {}", j.aparos) } else { String::new() },
            if j.mortes > 0 { format!(", mortes {}", j.mortes) } else { String::new() },
            if j.segurando_aggro > 0 { format!(", segurando {} mob(s)", j.segurando_aggro) } else { String::new() },
            if j.voce { "  (você)" } else { "" },
        );
        for s in j.skills.iter().take(10) {
            println!(
                "    {:<28} {:>14} {:>6}  {:>4}x  máx {}",
                s.nome,
                n(s.total, 0),
                p(s.porcentagem, 1),
                s.golpes,
                n(s.maximo, 0)
            );
        }
    }
}

struct Config {
    op_amostra: u16,
    procurar_quedas: bool,
    entidade: Option<u32>,
}

struct FluxoTcp {
    chave: String,
    montador: MontadorTcp,
    enquadrador: Enquadrador,
    desempacotador: Desempacotador,
    analise: Analise,
}

/// Tudo o que o relatório de um stream acumula (separado das camadas para o borrow das closures).
#[derive(Default)]
struct Analise {
    hora: Hora,
    pacotes_brutos: u32,
    pacotes: u32,
    opcodes: IndexMap<u16, i32>,
    falhas: IndexMap<String, i32>,
    eventos: Vec<(Hora, EventoDano)>,
    amostras: Vec<Vec<u8>>,
    personagens: IndexMap<u32, String>,

    // Conferência do campo de dano: a queda de HP entre dois 0x8D00 do mesmo alvo
    // comparada com a soma dos 0x3804/0x3805 nesse alvo entre eles, na ordem do stream.
    ultimo_hp: HashMap<u32, u64>,
    eventos_desde_ultimo_hp: HashMap<u32, Vec<EventoDano>>,
    conferencias: Vec<Conferencia>,

    // Diagnóstico: em que pacotes do intervalo aparece o valor exato da queda de HP?
    historico: Vec<Vec<u8>>,
    inicio_intervalo: HashMap<u32, usize>,
    procuras_queda: u32,
    inicio_diag: Option<Hora>,
}

struct Conferencia {
    alvo: u32,
    queda_hp: i64,
    dano: u64,
    eventos: Vec<EventoDano>,
}

impl FluxoTcp {
    fn novo(chave: String) -> Self {
        Self {
            chave,
            montador: MontadorTcp::default(),
            enquadrador: Enquadrador::default(),
            desempacotador: Desempacotador::default(),
            analise: Analise::default(),
        }
    }

    fn adicionar(&mut self, seg: &SegmentoTcp, config: &Config) {
        let FluxoTcp { montador, enquadrador, desempacotador, analise, .. } = self;
        montador.adicionar(seg.seq, seg.syn, &seg.dados, &mut |entrega| match entrega {
            Entrega::Dados(dados) => enquadrador.adicionar(dados, &mut |bruto| {
                analise.pacotes_brutos += 1;
                desempacotador.expandir(bruto, &mut |pacote| analise.ao_pacote(pacote, config));
            }),
            Entrega::Perda => enquadrador.reiniciar(),
        });
    }

    fn relatar(&self, config: &Config, amostras_hex: usize) {
        let a = &self.analise;
        println!();
        println!("=== Stream {}", self.chave);
        println!(
            "  TCP: {} bytes, retransmissões {}, lacunas {}",
            n_int(self.montador.bytes_entregues),
            self.montador.retransmissoes,
            self.montador.lacunas
        );
        println!(
            "  Enquadramento: sincronizado={}, dessincronizações {}, bytes descartados {}",
            if self.enquadrador.sincronizado { "True" } else { "False" },
            self.enquadrador.dessincronizacoes,
            n_int(self.enquadrador.bytes_descartados)
        );
        let d = &self.desempacotador;
        println!(
            "  Pacotes: {} no fio, {} depois de abrir {} blocos LZ4 (falhas {}, tamanho divergente {}, bytes sobrando {})",
            a.pacotes_brutos, a.pacotes, d.blocos_comprimidos, d.falhas_descompressao, d.tamanhos_divergentes, d.bytes_sobrando
        );
        if a.pacotes == 0 {
            return;
        }

        println!("  Opcodes mais frequentes:");
        for (op, quantos) in mais_frequentes(&a.opcodes).into_iter().take(25) {
            println!("    0x{op:04X} {:<20} {quantos:>6}", opcodes::nome(*op));
        }

        for (id, nome) in &a.personagens {
            println!("  InfoPersonagem: id={id} nome=\"{nome}\"");
        }

        println!("  Eventos de dano: {} válidos", a.eventos.len());
        for (motivo, quantos) in mais_frequentes(&a.falhas).into_iter().take(10) {
            println!("    descartado {quantos:>5}x  {motivo}");
        }

        let mut autores = agrupar(&a.eventos, |e| e.1.autor_id);
        autores.sort_by(|x, y| soma_dano(&y.1).total_cmp(&soma_dano(&x.1)));
        for (autor, lista) in autores {
            let total = soma_dano(&lista);
            let segundos_luta = segundos(lista[lista.len() - 1].0 - lista[0].0).max(1.0);
            let criticos = lista.iter().filter(|e| e.1.critico).count();
            let rotulo = a.personagens.get(&autor).map_or(String::new(), |nome| format!(" ({nome}, você)"));
            let alvos: HashSet<u32> = lista.iter().map(|e| e.1.alvo_id).collect();
            println!(
                "  Autor {autor}{rotulo}: {} golpes, dano {} em {} s = {} DPS, crítico {}, alvos {}",
                lista.len(),
                n(total, 0),
                n(segundos_luta, 1),
                n(total / segundos_luta, 0),
                p(criticos as f64 / lista.len() as f64, 0),
                alvos.len()
            );

            let mut skills = agrupar(&lista, |e| e.1.skill);
            skills.sort_by(|x, y| soma_dano(&y.1).total_cmp(&soma_dano(&x.1)));
            for (skill, eventos) in skills.into_iter().take(8) {
                let soma = soma_dano(&eventos);
                println!(
                    "      skill {skill:>10}: {:>4}x  {:>14}  (média {}, máx {}){}",
                    eventos.len(),
                    n(soma, 0),
                    n(soma / eventos.len() as f64, 0),
                    n_int(eventos.iter().map(|e| e.1.dano).max().unwrap_or(0)),
                    if eventos[0].1.periodico { " DoT" } else { "" }
                );
                // Diagnóstico do crítico: dentro da mesma skill, o tipo 3 deve ter média bem maior que o tipo 2.
                let mut tipos = agrupar(&eventos, |e| e.1.tipo_dano);
                tipos.sort_by_key(|t| t.0);
                let partes: Vec<String> = tipos
                    .iter()
                    .map(|(tipo, t)| format!("{tipo}: {}x média {}", t.len(), n(soma_dano(t) / t.len() as f64, 0)))
                    .collect();
                println!("        por tipo_dano: {}", partes.join("; "));
            }
        }

        a.relatar_conferencia_hp();

        for amostra in a.amostras.iter().take(amostras_hex) {
            println!("  hex 0x{:04X}: {}", config.op_amostra, hex(amostra));
        }
    }
}

impl Analise {
    fn ao_pacote(&mut self, pacote: &[u8], config: &Config) {
        self.pacotes += 1;
        let Some(op) = opcodes::ler(pacote) else { return };
        *self.opcodes.entry(op).or_insert(0) += 1;
        if op == config.op_amostra && self.amostras.len() < 50 {
            self.amostras.push(pacote.to_vec());
        }
        self.historico.push(pacote.to_vec());
        if let Some(entidade) = config.entidade {
            self.mostrar_se_envolve(entidade, op, pacote);
        }

        match op {
            opcodes::DANO => self.registrar(combate::dano(pacote), "Dano: "),
            opcodes::DANO_PERIODICO => self.registrar(combate::dano_periodico(pacote), "DoT: "),
            opcodes::INFO_PERSONAGEM => {
                if let Some(eu) = combate::info_personagem(pacote) {
                    self.personagens.insert(eu.entidade_id, eu.nome);
                }
            }
            opcodes::HP_RESTANTE => {
                if let Some((alvo, hp)) = combate::hp_restante(pacote) {
                    self.conferir_hp(alvo, hp, config);
                }
            }
            _ => {}
        }
    }

    fn registrar(&mut self, resultado: Result<EventoDano, String>, prefixo: &str) {
        match resultado {
            Ok(evento) => {
                self.eventos.push((self.hora, evento));
                self.eventos_desde_ultimo_hp.entry(evento.alvo_id).or_default().push(evento);
            }
            Err(motivo) => *self.falhas.entry(format!("{prefixo}{motivo}")).or_insert(0) += 1,
        }
    }

    fn conferir_hp(&mut self, alvo: u32, hp: u64, config: &Config) {
        let lista = self.eventos_desde_ultimo_hp.remove(&alvo).unwrap_or_default();
        if let Some(&anterior) = self.ultimo_hp.get(&alvo) {
            let dano = lista.iter().fold(0.0, |s, e| s + e.dano as f64) as u64;
            let queda = (anterior as i64).wrapping_sub(hp as i64);
            if config.procurar_quedas && queda > 0 && self.procuras_queda < 15 {
                self.procurar_queda(alvo, queda, self.inicio_intervalo.get(&alvo).copied().unwrap_or(0));
            }
            if queda != 0 || dano != 0 {
                self.conferencias.push(Conferencia { alvo, queda_hp: queda, dano, eventos: lista });
            }
        }
        self.ultimo_hp.insert(alvo, hp);
        self.eventos_desde_ultimo_hp.insert(alvo, Vec::new());
        self.inicio_intervalo.insert(alvo, self.historico.len());
    }

    // Diagnóstico: pacotes de combate e HP que citam uma entidade, em ordem (dano recebido, cura, HP do jogador).
    fn mostrar_se_envolve(&mut self, entidade: u32, op: u16, pacote: &[u8]) {
        if !matches!(op, opcodes::DANO | opcodes::DANO_PERIODICO | opcodes::HP_RESTANTE | opcodes::MORTE_ENTIDADE) {
            return;
        }
        let mut r = LeitorPacote::novo(pacote, 0);
        let Ok(primeiro) = r.ler_varint().and_then(|_| r.ler_u16()).and_then(|_| r.ler_varint()) else { return };
        let primeiro = primeiro as u32;

        let mut detalhe = String::new();
        if op == opcodes::DANO {
            let leitura = combate::dano(pacote);
            if primeiro != entidade && !leitura.as_ref().is_ok_and(|e| e.autor_id == entidade) {
                return;
            }
            detalhe = match leitura {
                Ok(e) => format!(
                    "autor {} -> alvo {} skill {} dano {} tipo {}{}{}",
                    e.autor_id,
                    e.alvo_id,
                    e.skill,
                    e.dano,
                    e.tipo_dano,
                    if e.aparo { " APARO" } else { "" },
                    if e.perfeito { " PERFEITO" } else { "" }
                ),
                Err(motivo) => format!("(não leu: {motivo})"),
            };
        } else if op == opcodes::DANO_PERIODICO {
            let leitura = combate::dano_periodico(pacote);
            if primeiro != entidade && !leitura.as_ref().is_ok_and(|e| e.autor_id == entidade) {
                return;
            }
            detalhe = match leitura {
                Ok(e) => format!("DoT autor {} -> alvo {} skill {} dano {}", e.autor_id, e.alvo_id, e.skill, e.dano),
                Err(motivo) => format!("(DoT não leu: {motivo})"),
            };
        } else if primeiro != entidade {
            return;
        }

        let inicio = *self.inicio_diag.get_or_insert(self.hora);
        println!("  [{:>6}s] 0x{op:04X} {detalhe}", f(segundos(self.hora - inicio), 2));
        println!("           {}", hex(pacote));
    }

    fn procurar_queda(&mut self, alvo: u32, queda: i64, desde: usize) {
        self.procuras_queda += 1;
        let mut padrao_varint = Vec::new();
        let mut v = queda as u64;
        loop {
            padrao_varint.push(if v < 0x80 { v as u8 } else { (v & 0x7F) as u8 | 0x80 });
            if v < 0x80 {
                break;
            }
            v >>= 7;
        }
        let padrao_u32 = (queda as u32).to_le_bytes();

        println!(
            "  [queda] alvo {alvo} caiu {queda} (varint {}), pacotes no intervalo: {}",
            hex(&padrao_varint),
            self.historico.len() - desde
        );
        for pacote in &self.historico[desde..] {
            let pos_v = procurar(pacote, &padrao_varint).map_or(-1, |p| p as i64);
            let pos_u = procurar(pacote, &padrao_u32).map_or(-1, |p| p as i64);
            let op = opcodes::ler(pacote).unwrap_or(0);
            if pos_v >= 0 || pos_u >= 0 {
                println!("     0x{op:04X} varint@{pos_v} u32@{pos_u}: {}", hex(pacote));
            } else if op == opcodes::DANO {
                println!("     0x{op:04X} (sem o valor): {}", hex(pacote));
            }
        }
    }

    fn relatar_conferencia_hp(&self) {
        let c = &self.conferencias;
        if c.is_empty() {
            return;
        }
        let iguais = c.iter().filter(|c| c.queda_hp == c.dano as i64).count();
        println!("  Conferência HP x dano: {iguais}/{} intervalos com queda de HP igual à soma do dano", c.len());

        // HP de jogador tem outro formato e dá valores absurdos: só intervalos com HP de mob plausível.
        const QUEDA_MAXIMA_PLAUSIVEL: i64 = 1_000_000_000;
        let validos: Vec<&Conferencia> =
            c.iter().filter(|c| c.queda_hp > 0 && c.queda_hp < QUEDA_MAXIMA_PLAUSIVEL && c.dano > 0).collect();
        let sem_dano = c.iter().filter(|c| c.queda_hp > 0 && c.dano == 0).count();
        let sem_queda = c.iter().filter(|c| c.queda_hp <= 0 && c.dano > 0).count();
        println!(
            "  Razão queda/dano em {} intervalos (queda sem dano: {sem_dano}, dano sem queda: {sem_queda})",
            validos.len()
        );

        let razao = |c: &&Conferencia| c.queda_hp as f64 / c.dano as f64;
        let resumo = |grupo: &[&Conferencia]| {
            let mut r: Vec<f64> = grupo.iter().map(razao).collect();
            r.sort_by(f64::total_cmp);
            format!("n={:>3} min={} mediana={} máx={}", r.len(), f(r[0], 3), f(r[r.len() / 2], 3), f(r[r.len() - 1], 3))
        };

        for (alvo, grupo) in por_quantidade(agrupar(&validos, |c| c.alvo)) {
            println!("    alvo {alvo:>6}: {}", resumo(&grupo));
        }

        let golpe_unico: Vec<&Conferencia> = validos.iter().copied().filter(|c| c.eventos.len() == 1).collect();
        println!(
            "  Intervalos com um golpe só ({}), por jogador (fator igual para todos?):",
            golpe_unico.len()
        );
        for (autor, grupo) in por_quantidade(agrupar(&golpe_unico, |c| c.eventos[0].autor_id)) {
            println!("    autor {autor:>6}: {}", resumo(&grupo));
        }

        println!("  Intervalos com um golpe só ({}), por skill:", golpe_unico.len());
        for (skill, grupo) in por_quantidade(agrupar(&golpe_unico, |c| c.eventos[0].skill)) {
            let mut tipos: Vec<String> = Vec::new();
            for c in &grupo {
                let tipo = c.eventos[0].tipo_dano.to_string();
                if !tipos.contains(&tipo) {
                    tipos.push(tipo);
                }
            }
            println!("    skill {skill:>9}: {}  tipos {}", resumo(&grupo), tipos.join(","));
        }

        let hp_vistos: HashSet<u32> = c.iter().map(|c| c.alvo).collect();
        let mut alvos_sem_hp: Vec<u32> = Vec::new();
        for (_, e) in &self.eventos {
            if !hp_vistos.contains(&e.alvo_id) && !alvos_sem_hp.contains(&e.alvo_id) {
                alvos_sem_hp.push(e.alvo_id);
            }
        }
        let lista: Vec<String> = alvos_sem_hp.iter().map(ToString::to_string).collect();
        println!("  Alvos com dano mas sem nenhum 0x8D00: {} ({})", alvos_sem_hp.len(), lista.join(", "));
    }
}

/// GroupBy do LINQ: grupos na ordem em que a chave apareceu primeiro.
fn agrupar<T: Clone, K: PartialEq + Copy>(itens: &[T], chave: impl Fn(&T) -> K) -> Vec<(K, Vec<T>)> {
    let mut grupos: Vec<(K, Vec<T>)> = Vec::new();
    for item in itens {
        let k = chave(item);
        match grupos.iter_mut().find(|g| g.0 == k) {
            Some(g) => g.1.push(item.clone()),
            None => grupos.push((k, vec![item.clone()])),
        }
    }
    grupos
}

/// Grupos do maior para o menor; empate fica na ordem de aparição (sort estável).
fn por_quantidade<K, T>(mut grupos: Vec<(K, Vec<T>)>) -> Vec<(K, Vec<T>)> {
    grupos.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    grupos
}

fn mais_frequentes<K>(contagem: &IndexMap<K, i32>) -> Vec<(&K, i32)> {
    let mut itens: Vec<(&K, i32)> = contagem.iter().map(|(k, v)| (k, *v)).collect();
    itens.sort_by(|a, b| b.1.cmp(&a.1));
    itens
}

fn soma_dano(eventos: &[(Hora, EventoDano)]) -> f64 {
    eventos.iter().fold(0.0, |s, e| s + e.1.dano as f64)
}
