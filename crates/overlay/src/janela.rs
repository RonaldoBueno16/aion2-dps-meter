//! A janela, no visual da 0.8.0 (inspirado no medidor do TK): cabeçalho com o logo e botões de
//! ícone, a barra do alvo (o chefe da luta ou o mob que mais apanhou), abas DPS | Tank | Healer,
//! uma linha por jogador (medalhão da classe, barra em degradê na cor dela, total, por segundo e %),
//! a ficha e as skills ao expandir, o rodapé com o estado e o tempo da luta e, embaixo dele, os
//! eventos de horário fixo e os chefes de campo mortos. Mais: configurações, lutas anteriores, os
//! chefes de campo da região e recolher para a borda.

mod chefes;
mod configuracoes;
mod drops;
mod item;
mod lutas;
mod recolher;
mod visual;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use eframe::egui::load::SizedTexture;
use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, FontData, FontFamily, FontId, Galley, Painter, Rect, RichText,
    Sense, Stroke, StrokeKind, TextFormat, TextureHandle, Ui, Vec2, ViewportCommand, pos2, vec2,
};
use indexmap::IndexMap;
use lutas::ResumoLuta;
use nucleo::captura::socket_bruto::CapturaSocketBruto;
use nucleo::formato::{f, n, p};
use nucleo::medicao::catalogo::{self, Busca, CatalogoSkills, InfoRegiao};
use nucleo::medicao::dados_jogo;
use nucleo::medicao::medidor::{Alvo, LinhaBuff, LinhaJogador, LinhaSkill, Medidor, PerfilJogador, Placar, Tabela};
use nucleo::medicao::sessao::Sessao;
use nucleo::protocolo::combate::ChefesDeCampo;
use nucleo::{Hora, TICKS_POR_SEGUNDO};
use recolher::{Dobra, Lado};
use serde::{Deserialize, Serialize};

use crate::atalho::Atalho;
use crate::atualizacao::{self, Atualizacao, Estado};
use crate::bandeja::{self, Bandeja};
use crate::config::{self, Config};
use crate::eventos;
use crate::jogo;

/// 470 e não os 390 do WPF: a tabela da aba DPS precisa de ~280 px ao lado do nome.
pub const LARGURA: f32 = 470.0;
const SALVAR_A_CADA: Duration = Duration::from_secs(30);
const SEMIBOLD: &str = "semibold";
const TEXTO_DA_ABA: [&str; 2] = ["Overlay ›", "‹ Overlay"];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Aba {
    Dps,
    Tank,
    Healer,
}

impl Aba {
    /// O que os três números da linha medem nesta aba (legenda acima das linhas).
    fn rotulos(self) -> [&'static str; 3] {
        match self {
            Aba::Dps => ["Dano", "DPS", "%"],
            Aba::Tank => ["Recebido", "DTPS", "%"],
            Aba::Healer => ["Cura", "HPS", "%"],
        }
    }
}

/// Altura de uma linha de jogador, em pontos.
const ALTURA_LINHA: f32 = 28.0;
/// Sem mudança no placar por esse tempo, o rodapé passa de "Em luta" para "Aguardando".
const PARADO: Duration = Duration::from_secs(5);
/// Logo do cabeçalho: o hexágono do axon.ico em 64 px.
const LOGO: &[u8] = include_bytes!("../assets/axon-64.png");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tela {
    Medidor,
    Configuracoes,
    Lutas,
    Chefes,
}

/// Buffs mostrados embaixo das skills de um jogador expandido.
const BUFFS_MOSTRADOS: usize = 8;

/// Último level e power de cada nome (inclusive o seu), para o overlay aberto no meio da sessão.
/// Mesmo formato do jogadores.json da versão em C#.
#[derive(Serialize, Deserialize)]
struct MemoriaSalva {
    #[serde(rename = "Eu", default)]
    eu: Option<String>,
    #[serde(rename = "Perfis", default)]
    perfis: IndexMap<String, PerfilJogador>,
}

pub struct Overlay {
    sessao: Arc<Mutex<Sessao>>,
    captura: Option<CapturaSocketBruto>,
    erro_captura: Option<String>,
    /// Sem a regra do firewall deste exe: a captura espera o clique no aviso (Liberar ou Agora não).
    pedir_firewall: bool,
    catalogo: &'static CatalogoSkills,
    aba: Aba,
    expandidos: HashSet<(Aba, u32)>,
    // Placar lido a cada `atualizacao_ms` (500 ms por padrão): com o mouse em cima o egui redesenha a
    // ~60 fps, e cada leitura segura o Mutex que as threads de captura usam.
    placar: Placar,
    /// Luta do histórico aberta no lugar da de agora: número e começo dela. None = ao vivo.
    vendo: Option<(u64, Hora)>,
    /// Resumo das lutas passadas, lido só com a tela de lutas aberta.
    lutas: Vec<ResumoLuta>,
    fluxo: Option<String>,
    /// Menor ida e volta TCP dos últimos 10 s, em ticks (`Sessao::ping`).
    ping: Option<i64>,
    /// Energia Odyle (básica, carregada), como o medidor recebeu.
    odyle: Option<(u64, Option<u64>)>,
    /// PNG do cristal da Odyle, depois de baixado.
    icone_odyle: Option<PathBuf>,
    /// Quando a captura abriu e quando o jogo apareceu aberto: o rodapé só aponta um problema
    /// depois de dar tempo de o servidor aparecer.
    captura_desde: Instant,
    jogo_desde: Option<Instant>,
    lido_em: Instant,
    salvo_em: Instant,
    /// Quando a duração ou os totais mudaram por último, e o que eles eram: o rodapé diz "Em luta"
    /// enquanto o placar anda.
    mudou_em: Instant,
    /// Quando o último resumo foi copiado: o rodapé avisa por uns segundos.
    copiado_em: Option<Instant>,
    assinatura: (i64, u64),
    icones: HashMap<PathBuf, Option<TextureHandle>>,
    /// Ícone da primeira skill de cada classe, usado no medalhão, quando já baixou.
    emblemas: HashMap<&'static str, PathBuf>,
    /// Ícone de cada evento, pelo nome na CDN, quando já baixou.
    icones_eventos: HashMap<String, PathBuf>,
    /// Retratos dos chefes de campo e ícones dos drops, só na memória (None: o PNG não abriu).
    imagens: HashMap<String, Option<TextureHandle>>,
    /// Quando cada imagem foi pedida: o skeleton pulsa por até ESPERA_DA_IMAGEM.
    imagens_pedidas: HashMap<String, Instant>,
    /// Algo pulsando neste quadro (skeleton): o próximo vem logo, e não no intervalo do placar.
    carregando: bool,
    /// Último 0x9101 (os chefes de campo da região) e quando chegou, como o medidor guardou.
    chefes: Option<(ChefesDeCampo, Hora)>,
    /// Nome e chefes de cada região, quando o questlog já mandou.
    regioes: HashMap<u32, InfoRegiao>,
    /// Aba aberta na tela de chefes: Mortos (true) ou Vivos.
    chefes_mortos: bool,
    /// Drops de um chefe de campo, num painel ao lado da janela.
    painel: Option<drops::PainelDrops>,
    /// Largura (em pontos) com que o tamanho da janela foi pedido por último.
    largura_aplicada: f32,
    /// Só no debug (--drops <código do NPC>): abre o painel no primeiro quadro.
    drops_inicial: Option<u32>,
    logo: Option<Option<TextureHandle>>,
    /// Altura do conteúdo (em pontos) e escala (pixels por ponto) com que o tamanho da janela foi
    /// pedido por último. A escala e não o zoom da config: ela muda também com o DPI do monitor.
    altura: f32,
    escala_aplicada: f32,
    /// --replay (só debug): a captura antiga não grava memória nem config, e a luta nunca zera.
    replay: bool,
    /// Só no debug (--expandir): abre todas as linhas, para conferir o desenho sem clicar.
    expandir_tudo: bool,
    /// Quantos jogadores a lista mostra (LIMITE_DE_LINHAS; no debug, --limite N para ver o "você
    /// abaixo" com uma captura de poucos jogadores).
    limite: usize,
    config: Config,
    tela: Tela,
    dobra: Dobra,
    /// HWND da janela (0 enquanto o primeiro quadro não chegou).
    janela: isize,
    /// Arraste da janela pelo fundo: cursor e [x, y, largura, altura] da janela no começo, em
    /// pixels físicos.
    arraste: Option<([i32; 2], [i32; 4])>,
    /// Sua linha (ou um esboço com o seu nome) para a amostra da tela de configurações.
    amostra: Option<LinhaJogador>,
    /// Ícone ao lado do relógio; None se o Windows não deixou criar.
    bandeja: Option<Bandeja>,
    /// O que foi pedido à janela por último: o clique passa por ela até o jogo (atalho ou menu da
    /// bandeja).
    atravessando: bool,
    atualizacao: Atualizacao,
    /// Depois de trocar o exe: Ok com a versão nova aberta (esta fecha), Err se não abriu.
    reabertura: Option<Result<(), String>>,
    /// Só no debug (--recolher / --recolher-e-voltar): recolhe aos 2 s e volta aos 4,5 s.
    teste_dobra: Option<(Instant, bool, bool)>,
}

impl Overlay {
    pub fn novo(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        configurar_estilo(&cc.egui_ctx)?;
        dados_jogo::carregar_nomes_padrao();
        let catalogo = dados_jogo::definir_catalogo(CatalogoSkills::novo(None));

        let opcoes_debug: Vec<String> = if cfg!(debug_assertions) { std::env::args().collect() } else { Vec::new() };
        let tem = |opcao: &str| opcoes_debug.iter().any(|a| a == opcao);
        let mut config = Config::carregar();
        if let Some(zoom) = opcoes_debug.iter().skip_while(|a| *a != "--zoom").nth(1).and_then(|z| z.parse().ok()) {
            config.zoom = config::arredondar_zoom(zoom);
        }
        config.compacta |= tem("--compacta");
        config.ocultar_nomes |= tem("--ocultar-nomes");
        cc.egui_ctx.set_zoom_factor(config.zoom);

        let sessao = Arc::new(Mutex::new(Sessao::default()));
        carregar_memoria(&mut travar(&sessao).medidor);
        let replay = arquivo_replay();
        // Ao vivo, o overlay fica dentro da área do jogo e os atalhos seguem o jogo; o replay de debug
        // roda sem ele.
        jogo::seguir(replay.is_none());
        // Mudar o firewall só com o clique do usuário. --pedir-firewall (debug) mostra o aviso no replay.
        let pedir_firewall = tem("--pedir-firewall") || (replay.is_none() && !crate::firewall::liberada());
        let (captura, erro_captura) = match &replay {
            Some(arquivo) => {
                let ate = opcoes_debug.iter().skip_while(|a| *a != "--ate").nth(1).and_then(|s| s.parse::<f64>().ok());
                reproduzir(sessao.clone(), arquivo.clone(), tem("--lutas"), ate);
                (None, None)
            }
            None => {
                travar(&sessao).medidor.inatividade = i64::from(config.inatividade) * TICKS_POR_SEGUNDO;
                if pedir_firewall { (None, None) } else { iniciar_captura(&sessao) }
            }
        };

        let mut overlay = Self {
            sessao,
            captura,
            erro_captura,
            pedir_firewall,
            catalogo,
            aba: if tem("--tank") { Aba::Tank } else { Aba::Dps },
            expandidos: HashSet::new(),
            placar: Placar::default(),
            vendo: None,
            lutas: Vec::new(),
            fluxo: None,
            ping: None,
            odyle: None,
            icone_odyle: None,
            captura_desde: Instant::now(),
            jogo_desde: None,
            lido_em: Instant::now(),
            salvo_em: Instant::now(),
            mudou_em: Instant::now(),
            copiado_em: None,
            assinatura: (0, 0),
            icones: HashMap::new(),
            emblemas: HashMap::new(),
            icones_eventos: HashMap::new(),
            imagens: HashMap::new(),
            imagens_pedidas: HashMap::new(),
            carregando: false,
            chefes: None,
            regioes: HashMap::new(),
            chefes_mortos: true,
            painel: None,
            largura_aplicada: LARGURA,
            drops_inicial: opcoes_debug.iter().skip_while(|a| *a != "--drops").nth(1).and_then(|c| c.parse().ok()),
            logo: None,
            altura: 0.0,
            escala_aplicada: 0.0,
            replay: replay.is_some(),
            expandir_tudo: tem("--expandir"),
            limite: opcoes_debug
                .iter()
                .skip_while(|a| *a != "--limite")
                .nth(1)
                .and_then(|n| n.parse().ok())
                .unwrap_or(LIMITE_DE_LINHAS),
            config,
            tela: if tem("--config") {
                Tela::Configuracoes
            } else if tem("--lutas") {
                Tela::Lutas
            } else if tem("--chefes") {
                Tela::Chefes
            } else {
                Tela::Medidor
            },
            dobra: Dobra::Aberto,
            janela: manter_sem_ativar(cc),
            arraste: None,
            amostra: None,
            bandeja: None,
            atravessando: false,
            atualizacao: if tem("--nova-versao") { Atualizacao::falsa() } else { Atualizacao::iniciar() },
            reabertura: None,
            teste_dobra: (tem("--recolher") || tem("--recolher-e-voltar"))
                .then(|| (Instant::now(), tem("--recolher-e-voltar"), false)),
        };
        let c = &overlay.config;
        let atalhos =
            [&c.atalho_mostrar, &c.atalho_atravessar, &c.atalho_resumo, &c.atalho_compacta].map(|a| Atalho::ler(a));
        overlay.bandeja = Bandeja::iniciar(overlay.janela, atalhos);
        overlay.ler_placar();
        Ok(overlay)
    }

    fn ler_placar(&mut self) {
        let sessao = travar(&self.sessao);
        // Luta do histórico aberta: o placar dela, parado. Se ela saiu do histórico (20 lutas
        // depois), volta ao vivo; os ids se repetem entre lutas, então nada fica expandido.
        let lutas = sessao.medidor.lutas_passadas();
        let passada =
            self.vendo.and_then(|(numero, _)| lutas.iter().find(|l| l.numero == numero)).map(|l| l.placar.clone());
        if self.vendo.is_some() && passada.is_none() {
            self.vendo = None;
            self.expandidos.clear();
        }
        self.placar = match passada {
            Some(placar) => (*placar).clone(),
            None => sessao.medidor.obter_placar(),
        };
        if self.config.ocultar_nomes {
            ocultar_nomes(&mut self.placar);
        }
        if self.tela == Tela::Lutas {
            self.lutas = lutas.iter().map(|luta| ResumoLuta::de(luta, self.config.ocultar_nomes)).collect();
        }
        self.fluxo = sessao.fluxo.clone();
        self.ping = sessao.ping();
        self.odyle = sessao.medidor.odyle;
        self.chefes = sessao.medidor.chefes_de_campo.clone();
        let memoria = (self.tela == Tela::Configuracoes).then(|| sessao.medidor.exportar_memoria());
        drop(sessao);
        self.lido_em = Instant::now();
        let totais = self.placar.dano.total + self.placar.dano_recebido.total + self.placar.cura.total;
        let assinatura = (self.placar.duracao, totais.to_bits());
        if assinatura != self.assinatura {
            self.assinatura = assinatura;
            self.mudou_em = self.lido_em;
        }
        if self.jogo_desde.is_none() && jogo::aberto() {
            self.jogo_desde = Some(self.lido_em);
        }
        if let Some((eu, perfis)) = memoria {
            self.amostra = Some(amostra(self.tabela_bruta(), eu, &perfis));
        }
        if self.expandir_tudo {
            for (aba, tabela) in [(Aba::Dps, &self.placar.dano), (Aba::Tank, &self.placar.dano_recebido), (Aba::Healer, &self.placar.cura)] {
                self.expandidos.extend(tabela.jogadores.iter().map(|j| (aba, j.id)));
            }
        }
    }

    fn salvar_memoria(&mut self) {
        self.salvo_em = Instant::now();
        if self.replay {
            return;
        }
        let (eu, perfis) = travar(&self.sessao).medidor.exportar_memoria();
        if perfis.is_empty() {
            return;
        }
        let caminho = arquivo_memoria();
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        // Sem disco agora: tenta de novo no próximo ciclo.
        if let Ok(json) = serde_json::to_string(&MemoriaSalva { eu, perfis }) {
            let _ = catalogo::escrever_trocando(&caminho, json.as_bytes());
        }
    }

    /// Config mudou: a inatividade vale na hora para o medidor; o zoom, no próximo quadro.
    fn aplicar_config(&mut self) {
        if self.replay {
            return;
        }
        travar(&self.sessao).medidor.inatividade = i64::from(self.config.inatividade) * TICKS_POR_SEGUNDO;
        self.config.salvar();
    }

    fn intervalo(&self) -> Duration {
        Duration::from_millis(u64::from(self.config.atualizacao_ms))
    }

    fn copiar_resumo(&mut self, ctx: &egui::Context) {
        ctx.copy_text(resumo(&self.placar, self.config.resumo_em_linhas));
        self.copiado_em = Some(Instant::now());
    }

    fn alternar_compacta(&mut self) {
        self.fechar_drops();
        self.config.compacta = !self.config.compacta;
        self.aplicar_config();
    }

    fn largura_janela(&self) -> f32 {
        if self.painel.is_some() { LARGURA + drops::VAO + drops::LARGURA_PAINEL } else { LARGURA }
    }

    /// Abre os drops do chefe ao lado da janela, do lado com espaço (abrindo à esquerda, a janela anda
    /// para a esquerda e o medidor fica onde está); fecha, se já são os dele.
    fn alternar_drops(&mut self, codigo: u32, ppp: f32) {
        match self.painel.as_ref().map(|p| (p.codigo, p.lado)) {
            Some((aberto, _)) if aberto == codigo => return self.fechar_drops(),
            Some((_, lado)) => self.painel = Some(drops::PainelDrops::novo(codigo, lado)),
            None => {
                let painel = ((drops::VAO + drops::LARGURA_PAINEL) * ppp).round() as i32;
                let total = (self.largura_janela() * ppp).round() as i32 + painel;
                let (lado, dx) = recolher::lugar_do_painel(self.janela, painel, total);
                recolher::ajustar_largura(self.janela, total, dx);
                self.painel = Some(drops::PainelDrops::novo(codigo, lado));
            }
        }
        // Uma falha antes (sem internet) não impede de tentar de novo ao abrir.
        dados_jogo::repetir_falhas();
        self.imagens_pedidas.clear();
        // Os nomes dos atributos (~200 KB) já vêm enquanto a lista carrega: a primeira ficha não espera.
        let _ = dados_jogo::atributos();
    }

    /// Fecha o painel de drops: a janela volta à largura do medidor, que fica onde está.
    fn fechar_drops(&mut self) {
        let Some(painel) = self.painel.take() else { return };
        let ppp = if self.escala_aplicada > 0.0 { self.escala_aplicada } else { 1.0 };
        let painel_px = ((drops::VAO + drops::LARGURA_PAINEL) * ppp).round() as i32;
        let dx = if painel.lado == Lado::Esquerda { painel_px } else { 0 };
        recolher::ajustar_largura(self.janela, (LARGURA * ppp).round() as i32, dx);
    }

    fn zerar(&mut self) {
        travar(&self.sessao).medidor.reiniciar();
        self.expandidos.clear();
        self.ler_placar();
    }

    /// Abre uma luta do histórico (`Some`) ou volta à de agora (`None`).
    fn ver(&mut self, luta: Option<(u64, Hora)>) {
        self.vendo = luta;
        self.tela = Tela::Medidor;
        self.expandidos.clear();
        self.ler_placar();
    }

    fn tabela_bruta(&self) -> &Tabela {
        match self.aba {
            Aba::Dps => &self.placar.dano,
            Aba::Tank => &self.placar.dano_recebido,
            Aba::Healer => &self.placar.cura,
        }
    }

    /// A tabela da aba atual, só com você se "Só o meu dano" estiver ligado.
    fn tabela(&self) -> Tabela {
        let tabela = self.tabela_bruta();
        if self.config.so_meu_dano { tabela.so_voce() } else { tabela.clone() }
    }

    fn em_luta(&self) -> bool {
        self.vendo.is_none() && self.placar.duracao > 0 && self.mudou_em.elapsed() < PARADO
    }

    fn conteudo(&mut self, ui: &mut Ui) {
        match self.tela {
            Tela::Configuracoes => return self.tela_configuracoes(ui),
            Tela::Lutas => return self.tela_lutas(ui),
            Tela::Chefes => return self.tela_chefes(ui),
            Tela::Medidor => {}
        }
        let tabela = self.tabela();
        if self.config.compacta && !self.pedir_firewall {
            return self.barra_compacta(ui);
        }
        self.cabecalho(ui);
        ui.add_space(8.0);
        if self.pedir_firewall {
            self.aviso_firewall(ui);
            ui.add_space(6.0);
            self.status(ui);
            return;
        }
        if let Some(alvo) = self.placar.alvo.clone() {
            self.barra_do_alvo(ui, &alvo);
            ui.add_space(6.0);
        }
        self.abas(ui);
        ui.add_space(4.0);
        self.linhas(ui, &tabela);
        ui.add_space(8.0);
        self.rodape(ui, &tabela);
        self.eventos(ui);
        ui.add_space(2.0);
        self.status(ui);
    }

    fn cabecalho(&mut self, ui: &mut Ui) {
        let logo = self.logo(ui.ctx());
        ui.horizontal(|ui| {
            if let Some(logo) = logo {
                let (rect, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                ui.painter().image(logo.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                ui.add_space(6.0);
            }
            let mut marca = LayoutJob::default();
            let formato = TextFormat {
                font_id: fonte(15.0, true),
                color: visual::DOURADO,
                extra_letter_spacing: 2.5,
                ..Default::default()
            };
            marca.append("AXON", 0.0, formato);
            ui.label(marca);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if visual::botao_icone(ui, "✕", 13.0).on_hover_text("Fechar o medidor").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                // A seta aponta para a borda para onde a janela vai.
                let seta = match recolher::lado_mais_perto(self.janela) {
                    Lado::Esquerda => "‹",
                    Lado::Direita => "›",
                };
                if visual::botao_icone(ui, seta, 17.0).on_hover_text("Recolher para a borda da tela").clicked() {
                    // O painel de drops fecha antes: o recolher mede a janela só com o medidor.
                    self.fechar_drops();
                    self.dobra.recolher(self.janela);
                }
                if visual::botao_icone(ui, "⚙", 15.0).on_hover_text("Configurações").clicked() {
                    self.tela = Tela::Configuracoes;
                    self.ler_placar();
                }
                if visual::botao_icone(ui, "☰", 14.0).on_hover_text("Lutas anteriores").clicked() {
                    self.tela = Tela::Lutas;
                    self.ler_placar();
                }
                if visual::botao_icone(ui, "♛", 14.0).on_hover_text("Chefes de campo (vivos e mortos)").clicked() {
                    self.tela = Tela::Chefes;
                }
                if visual::botao_icone(ui, "▭", 14.0).on_hover_text("Barra compacta: uma linha só").clicked() {
                    self.alternar_compacta();
                }
                let dica = "Copiar o resumo da luta (DPS e % de cada um) para colar no chat";
                if visual::botao_icone(ui, "⧉", 14.0).on_hover_text(dica).clicked() {
                    self.copiar_resumo(ui.ctx());
                }
                // Vendo uma luta passada, o Zerar (que mexe na de agora) dá lugar à volta ao vivo.
                if self.vendo.is_some() {
                    if botao(ui, "● Ao vivo", true).on_hover_text("Volta à luta de agora").clicked() {
                        self.ver(None);
                    }
                } else if visual::botao_icone(ui, "↺", 16.0).on_hover_text("Zerar: começa uma luta nova").clicked() {
                    self.zerar();
                }
            });
        });
    }

    /// O alvo da luta: retrato (ou espadas), nome e level do questlog, HP como o servidor manda e
    /// o seu dano e o do grupo nele.
    fn barra_do_alvo(&mut self, ui: &mut Ui, alvo: &Alvo) {
        let retrato = alvo.retrato.as_deref().and_then(|caminho| self.textura(ui.ctx(), caminho));
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 48.0), Sense::hover());
        let maximo = alvo.hp_maximo.map_or_else(String::new, |m| format!(" de {}", n(m as f64, 0)));
        resposta.on_hover_text(format!(
            "Alvo: na guerra com chefe, o chefe; sem chefe, o último mob em que você bateu (o alvo selecionado \
             no jogo não chega ao Axon). HP{maximo} como o servidor manda (0x8D00; o máximo e o % vêm do pacote \
             de criação do mob). \"Derrota em\": quanto falta na velocidade em que o HP caiu nos últimos 30 s. \
             Sem nome (\"Chefe #id\", \"Alvo #id\"): o mob já estava na tela quando o Axon abriu, e o nome e o \
             HP máximo vêm do pacote de criação dele; abra o Axon antes de chegar à luta."
        ));
        let pintor = ui.painter().clone();
        let (forte, fraco) = if alvo.morto {
            (
                Color32::from_rgba_unmultiplied(0x4A, 0x40, 0x30, 0xF0),
                Color32::from_rgba_unmultiplied(0x1E, 0x1A, 0x14, 0xE6),
            )
        } else {
            (
                Color32::from_rgba_unmultiplied(0x8E, 0x1B, 0x22, 0xF0),
                Color32::from_rgba_unmultiplied(0x2B, 0x0B, 0x10, 0xE6),
            )
        };
        visual::degrade(&pintor, rect, 6.0, forte, fraco);
        pintor.rect_stroke(
            rect,
            6,
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xFF, 0x7B, 0x7B, 0x50)),
            StrokeKind::Inside,
        );

        let centro = pos2(rect.min.x + 24.0, rect.center().y);
        pintor.circle_filled(centro, 17.0, Color32::from_rgb(0x1A, 0x0A, 0x0D));
        match retrato {
            Some(textura) => {
                let circulo = Rect::from_center_size(centro, Vec2::splat(33.0));
                egui::Image::new(SizedTexture::new(textura.id(), circulo.size()))
                    .corner_radius(16)
                    .paint_at(ui, circulo);
            }
            None => {
                let simbolo = if alvo.morto { "☠" } else { "⚔" };
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(simbolo.into(), fonte(17.0, false), visual::DOURADO));
                pintor.galley(centro - galley.size() / 2.0, galley, visual::DOURADO);
            }
        }
        pintor.circle_stroke(centro, 17.0, Stroke::new(1.5_f32, visual::DOURADO));

        // Barra de HP no pé do card, quando o spawn trouxe o máximo.
        let fracao = match (alvo.hp, alvo.hp_maximo) {
            (Some(hp), Some(maximo)) if !alvo.morto => Some(hp as f64 / maximo as f64),
            _ => None,
        };
        if let Some(fracao) = fracao {
            let canto = pos2(rect.min.x + 48.0, rect.max.y - 7.0);
            let trilho = Rect::from_min_max(canto, pos2(rect.max.x - 10.0, rect.max.y - 4.0));
            pintor.rect_filled(trilho, 2, Color32::from_black_alpha(0xA0));
            let cheio = trilho.with_max_x(trilho.min.x + trilho.width() * fracao.clamp(0.0, 1.0) as f32);
            pintor.rect_filled(cheio, 2, visual::VERMELHO_CLARO);
        }

        // Direita: HP (ou "Derrotado") em cima, o dano do grupo embaixo.
        let mut hp = LayoutJob::default();
        if alvo.morto {
            trecho(&mut hp, "Derrotado", 13.0, true, visual::DOURADO);
        } else {
            trecho(&mut hp, "HP ", 10.0, false, branco(0xBB));
            let valor = alvo.hp.map_or_else(|| "?".to_string(), |hp| n(hp as f64, 0));
            trecho(&mut hp, &valor, 13.0, true, visual::VERMELHO_CLARO);
            if let Some(fracao) = fracao {
                trecho(&mut hp, &format!("  {}", p(fracao, 1)), 10.0, true, branco(0xDD));
            }
        }
        let hp = montar(ui, hp);
        let mut dano = LayoutJob::default();
        if alvo.meu_dano > 0.0 {
            trecho(&mut dano, "Você ", 10.0, false, branco(0xBB));
            trecho(&mut dano, &compacto(alvo.meu_dano), 10.0, true, visual::DOURADO);
            trecho(&mut dano, &format!("  ·  grupo {}", compacto(alvo.dano)), 10.0, false, branco(0xBB));
        } else {
            trecho(&mut dano, &format!("{} de dano do grupo", compacto(alvo.dano)), 10.0, false, branco(0xBB));
        }
        let dano = montar(ui, dano);
        let direita = hp.size().x.max(dano.size().x);

        let mut job = LayoutJob::single_section(nome_do_alvo(alvo), TextFormat::simple(fonte(13.0, true), texto()));
        job.wrap = uma_linha((largura - 48.0 - direita - 18.0).max(60.0));
        let nome = montar(ui, job);
        let mut detalhe = LayoutJob::default();
        if alvo.nivel > 0 {
            trecho(&mut detalhe, &format!("Nv {}", alvo.nivel), 10.0, false, branco(0xCC));
        }
        if alvo.chefe {
            let separador = if alvo.nivel > 0 { "  ·  " } else { "" };
            trecho(&mut detalhe, &format!("{separador}Chefe"), 10.0, true, visual::DOURADO);
        }
        if let Some(segundos) = alvo.derrota_em {
            let separador = if detalhe.sections.is_empty() { "" } else { "  ·  " };
            let falta = minutos_e_segundos((segundos * TICKS_POR_SEGUNDO as f64) as i64);
            trecho(&mut detalhe, &format!("{separador}derrota em {falta}"), 10.0, false, branco(0xCC));
        }
        let detalhe = (!detalhe.sections.is_empty()).then(|| montar(ui, detalhe));

        let x = rect.min.x + 48.0;
        let altura = nome.size().y + detalhe.as_ref().map_or(0.0, |g| g.size().y);
        let topo = rect.center().y - altura / 2.0;
        let altura_nome = nome.size().y;
        visual::com_sombra(&pintor, pos2(x, topo), nome);
        if let Some(detalhe) = detalhe {
            visual::com_sombra(&pintor, pos2(x, topo + altura_nome), detalhe);
        }
        let x = rect.max.x - 10.0;
        let topo = rect.center().y - (hp.size().y + dano.size().y) / 2.0;
        let altura_hp = hp.size().y;
        visual::com_sombra(&pintor, pos2(x - hp.size().x, topo), hp);
        visual::com_sombra(&pintor, pos2(x - dano.size().x, topo + altura_hp), dano);
    }

    fn abas(&mut self, ui: &mut Ui) {
        let abas = [
            (Aba::Dps, "DPS", "Dano causado em monstros"),
            (
                Aba::Tank,
                "Tank",
                "Dano recebido de monstros. No mouse: golpes aparados e \"aggro N\" (N monstros têm este jogador \
                 como último alvo, 8 s); a ameaça em número fica no servidor e não chega ao jogo.",
            ),
            (
                Aba::Healer,
                "Healer",
                "Cura feita em jogadores. O pacote não separa a sobrecura, então o HPS pode incluir cura que \
                 passou do HP cheio. Leitura ainda não conferida numa luta com curandeiro.",
            ),
        ];
        let direita = ui.max_rect().max.x;
        let linha = ui
            .horizontal(|ui| {
                for (aba, nome, dica) in abas {
                    if visual::aba(ui, nome, self.aba == aba).on_hover_text(dica).clicked() {
                        self.aba = aba;
                        self.ler_placar();
                    }
                }
                if self.placar.so_chefe && self.aba == Aba::Dps {
                    ui.add_space(4.0);
                    let selo = RichText::new("⚔ só no chefe").font(fonte(9.5, true)).color(visual::DOURADO);
                    ui.label(selo).on_hover_text(
                        "Guerra com chefe: o DPS conta só o dano no chefe, do primeiro golpe nele em diante. \
                         Golpes nos mobs em volta ficam de fora.",
                    );
                }
            })
            .response
            .rect;
        // Legenda dos três números, alinhada com eles.
        let tres = Tres::medir(ui, self.config.numeros);
        let legendas = self.aba.rotulos().map(|r| {
            montar(ui, LayoutJob::single_section(r.into(), TextFormat::simple(fonte(9.5, false), branco(0x88))))
        });
        let faixa = Rect::from_min_max(pos2(linha.min.x, linha.min.y), pos2(direita, linha.max.y));
        tres.pintar(ui.painter(), faixa, legendas, false);
    }

    fn linhas(&mut self, ui: &mut Ui, tabela: &Tabela) {
        if tabela.jogadores.is_empty() {
            let vazio = if self.config.so_meu_dano && !self.placar.voce_reconhecido {
                "Você ainda não foi reconhecido: abra o overlay antes de entrar no mundo ou faça um abate."
            } else {
                match self.aba {
                    Aba::Tank => "Nenhum golpe de monstro em jogador ainda.",
                    Aba::Healer => "Nenhuma cura vista ainda.",
                    Aba::Dps => "Sem dano ainda.",
                }
            };
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(vazio).font(fonte(12.0, false)).color(texto().gamma_multiply(0.6))).wrap());
            ui.add_space(4.0);
            return;
        }

        let tres = Tres::medir(ui, self.config.numeros);
        let maior = tabela.jogadores[0].total;
        let voce = tabela.jogadores.iter().position(|j| j.voce);
        let (primeiras, abaixo) = linhas_mostradas(tabela.jogadores.len(), voce, self.limite);
        // Posição no placar inteiro da aba: com "Só o meu dano" a sua continua a real, e não 1.
        let bruta = &self.tabela_bruta().jogadores;
        let posicao = |j: &LinhaJogador| bruta.iter().position(|b| b.id == j.id).map_or(0, |i| i + 1);
        let mostradas: Vec<(usize, usize)> =
            primeiras.chain(abaixo).map(|i| (i, posicao(&tabela.jogadores[i]))).collect();
        for (i, posicao) in mostradas {
            if Some(i) == abaixo {
                separador(ui);
            }
            let j = &tabela.jogadores[i];
            self.linha_jogador(ui, j, posicao, maior, &tres);
            if self.expandidos.contains(&(self.aba, j.id)) {
                self.ficha(ui, j);
                for s in &j.skills {
                    self.linha_skill(ui, s, j.classe, &tres);
                }
                if !j.buffs.is_empty() {
                    self.buffs(ui, &j.buffs);
                }
                ui.add_space(4.0);
            }
        }
    }

    /// Dica da linha na aba Tank: as contagens que não cabem na linha.
    fn detalhe_tank(j: &LinhaJogador) -> String {
        let mut texto = format!(
            "{} golpes recebidos  ·  {} aparados ({})  ·  {} {}",
            j.golpes,
            j.aparos,
            p(razao(j.aparos, j.golpes), 0),
            j.mortes,
            if j.mortes == 1 { "morte" } else { "mortes" }
        );
        if j.segurando_aggro > 0 {
            let monstros = if j.segurando_aggro == 1 { "monstro" } else { "monstros" };
            texto += &format!("  ·  alvo de {} {monstros}", j.segurando_aggro);
        }
        texto
    }

    /// Dica da linha do jogador: classe, level, GS e os números que saíram da linha.
    fn dica_jogador(&self, j: &LinhaJogador) -> String {
        let perfil =
            segmentos_do_perfil(j).map(|pedacos| pedacos.into_iter().map(|(t, _)| t).collect::<String>()).join("  ·  ");
        let numeros = numeros_extras(self.aba, j.golpes, j.criticos, j.aparos, j.total, j.maximo);
        let extra = match self.aba {
            Aba::Dps => detalhe_dps(j.golpes, j.costas),
            Aba::Tank => Self::detalhe_tank(j),
            Aba::Healer => format!("{} curas", j.golpes),
        };
        format!("{perfil}\n{numeros}\n{extra}\nClique para ver as skills")
    }

    fn linha_jogador(&mut self, ui: &mut Ui, j: &LinhaJogador, posicao: usize, maior: f64, tres: &Tres) {
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, ALTURA_LINHA + 3.0), Sense::click());
        let linha = Rect::from_min_size(rect.min, vec2(largura, ALTURA_LINHA));
        let fracao = if maior > 0.0 { (j.total / maior) as f32 } else { 0.0 };
        let numeros = numeros_da_linha(ui, textos(j.total, j.por_segundo, j.porcentagem), 12.0);
        let expandido = self.expandidos.contains(&(self.aba, j.id));
        self.pintar_linha(ui, linha, j, Some(posicao), fracao, numeros, tres, expandido);

        let dica = self.dica_jogador(j);
        let resposta = resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica);
        if resposta.clicked() && !self.expandidos.remove(&(self.aba, j.id)) {
            self.expandidos.insert((self.aba, j.id));
        }
    }

    /// Fundo, barra em degradê na cor da classe, medalhão, nome e os três números. Também desenha a
    /// amostra da tela de configurações.
    #[allow(clippy::too_many_arguments)]
    fn pintar_linha(
        &mut self,
        ui: &Ui,
        linha: Rect,
        j: &LinhaJogador,
        posicao: Option<usize>,
        fracao: f32,
        numeros: [Arc<Galley>; 3],
        tres: &Tres,
        expandido: bool,
    ) {
        let cor = cor_da_classe(j.classe);
        let emblema = self.emblema(ui.ctx(), j.classe);
        let pintor = ui.painter().clone();
        pintor.rect_filled(linha, 5, Color32::from_black_alpha(0x5A));
        let barra = Rect::from_min_size(linha.min, vec2(linha.width() * fracao.clamp(0.0, 1.0), linha.height()));
        visual::degrade(&pintor, barra, 5.0, visual::escurecer(cor, 0.42, 0xF0), visual::escurecer(cor, 0.88, 0xF0));
        if barra.width() > 12.0 {
            // Brilho fino no alto da barra.
            pintor.hline(barra.x_range().shrink(5.0), barra.min.y + 1.5, Stroke::new(1.0_f32, branco(0x30)));
        }
        if j.voce {
            pintor.rect_stroke(linha, 5, Stroke::new(1.5_f32, visual::DOURADO), StrokeKind::Inside);
        }

        // Medalhão: o emblema oficial da classe, tingido na cor dela (como no medidor Abyss), num
        // círculo com anel da mesma cor.
        let centro = pos2(linha.min.x + 15.0, linha.center().y);
        pintor.circle_filled(centro, 11.0, Color32::from_rgb(0x12, 0x15, 0x1B));
        match emblema {
            Some(textura) => {
                let quadrado = Rect::from_center_size(centro, Vec2::splat(19.0));
                egui::Image::new(SizedTexture::new(textura.id(), quadrado.size())).tint(cor).paint_at(ui, quadrado);
            }
            None => {
                let sigla: String = j.classe.chars().take(2).collect();
                let sigla = if sigla.is_empty() { "?".to_string() } else { sigla };
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(sigla, fonte(9.5, true), cor));
                pintor.galley(centro - galley.size() / 2.0, galley, cor);
            }
        }
        pintor.circle_stroke(centro, 11.0, Stroke::new(1.5_f32, cor));

        // Selos da aba Tank, medidos à parte: o corte com "…" encurta o nome e eles ficam inteiros.
        let tank = self.aba == Aba::Tank;
        let mut selos = LayoutJob::default();
        if tank && j.segurando_aggro > 0 {
            trecho(&mut selos, &format!("  aggro {}", j.segurando_aggro), 10.0, true, Color32::from_rgb(0xFF, 0xB5, 0x47));
        }
        if tank && j.mortes > 0 {
            trecho(&mut selos, &format!("  ☠{}", j.mortes), 10.0, j.voce, Color32::from_rgb(0xFF, 0x8B, 0x8B));
        }
        let selos = (!selos.sections.is_empty()).then(|| montar(ui, selos));
        let largura_selos = selos.as_ref().map_or(0.0, |g| g.size().x);

        let inicio_nome = 32.0;
        let mut nome = LayoutJob::default();
        if let Some(posicao) = posicao {
            trecho(&mut nome, &format!("{posicao} "), 10.5, false, branco(0xAA));
        }
        trecho(&mut nome, &j.nome, 12.5, true, texto());
        if j.voce {
            trecho(&mut nome, " (você)", 10.5, true, visual::DOURADO);
        }
        trecho(&mut nome, if expandido { " ▾" } else { "" }, 10.5, false, branco(0xAA));
        nome.wrap = uma_linha((linha.width() - inicio_nome - tres.largura_total() - largura_selos - 8.0).max(40.0));
        let nome = montar(ui, nome);
        let largura_nome = nome.size().x;
        let y = linha.center().y - nome.size().y / 2.0;
        visual::com_sombra(&pintor, pos2(linha.min.x + inicio_nome, y), nome);
        if let Some(selos) = selos {
            let y = linha.center().y - selos.size().y / 2.0;
            visual::com_sombra(&pintor, pos2(linha.min.x + inicio_nome + largura_nome, y), selos);
        }
        tres.pintar(&pintor, linha, numeros, true);
    }

    /// Ao expandir: classe, level, GS e os números que não cabem na linha (CRIT, AVG, MAX...).
    fn ficha(&self, ui: &mut Ui, j: &LinhaJogador) {
        let mut job = linha_perfil(j);
        trecho(&mut job, "     ", 10.0, false, branco(0xCC));
        trecho(
            &mut job,
            &numeros_extras(self.aba, j.golpes, j.criticos, j.aparos, j.total, j.maximo),
            10.0,
            false,
            branco(0xCC),
        );
        job.wrap = uma_linha(ui.available_width() - 20.0);
        let galley = montar(ui, job);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), galley.size().y + 6.0), Sense::hover());
        ui.painter().galley(pos2(rect.min.x + 18.0, rect.min.y + 3.0), galley, texto());
    }

    /// Skill expandida: ícone, nome e os mesmos três números (a % é a parte no total do jogador),
    /// com uma barra fina na cor da classe pela parte.
    fn linha_skill(&mut self, ui: &mut Ui, s: &LinhaSkill, classe: &str, tres: &Tres) {
        let largura = ui.available_width();
        let formato = TextFormat::simple(fonte(11.0, false), texto().gamma_multiply(0.9));
        let numeros = numeros_da_linha(ui, textos(s.total, s.por_segundo, s.porcentagem), 11.0);

        // Moldura fixa: a linha não pula quando o ícone termina de baixar.
        let inicio_nome = 18.0 + 18.0 + 6.0;
        let mut nome = LayoutJob::single_section(s.nome.clone(), formato);
        nome.wrap = uma_linha((largura - inicio_nome - 8.0 - tres.largura_total()).max(40.0));
        let nome = montar(ui, nome);

        let altura = nome.size().y.max(20.0);
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, altura + 1.0), Sense::hover());
        let media = if s.golpes > 0 { s.total / f64::from(s.golpes) } else { 0.0 };
        let mut dica = format!(
            "{} golpes  ·  CRIT {}  ·  AVG {}  ·  MAX {}",
            s.golpes,
            p(razao(s.criticos, s.golpes), 0),
            compacto(media),
            compacto(s.maximo)
        );
        if self.aba == Aba::Dps {
            dica += &format!("  ·  {} pelas costas", p(razao(s.costas, s.golpes), 0));
        }
        resposta.on_hover_text(dica);
        let linha = Rect::from_min_size(rect.min, vec2(largura, altura));

        let cor = cor_da_classe(classe);
        let fundo = Rect::from_min_max(pos2(linha.min.x + 14.0, linha.min.y + 1.0), linha.max);
        let parte = Rect::from_min_size(
            fundo.min,
            vec2(fundo.width() * (s.porcentagem as f32).clamp(0.0, 1.0), fundo.height()),
        );
        ui.painter().rect_filled(parte, 3, Color32::from_rgba_unmultiplied(cor.r(), cor.g(), cor.b(), 0x2E));

        self.icone(ui, s.icone.as_deref(), linha);
        ui.painter().galley(pos2(linha.min.x + inicio_nome, linha.center().y - nome.size().y / 2.0), nome, texto());
        tres.pintar(ui.painter(), linha, numeros, false);
    }

    /// Buffs que o jogador recebeu na luta, do mais ativo para o menos: ícone e nome da skill que dá
    /// o buff e a parte da luta com ele ativo.
    fn buffs(&mut self, ui: &mut Ui, buffs: &[LinhaBuff]) {
        let largura = ui.available_width();
        let inicio_nome = 18.0 + 18.0 + 6.0;
        let titulo = montar(
            ui,
            LayoutJob::single_section("Buffs recebidos".into(), TextFormat::simple(fonte(10.0, false), branco(0x99))),
        );
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, titulo.size().y + 4.0), Sense::hover());
        ui.painter().galley(pos2(rect.min.x + inicio_nome, rect.min.y + 3.0), titulo, texto());
        resposta.on_hover_text(
            "Parte da luta com o buff ativo. O nome é o da skill que dá o buff; efeitos da mesma skill, \
             ou o mesmo buff vindo de jogadores diferentes, contam juntos.",
        );

        for b in buffs.iter().take(BUFFS_MOSTRADOS) {
            let formato = TextFormat::simple(fonte(11.0, false), texto().gamma_multiply(0.85));
            let parte = montar(ui, LayoutJob::single_section(p(b.fracao, 0), formato.clone()));
            let mut nome = LayoutJob::single_section(b.nome.clone(), formato);
            nome.wrap = uma_linha((largura - inicio_nome - parte.size().x - MARGEM_DIREITA - 16.0).max(40.0));
            let nome = montar(ui, nome);
            let altura = nome.size().y.max(18.0).max(parte.size().y);
            let (rect, _) = ui.allocate_exact_size(vec2(largura, altura + 2.0), Sense::hover());
            let linha = Rect::from_min_size(rect.min, vec2(largura, altura));

            self.icone(ui, b.icone.as_deref(), linha);
            let pintor = ui.painter();
            pintor.galley(pos2(linha.min.x + inicio_nome, linha.center().y - nome.size().y / 2.0), nome, texto());
            let x = linha.max.x - MARGEM_DIREITA - parte.size().x;
            pintor.galley(pos2(x, linha.center().y - parte.size().y / 2.0), parte, texto());
        }
    }

    /// Moldura fixa de 18 px com o ícone da skill (a linha não pula quando o ícone termina de baixar).
    fn icone(&mut self, ui: &Ui, caminho: Option<&Path>, linha: Rect) {
        let moldura = Rect::from_min_size(pos2(linha.min.x + 18.0, linha.min.y), vec2(18.0, 18.0));
        ui.painter().rect_filled(moldura, 3, branco(0x22));
        if let Some(icone) = caminho.and_then(|c| self.textura(ui.ctx(), c)) {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            ui.painter().image(icone.id(), moldura, uv, Color32::WHITE);
        }
    }

    /// Estado da luta (bolinha verde enquanto o placar anda), total da aba e o tempo da luta.
    fn rodape(&mut self, ui: &mut Ui, tabela: &Tabela) {
        let largura = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(largura, 24.0), Sense::hover());
        let pintor = ui.painter();
        pintor.hline(
            rect.x_range(),
            rect.min.y,
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xE6, 0xC0, 0x6A, 0x38)),
        );
        let meio = rect.center().y + 2.0;

        let copiado = self.copiado_em.is_some_and(|t| t.elapsed() < Duration::from_secs(3));
        let (cor, estado) = match self.vendo {
            _ if copiado => (visual::DOURADO, "Resumo copiado".to_string()),
            Some((_, inicio)) => (Color32::from_rgb(0x6E, 0xA8, 0xFF), format!("Luta das {}", hora_local(inicio))),
            None if self.em_luta() => (Color32::from_rgb(0x5B, 0xD1, 0x6B), "Em luta".to_string()),
            None => (branco(0x77), "Aguardando".to_string()),
        };
        let bolinha = pos2(rect.min.x + 7.0, meio);
        if self.em_luta() {
            pintor.circle_filled(bolinha, 6.5, Color32::from_rgba_unmultiplied(cor.r(), cor.g(), cor.b(), 0x40));
        }
        pintor.circle_filled(bolinha, 3.5, cor);
        let mut job = LayoutJob::default();
        trecho(&mut job, &estado, 11.0, true, branco(0xCC));
        if tabela.total > 0.0 {
            trecho(&mut job, &format!("   ·   Total {}", compacto(tabela.total)), 11.0, false, branco(0x99));
        }
        let esquerda = montar(ui, job);
        let fim_esquerda = rect.min.x + 18.0 + esquerda.size().x;
        pintor.galley(pos2(rect.min.x + 18.0, meio - esquerda.size().y / 2.0), esquerda, texto());

        let tempo = montar(
            ui,
            LayoutJob::single_section(
                minutos_e_segundos(self.placar.duracao),
                TextFormat::simple(fonte(13.0, true), texto()),
            ),
        );
        let largura_tempo = tempo.size().x;
        pintor.galley(pos2(rect.max.x - 6.0 - largura_tempo, meio - tempo.size().y / 2.0), tempo, texto());
        let mut livre_ate = rect.max.x - 6.0 - largura_tempo - 14.0;
        if let Some(ping) = self.ping {
            livre_ate = sinal_de_ping(ui, ping, rect, livre_ate, meio) - 10.0;
        }

        // Depois do total, se couber antes do ping: a Energia Odyle.
        self.odyle_no_rodape(ui, rect, meio, fim_esquerda, livre_ate);
    }

    /// Energia Odyle: o cristal (ou "Odyle", enquanto o ícone não baixou) e os valores.
    fn odyle_no_rodape(&mut self, ui: &Ui, rect: Rect, meio: f32, inicio: f32, livre_ate: f32) {
        let Some((basica, carregada)) = self.odyle else { return };
        let icone = self.icone_odyle(ui.ctx());
        let rotulo = if icone.is_some() { "   ·   " } else { "   ·   Odyle " };
        let formato = TextFormat::simple(fonte(11.0, false), branco(0x99));
        let rotulo = montar(ui, LayoutJob::single_section(rotulo.into(), formato));
        let mut job = LayoutJob::default();
        trecho(&mut job, &n(basica as f64, 0), 11.0, true, branco(0xCC));
        if let Some(carregada) = carregada {
            trecho(&mut job, &format!(" (+{})", n(carregada as f64, 0)), 11.0, false, branco(0x99));
        }
        let valor = montar(ui, job);
        let largura_icone = if icone.is_some() { 17.0 } else { 0.0 };
        let (largura_rotulo, largura_valor) = (rotulo.size().x, valor.size().x);
        if inicio + largura_rotulo + largura_icone + largura_valor > livre_ate {
            return;
        }
        let pintor = ui.painter();
        pintor.galley(pos2(inicio, meio - rotulo.size().y / 2.0), rotulo, texto());
        let x = inicio + largura_rotulo;
        if let Some(icone) = icone {
            let quadrado = Rect::from_center_size(pos2(x + 7.5, meio), Vec2::splat(15.0));
            egui::Image::new(SizedTexture::new(icone.id(), quadrado.size())).paint_at(ui, quadrado);
        }
        pintor.galley(pos2(x + largura_icone, meio - valor.size().y / 2.0), valor, texto());
        let area = Rect::from_min_max(pos2(x, rect.min.y), pos2(x + largura_icone + largura_valor, rect.max.y));
        ui.interact(area, ui.id().with("odyle"), Sense::hover()).on_hover_text(
            "Energia Odyle: a básica e, entre parênteses, a carregada, como o servidor manda no login \
             (ticket 60000001 do 0x610B) e a cada mudança (0x610C, como no uso de essência OD). Com o \
             Axon aberto depois do login, ela aparece na próxima mudança ou no próximo login. O máximo \
             (o /840 da tela) não vem no pacote.",
        );
    }

    /// Eventos de horário fixo e chefes de campo mortos, embaixo do rodapé. Recolhida, uma linha com os
    /// próximos que couberem, eventos e chefes juntos; expandida, os eventos um por linha, com o início
    /// e a contagem, e só o resumo dos chefes da região no fim. O clique no cabeçalho alterna.
    fn eventos(&mut self, ui: &mut Ui) {
        let agora = nucleo::agora();
        let lista: Vec<LinhaEvento> = eventos::em_ordem(agora)
            .into_iter()
            .map(|(evento, estado)| LinhaEvento {
                nome: evento.nome.into(),
                icone: evento
                    .icone
                    .as_ref()
                    .map_or(IconeDaLinha::Recomecar, |i| IconeDaLinha::Jogo(i.nome.into(), i.recorte)),
                estado,
                horario: eventos::horario(evento, agora),
                dica: format!("{}\n\n{}", evento.dica, eventos::ORIGEM),
            })
            .collect();
        let vistos = self.chefes_da_regiao();
        let mut mortos: Vec<_> = vistos.iter().flat_map(|v| &v.chefes).filter(|c| !c.vivo).collect();
        mortos.sort_by_key(|c| c.hora_ms);
        let mortos: Vec<LinhaEvento> = mortos.into_iter().map(|c| chefes::linha_do_chefe(c, agora)).collect();

        let expandida = self.config.eventos_expandidos;
        let largura = ui.available_width();
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, 22.0), Sense::click());
        let pintor = ui.painter();
        if resposta.hovered() {
            pintor.rect_filled(rect, 4, branco(0x0C));
        }
        pintor.hline(
            rect.x_range(),
            rect.min.y,
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xE6, 0xC0, 0x6A, 0x20)),
        );
        let meio = rect.center().y;
        let seta = if expandida { "▾" } else { "▸" };
        let formato = TextFormat::simple(fonte(11.0, false), branco(0x99));
        let seta = montar(ui, LayoutJob::single_section(seta.into(), formato));
        pintor.galley(pos2(rect.min.x + 7.0 - seta.size().x / 2.0, meio - seta.size().y / 2.0), seta, texto());

        let inicio = rect.min.x + 18.0;
        let fim = rect.max.x - 6.0;
        let mut dica_recolhida = String::new();
        if expandida {
            let mut job = LayoutJob::default();
            trecho(&mut job, "Eventos", 11.0, true, branco(0xCC));
            let titulo = montar(ui, job);
            pintor.galley(pos2(inicio, meio - titulo.size().y / 2.0), titulo, texto());
            let formato = TextFormat::simple(fonte(10.0, false), branco(0x77));
            let fuso = montar(ui, LayoutJob::single_section("horário de Brasília".into(), formato));
            pintor.galley(pos2(fim - fuso.size().x, meio - fuso.size().y / 2.0), fuso, texto());
        } else {
            // Os próximos, eventos e chefes pela contagem, enquanto couberem inteiros: o ícone (o
            // nome, sem ícone do jogo) e a contagem. No empate, o evento antes.
            let mut juntas: Vec<&LinhaEvento> = lista.iter().chain(&mortos).collect();
            juntas.sort_by_key(|l| eventos::ordem(l.estado));
            let mut x = inicio;
            let mut mostrados = Vec::new();
            for (i, linha) in juntas.into_iter().enumerate() {
                let mut job = LayoutJob::default();
                if i > 0 {
                    trecho(&mut job, "   ·   ", 11.0, false, branco(0x99));
                }
                let icone = match &linha.icone {
                    IconeDaLinha::Jogo(nome, recorte) => Some((nome.as_str(), *recorte, false)),
                    IconeDaLinha::Web(nome, recorte) => Some((nome.as_str(), *recorte, true)),
                    IconeDaLinha::Recomecar | IconeDaLinha::Moldura => {
                        trecho(&mut job, &format!("{} ", linha.nome), 11.0, false, branco(0x99));
                        None
                    }
                };
                let antes = montar(ui, job);
                let mut job = LayoutJob::default();
                contagem_do_evento(&mut job, linha.estado);
                let contagem = montar(ui, job);
                let largura_icone = if icone.is_some() { 20.0 } else { 0.0 };
                let (largura_antes, largura_contagem) = (antes.size().x, contagem.size().x);
                if x + largura_antes + largura_icone + largura_contagem > fim {
                    break;
                }
                pintor.galley(pos2(x, meio - antes.size().y / 2.0), antes, texto());
                x += largura_antes;
                if let Some((nome, recorte, web)) = icone {
                    let quadrado = Rect::from_center_size(pos2(x + 8.0, meio), Vec2::splat(16.0));
                    if web {
                        self.imagem_web(ui, nome, recorte, quadrado);
                    } else {
                        self.icone_do_evento(ui, nome, recorte, quadrado);
                    }
                    x += largura_icone;
                }
                pintor.galley(pos2(x, meio - contagem.size().y / 2.0), contagem, texto());
                x += largura_contagem;
                mostrados.push(format!("{} {}", linha.nome, texto_da_contagem(linha.estado)));
            }
            // Um tooltip só, com o nome do que está na linha: o clique continua sendo da linha toda.
            mostrados.push(String::new());
            mostrados.push("Clique para ver todos, com o horário de cada um.".into());
            dica_recolhida = mostrados.join("\n");
        }
        let dica = if expandida { "Recolher os eventos" } else { dica_recolhida.as_str() };
        if resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            self.config.eventos_expandidos = !expandida;
            self.aplicar_config();
        }
        if !expandida {
            return;
        }

        for linha in &lista {
            self.linha_do_evento(ui, linha, inicio, fim);
        }
        if let Some(vistos) = &vistos {
            self.titulo_dos_chefes(ui, vistos, inicio, fim);
        }
        ui.add_space(2.0);
    }

    /// Uma linha da área expandida: o ponto do estado, o ícone, o nome, o início e a contagem.
    fn linha_do_evento(&mut self, ui: &mut Ui, linha: &LinhaEvento, inicio: f32, fim: f32) {
        let (rect, resposta) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
        let pintor = ui.painter();
        let meio = rect.center().y;
        let verde = Color32::from_rgb(0x5B, 0xD1, 0x6B);
        // Verde aberto, dourado faltando até 10 min, apagado no resto.
        let (cor, destaque) = match linha.estado {
            eventos::Estado::Aberto(_) => (verde, true),
            eventos::Estado::Fechado(s) if s <= 600 => (visual::DOURADO, true),
            eventos::Estado::Fechado(_) => (branco(0x40), false),
        };
        let ponto = pos2(rect.min.x + 7.0, meio);
        if matches!(linha.estado, eventos::Estado::Aberto(_)) {
            let halo = Color32::from_rgba_unmultiplied(verde.r(), verde.g(), verde.b(), 0x40);
            pintor.circle_filled(ponto, 6.0, halo);
        }
        pintor.circle_filled(ponto, 3.0, cor);

        // Ícone em 16 px; nos resets, a seta de recomeçar no lugar dele.
        let quadrado = Rect::from_center_size(pos2(inicio + 8.0, meio), Vec2::splat(16.0));
        match &linha.icone {
            IconeDaLinha::Jogo(nome, recorte) => self.icone_do_evento(ui, nome, *recorte, quadrado),
            IconeDaLinha::Web(nome, recorte) => self.imagem_web(ui, nome, *recorte, quadrado),
            IconeDaLinha::Recomecar => {
                let formato = TextFormat::simple(fonte(13.0, false), branco(0x99));
                let seta = montar(ui, LayoutJob::single_section("↻".into(), formato));
                pintor.galley(quadrado.center() - seta.size() / 2.0, seta, texto());
            }
            IconeDaLinha::Moldura => {
                pintor.rect_filled(quadrado, 3, branco(0x22));
            }
        }
        let formato = TextFormat::simple(fonte(11.0, false), if destaque { texto() } else { branco(0xBB) });
        let nome = montar(ui, LayoutJob::single_section(linha.nome.clone(), formato));
        pintor.galley(pos2(inicio + 22.0, meio - nome.size().y / 2.0), nome, texto());
        let mut job = LayoutJob::default();
        contagem_do_evento(&mut job, linha.estado);
        let contagem = montar(ui, job);
        pintor.galley(pos2(fim - contagem.size().x, meio - contagem.size().y / 2.0), contagem, texto());
        // O início numa coluna própria, alinhado à direita antes da contagem mais larga.
        let formato = TextFormat::simple(fonte(10.0, false), branco(0x77));
        let horario = montar(ui, LayoutJob::single_section(linha.horario.clone(), formato));
        pintor.galley(pos2(fim - 84.0 - horario.size().x, meio - horario.size().y / 2.0), horario, texto());
        resposta.on_hover_text(&linha.dica);
    }

    /// "Chefes de Altgard   20 vivos, 4 mortos" depois dos eventos, sem os chefes um a um: o clique abre
    /// a tela de chefes. Com a lista antiga, a hora dela à direita.
    fn titulo_dos_chefes(&mut self, ui: &mut Ui, vistos: &chefes::ChefesDaRegiao, inicio: f32, fim: f32) {
        let (rect, resposta) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
        let pintor = ui.painter();
        if resposta.hovered() {
            pintor.rect_filled(rect, 4, branco(0x0C));
        }
        let meio = rect.center().y;
        let formato = TextFormat::simple(fonte(11.0, false), branco(0x99));
        let coroa = montar(ui, LayoutJob::single_section("♛".into(), formato));
        pintor.galley(pos2(rect.min.x + 7.0 - coroa.size().x / 2.0, meio - coroa.size().y / 2.0), coroa, texto());
        let vivos = vistos.chefes.iter().filter(|c| c.vivo).count();
        let mortos = vistos.chefes.len() - vivos;
        let plural = |n: usize, um: &str, varios: &str| format!("{n} {}", if n == 1 { um } else { varios });
        let mut job = LayoutJob::default();
        trecho(&mut job, &format!("Chefes de {}", vistos.regiao), 11.0, true, branco(0xCC));
        let contagem = format!("   {}, {}", plural(vivos, "vivo", "vivos"), plural(mortos, "morto", "mortos"));
        trecho(&mut job, &contagem, 10.0, false, branco(0x88));
        let titulo = montar(ui, job);
        pintor.galley(pos2(inicio, meio - titulo.size().y / 2.0), titulo, texto());
        if let Some(desde) = vistos.antiga_desde {
            let formato = TextFormat::simple(fonte(10.0, false), visual::AMARELO);
            let aviso = montar(ui, LayoutJob::single_section(format!("lista das {}", hora_local(desde)), formato));
            pintor.galley(pos2(fim - aviso.size().x, meio - aviso.size().y / 2.0), aviso, texto());
        }
        let dica = "Abrir a tela de chefes de campo, com os vivos e os mortos.";
        if resposta.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica).clicked() {
            self.tela = Tela::Chefes;
        }
    }

    /// Imagem da CDN só na memória (retrato de chefe, ícone de drop) com o recorte dado. Enquanto
    /// chega, o skeleton pulsando (até ESPERA_DA_IMAGEM); sem ela (falha ou demora), a moldura parada.
    fn imagem_web(&mut self, ui: &Ui, nome: &str, recorte: [f32; 4], quadrado: Rect) {
        if !self.imagens.contains_key(nome) {
            match dados_jogo::imagem(nome) {
                Busca::Pronto(png) => {
                    let textura = decodificar(std::io::Cursor::new(png.as_slice()))
                        .map(|imagem| ui.ctx().load_texture(nome, imagem, opcoes_de_textura()));
                    self.imagens.insert(nome.to_string(), textura);
                }
                Busca::Buscando => {
                    let desde = *self.imagens_pedidas.entry(nome.to_string()).or_insert_with(Instant::now);
                    let esperando = desde.elapsed() < ESPERA_DA_IMAGEM;
                    self.carregando |= esperando;
                    ui.painter().rect_filled(quadrado, 3, if esperando { pulso(ui) } else { branco(0x22) });
                    return;
                }
                Busca::Falhou => {
                    ui.painter().rect_filled(quadrado, 3, branco(0x22));
                    return;
                }
            }
        }
        let Some(Some(textura)) = self.imagens.get(nome) else {
            ui.painter().rect_filled(quadrado, 3, branco(0x22));
            return;
        };
        let [x0, y0, x1, y1] = recorte;
        egui::Image::new(SizedTexture::new(textura.id(), quadrado.size()))
            .uv(Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)))
            .corner_radius(3)
            .paint_at(ui, quadrado);
    }

    /// Ícone de evento da CDN do jogo (cache em disco) com o recorte dado; enquanto não baixou, a
    /// moldura.
    fn icone_do_evento(&mut self, ui: &Ui, nome: &str, recorte: [f32; 4], quadrado: Rect) {
        ui.painter().rect_filled(quadrado, 3, branco(0x22));
        if !self.icones_eventos.contains_key(nome) {
            // Como no emblema: só pergunta ao catálogo logo depois de ler o placar (cada pergunta
            // olha o disco).
            if self.lido_em.elapsed() > Duration::from_millis(100) {
                return;
            }
            let Some(caminho) = dados_jogo::icone_do_jogo(nome) else { return };
            self.icones_eventos.insert(nome.to_string(), caminho);
        }
        let caminho = self.icones_eventos[nome].clone();
        let Some(textura) = self.textura(ui.ctx(), &caminho) else { return };
        let [x0, y0, x1, y1] = recorte;
        egui::Image::new(SizedTexture::new(textura.id(), quadrado.size()))
            .uv(Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)))
            .corner_radius(3)
            .paint_at(ui, quadrado);
    }

    /// Barra compacta: numa linha só, o alvo e o HP, o seu DPS, o do grupo e o ping.
    fn barra_compacta(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let (rect, resposta) = ui.allocate_exact_size(vec2(ui.available_width() - 26.0, 24.0), Sense::hover());
            resposta.on_hover_text(
                "Barra compacta. \"grupo\": o dano de todos dividido pelo tempo da luta; \"Você\": o seu DPS, \
                 do seu primeiro ao último golpe (o mesmo da aba DPS).",
            );
            let pintor = ui.painter().clone();
            let meio = rect.center().y;
            let cor = if self.em_luta() { Color32::from_rgb(0x5B, 0xD1, 0x6B) } else { branco(0x77) };
            pintor.circle_filled(pos2(rect.min.x + 7.0, meio), 3.5, cor);

            // Da direita para a esquerda: ping, DPS; o alvo fica com o que sobrar.
            let mut x = rect.max.x - 4.0;
            if let Some(ping) = self.ping {
                x = sinal_de_ping(ui, ping, rect, x, meio) - 10.0;
            }
            let dano = &self.placar.dano;
            let segundos = self.placar.duracao as f64 / TICKS_POR_SEGUNDO as f64;
            let mut job = LayoutJob::default();
            if let Some(voce) = dano.jogadores.iter().find(|j| j.voce) {
                trecho(&mut job, "Você ", 10.0, false, branco(0xBB));
                trecho(&mut job, &format!("{}/s", compacto(voce.por_segundo)), 12.0, true, visual::DOURADO);
            }
            if dano.total > 0.0 && segundos > 0.0 {
                let separador = if job.sections.is_empty() { "" } else { "  ·  " };
                let grupo = format!("{separador}grupo {}/s", compacto(dano.total / segundos.max(1.0)));
                trecho(&mut job, &grupo, 10.0, false, branco(0xBB));
            }
            let dps = montar(ui, job);
            x -= dps.size().x;
            let altura_dps = dps.size().y;
            visual::com_sombra(&pintor, pos2(x, meio - altura_dps / 2.0), dps);

            // HP e "derrota em" antes do nome: sem espaço, a reticência corta o nome.
            let mut job = LayoutJob::default();
            match &self.placar.alvo {
                Some(alvo) => {
                    if alvo.morto {
                        trecho(&mut job, "Derrotado  ", 10.0, true, visual::DOURADO);
                    } else if let (Some(hp), Some(maximo)) = (alvo.hp, alvo.hp_maximo) {
                        let fracao = format!("{}  ", p(hp as f64 / maximo as f64, 1));
                        trecho(&mut job, &fracao, 11.0, true, visual::VERMELHO_CLARO);
                    }
                    if let Some(segundos) = alvo.derrota_em {
                        let falta = minutos_e_segundos((segundos * TICKS_POR_SEGUNDO as f64) as i64);
                        trecho(&mut job, &format!("derrota em {falta}  "), 10.0, false, branco(0xCC));
                    }
                    trecho(&mut job, &nome_do_alvo(alvo), 12.0, true, texto());
                }
                None if self.fluxo.is_none() => trecho(&mut job, self.procurando(), 10.0, false, branco(0x99)),
                None => trecho(&mut job, "Aguardando luta", 10.0, false, branco(0x99)),
            }
            let inicio = rect.min.x + 18.0;
            job.wrap = uma_linha((x - 10.0 - inicio).max(40.0));
            let alvo = montar(ui, job);
            let altura_alvo = alvo.size().y;
            visual::com_sombra(&pintor, pos2(inicio, meio - altura_alvo / 2.0), alvo);

            if visual::botao_icone(ui, "▭", 14.0).on_hover_text("Voltar ao medidor completo").clicked() {
                self.alternar_compacta();
            }
        });
    }

    fn logo(&mut self, ctx: &egui::Context) -> Option<TextureHandle> {
        if self.logo.is_none() {
            let imagem = decodificar(std::io::Cursor::new(LOGO));
            self.logo = Some(imagem.map(|imagem| ctx.load_texture("logo", imagem, opcoes_de_textura())));
        }
        self.logo.clone().flatten()
    }

    /// Emblema oficial da classe (CDN do jogo), o mesmo para todos os jogadores dela. None enquanto
    /// não baixou.
    fn emblema(&mut self, ctx: &egui::Context, classe: &'static str) -> Option<TextureHandle> {
        if !self.emblemas.contains_key(classe) {
            // Só pergunta ao catálogo a cada leitura do placar: cada pergunta olha o disco.
            if self.lido_em.elapsed() > Duration::from_millis(100) {
                return None;
            }
            self.emblemas.insert(classe, dados_jogo::emblema_classe(classe)?);
        }
        let caminho = self.emblemas[classe].clone();
        self.textura(ctx, &caminho)
    }

    /// Cristal da Energia Odyle (CDN do jogo). None enquanto não baixou.
    fn icone_odyle(&mut self, ctx: &egui::Context) -> Option<TextureHandle> {
        if self.icone_odyle.is_none() && self.lido_em.elapsed() <= Duration::from_millis(100) {
            self.icone_odyle = dados_jogo::icone_odyle();
        }
        let caminho = self.icone_odyle.clone()?;
        self.textura(ctx, &caminho)
    }

    fn status(&mut self, ui: &mut Ui) {
        // O endereço do servidor não aparece: o rodapé só avisa o que ainda falta ou o que parou.
        let mut partes: Vec<String> = Vec::new();
        match &self.erro_captura {
            Some(erro) => partes.push(erro.clone()),
            // A captura ainda espera o clique no aviso do firewall: não há o que procurar.
            None if self.pedir_firewall => {}
            None => {
                if self.fluxo.is_none() {
                    partes.push(self.procurando().into());
                }
                if self.catalogo.quantidade() == 0 {
                    partes.push("baixando nomes das skills...".into());
                }
            }
        }
        // Com o clique atravessando, o mouse não alcança o overlay: o rodapé diz como voltar.
        if self.atravessando {
            partes.push(match bandeja::atalho(bandeja::ALTERNAR_CLIQUE) {
                Some((atalho, false)) => {
                    format!("clique atravessando: {atalho} ou o menu da bandeja desliga")
                }
                _ => "clique atravessando: o menu da bandeja desliga".into(),
            });
        }
        // Versão do build, para os amigos dizerem qual usam; com versão nova no GitHub, o botão
        // Atualizar, e sem ela o Verificar atualização.
        let versao = concat!("v", env!("CARGO_PKG_VERSION"));
        let estado = self.atualizacao.estado();
        partes.push(match &estado {
            Estado::Nada => versao.to_string(),
            Estado::Consultando => format!("{versao}: verificando..."),
            Estado::EmDia => format!("{versao}: é a versão mais nova"),
            Estado::SemResposta => format!("{versao}: o GitHub não respondeu"),
            Estado::Disponivel(n) => format!("{versao} → v{}", n.versao),
            Estado::Baixando(n) => format!("{versao} → v{}: baixando...", n.versao),
            Estado::Falhou(n, erro) => format!("{versao} → v{}: {erro}", n.versao),
            Estado::Pronta(n) => match &self.reabertura {
                Some(Err(erro)) => format!("v{} instalada, mas não deu para {erro}: abra o Axon de novo", n.versao),
                _ => format!("v{} instalada, reabrindo...", n.versao),
            },
        });
        let status = RichText::new(partes.join("  ·  ")).font(fonte(10.0, false)).color(branco(0x99));
        let baixar = "Baixa a versão nova do GitHub, confere o arquivo e reabre o Axon";
        let (rotulo, dica, instalar) = match estado {
            Estado::Disponivel(_) => ("Atualizar", baixar, true),
            Estado::Falhou(..) => ("Tentar de novo", baixar, true),
            Estado::Nada | Estado::EmDia | Estado::SemResposta => {
                ("Verificar atualização", "Consulta no GitHub se saiu versão nova, sem reabrir o Axon", false)
            }
            Estado::Consultando | Estado::Baixando(_) | Estado::Pronta(_) => {
                ui.add(egui::Label::new(status).wrap());
                return;
            }
        };
        // Botão à direita; o texto ocupa o resto, alinhado à esquerda como sem o botão. O de
        // instalar fica em destaque; o de verificar, discreto.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, rotulo, instalar).on_hover_text(dica).clicked() {
                    if instalar {
                        self.atualizacao.atualizar();
                    } else {
                        self.atualizacao.verificar();
                    }
                }
                ui.with_layout(egui::Layout::left_to_right(Align::Center), |ui| {
                    ui.add(egui::Label::new(status).wrap());
                });
            });
        });
    }

    /// Primeira abertura (ou exe em outra pasta): explica a regra do firewall, que só é criada com o
    /// clique. "Agora não" abre a captura sem a regra; se o firewall barrar, o rodapé avisa, e o aviso
    /// volta na próxima abertura.
    fn aviso_firewall(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Liberar o Axon no Firewall do Windows").font(fonte(12.0, true)).color(texto()));
        ui.add_space(4.0);
        let explicacao = "Para medir, o Axon precisa receber os pacotes que o servidor do jogo manda para o seu PC. \
            Liberar cria a regra de entrada \"Axon (captura)\", só para este programa, no lugar das regras antigas \
            dele (inclusive um bloqueio deixado pelo aviso do Windows). Para desfazer: Firewall do Windows > \
            Configurações avançadas > Regras de Entrada.";
        ui.add(egui::Label::new(RichText::new(explicacao).font(fonte(11.0, false)).color(branco(0xBB))).wrap());
        ui.add_space(8.0);
        let (liberar, agora_nao) = ui
            .horizontal(|ui| {
                let liberar = botao(ui, "Liberar", true).on_hover_text("Cria a regra e começa a medir");
                ui.add_space(6.0);
                let dica = "Começa a medir sem a regra; se o firewall barrar, o rodapé avisa. O aviso volta na próxima abertura.";
                let agora_nao = botao(ui, "Agora não", false).on_hover_text(dica);
                (liberar.clicked(), agora_nao.clicked())
            })
            .inner;
        if liberar || agora_nao {
            self.comecar_captura(liberar);
        }
    }

    /// Depois do clique no aviso do firewall. No replay (--pedir-firewall) só some o aviso.
    fn comecar_captura(&mut self, liberar: bool) {
        self.pedir_firewall = false;
        if self.replay {
            return;
        }
        if liberar {
            crate::firewall::liberar();
        }
        (self.captura, self.erro_captura) = iniciar_captura(&self.sessao);
        // O rodapé espera 15 s de captura aberta antes de acusar o firewall.
        self.captura_desde = Instant::now();
    }

    /// Sem o servidor do jogo ainda, diz o que os contadores da captura apontam.
    fn procurando(&self) -> &'static str {
        let Some(captura) = &self.captura else { return PROCURANDO };
        let c = &captura.contadores;
        procurando(
            c.tcp_entrada.load(Ordering::Relaxed),
            c.tcp_saida.load(Ordering::Relaxed),
            self.captura_desde.elapsed(),
            self.jogo_desde.map(|t| t.elapsed()),
        )
    }

    /// Recolhido: só a aba "Overlay ›" colada na borda; clicar traz a janela de volta.
    fn aba_recolhida(&mut self, ui: &mut Ui, lado: Lado) {
        let rect = ui.max_rect();
        let resposta = ui
            .interact(rect, ui.id().with("aba recolhida"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Abrir o medidor");
        // Canto reto do lado da borda da tela, arredondado do lado de dentro.
        let raio = match lado {
            Lado::Esquerda => CornerRadius { nw: 0, sw: 0, ne: 6, se: 6 },
            Lado::Direita => CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 },
        };
        let fundo = self.config.alfa_do_fundo();
        let alfa = if resposta.hovered() { fundo.max(0xF2) } else { fundo };
        ui.painter().rect_filled(rect, raio, Color32::from_rgba_unmultiplied(0x10, 0x14, 0x18, alfa));
        ui.painter().rect_stroke(rect, raio, Stroke::new(1.0_f32, branco(0x33)), StrokeKind::Inside);
        let rotulo = TEXTO_DA_ABA[usize::from(lado == Lado::Direita)];
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(rotulo.to_string(), fonte(12.0, true), texto()));
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, texto());
        if resposta.clicked() {
            let largura = (LARGURA * ui.ctx().pixels_per_point()).round() as i32;
            self.dobra.abrir(self.janela, largura);
        }
    }

    fn textura(&mut self, ctx: &egui::Context, caminho: &Path) -> Option<TextureHandle> {
        if let Some(pronta) = self.icones.get(caminho) {
            return pronta.clone();
        }
        let textura = decodificar_png(caminho)
            .map(|imagem| ctx.load_texture(caminho.to_string_lossy(), imagem, opcoes_de_textura()));
        self.icones.insert(caminho.to_path_buf(), textura.clone());
        textura
    }

    /// Só no debug: recolhe sozinho aos 2 s e, com --recolher-e-voltar, abre de novo aos 4,5 s.
    fn testar_dobra(&mut self, ctx: &egui::Context) {
        let Some((inicio, voltar, recolheu)) = self.teste_dobra else { return };
        let s = inicio.elapsed().as_secs_f32();
        if !recolheu && s > 2.0 && self.dobra.aberto() && self.janela != 0 {
            self.dobra.recolher(self.janela);
            self.teste_dobra = Some((inicio, voltar, true));
        }
        if voltar && s > 4.5 && matches!(self.dobra, Dobra::Recolhido { .. }) {
            self.dobra.abrir(self.janela, (LARGURA * ctx.pixels_per_point()).round() as i32);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl eframe::App for Overlay {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.janela = manter_sem_ativar(frame);
        self.carregando = false;
        // Pelo winit (WS_EX_TRANSPARENT): ele guarda o estado e não apaga o bit ao recalcular o estilo.
        if bandeja::atravessando() != self.atravessando {
            self.atravessando = !self.atravessando;
            ctx.send_viewport_cmd(ViewportCommand::MousePassthrough(self.atravessando));
        }
        if (ctx.zoom_factor() - self.config.zoom).abs() > 0.001 {
            ctx.set_zoom_factor(self.config.zoom);
        }
        if self.lido_em.elapsed() >= self.intervalo() {
            self.ler_placar();
        }
        let (pediu_resumo, pediu_compacta) = bandeja::pedidos();
        if pediu_resumo {
            self.copiar_resumo(ctx);
        }
        if pediu_compacta {
            self.alternar_compacta();
        }
        if self.salvo_em.elapsed() >= SALVAR_A_CADA {
            self.salvar_memoria();
        }
        // Exe trocado: grava a memória antes de a versão nova ler, abre a nova e fecha esta.
        if self.reabertura.is_none() && matches!(self.atualizacao.estado(), Estado::Pronta(_)) {
            self.salvar_memoria();
            let aberta = atualizacao::abrir_novo();
            if aberta.is_ok() {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            self.reabertura = Some(aberta);
        }
        self.testar_dobra(ctx);

        let aba = tamanho_da_aba(ctx);
        let animando = self.dobra.quadro(ctx, aba, vec2(LARGURA, self.altura));
        if animando {
            // O deslize precisa de um quadro atrás do outro (a janela não recebe input que os peça).
            ctx.request_repaint();
        } else {
            // Sem isso o egui só redesenha com input, e o placar congelaria.
            ctx.request_repaint_after(self.intervalo());
        }

        if let Some(codigo) = self.drops_inicial.take() {
            self.alternar_drops(codigo, ctx.pixels_per_point());
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            if let Dobra::Recolhido { lado, .. } = self.dobra {
                self.aba_recolhida(ui, lado);
                return;
            }

            // Arrasta a janela pelo fundo; botões e linhas ficam por cima e pegam o clique. click_and_drag
            // e não só drag: assim um clique parado não mexe a janela.
            let fundo = ui.interact(ui.max_rect(), ui.id().with("arrastar"), Sense::click_and_drag());
            if fundo.drag_started() && self.dobra.aberto() {
                self.arraste = cursor_e_canto(self.janela);
            }
            match self.arraste {
                Some((cursor, canto)) if fundo.dragged() => seguir_cursor(self.janela, cursor, canto),
                _ => self.arraste = None,
            }

            // O medidor numa folha de LARGURA e, com os drops abertos, o painel em outra ao lado, com um
            // vão transparente entre as duas.
            let folha = egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(0x0D, 0x10, 0x15, self.config.alfa_do_fundo()))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0xE6, 0xC0, 0x6A, 0x55)))
                .corner_radius(8)
                .inner_margin(8);
            let lado_do_painel = self.painel.as_ref().map(|p| p.lado);
            let inicio = ui.max_rect().min;
            let x_medidor =
                if lado_do_painel == Some(Lado::Esquerda) { drops::LARGURA_PAINEL + drops::VAO } else { 0.0 };
            let medidor = Rect::from_min_size(inicio + vec2(x_medidor, 0.0), vec2(LARGURA, ui.max_rect().height()));
            let quadro = ui.scope_builder(egui::UiBuilder::new().max_rect(medidor), |ui| {
                folha.show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.set_width(ui.available_width());
                    self.conteudo(ui);
                })
            });
            let mut altura = quadro.inner.response.rect.height().ceil();
            if let Some(lado) = lado_do_painel {
                // A folha do painel pode crescer até a altura da área (do jogo); passando disso, rola.
                let ppp = ctx.pixels_per_point();
                let limite = recolher::altura_da_area(self.janela).map_or(900.0, |h| h as f32 / ppp) - 16.0;
                let x = if lado == Lado::Esquerda { 0.0 } else { LARGURA + drops::VAO };
                let area = Rect::from_min_size(inicio + vec2(x, 0.0), vec2(drops::LARGURA_PAINEL, limite));
                let painel = ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                    folha.show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.set_width(ui.available_width());
                        self.painel_drops(ui, limite - 16.0);
                    })
                });
                altura = altura.max(painel.inner.response.rect.height().ceil());
            }

            // Tamanho pelo conteúdo (o SizeToContent do WPF): só manda o comando quando a altura, a
            // largura (painel de drops) ou a escala mudam. Recolhendo ou voltando, quem manda no
            // tamanho é a animação.
            let largura = self.largura_janela();
            let escala = ctx.pixels_per_point();
            let escala_mudou = (self.escala_aplicada - escala).abs() > 0.001;
            let mudou = (altura - self.altura).abs() >= 1.0 || (largura - self.largura_aplicada).abs() >= 1.0;
            if self.dobra.aberto() && (mudou || escala_mudou) {
                self.altura = altura;
                self.largura_aplicada = largura;
                self.escala_aplicada = escala;
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(largura, altura)));
            }
        });
        // Skeleton pulsando: quadros seguidos só enquanto algo carrega.
        if self.carregando {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.bandeja = None;
        self.captura = None;
        self.salvar_memoria();
    }
}

/// Ping terminando em `fim`: barrinhas na cor da faixa e os ms, com a explicação no mouse. Devolve o
/// x onde o desenho começa.
fn sinal_de_ping(ui: &Ui, ping: i64, rect: Rect, fim: f32, meio: f32) -> f32 {
    let pintor = ui.painter();
    let ms = ping as f64 * 1000.0 / TICKS_POR_SEGUNDO as f64;
    let (cor, acesas) = match ms {
        ..=60.0 => (Color32::from_rgb(0x5B, 0xD1, 0x6B), 3),
        ..=120.0 => (visual::AMARELO, 2),
        ..=200.0 => (Color32::from_rgb(0xFF, 0xB5, 0x47), 2),
        _ => (visual::VERMELHO_CLARO, 1),
    };
    let formato = TextFormat::simple(fonte(11.0, false), branco(0xBB));
    let valor = montar(ui, LayoutJob::single_section(format!("{} ms", n(ms.round(), 0)), formato));
    let x_valor = fim - valor.size().x;
    let altura_valor = valor.size().y;
    pintor.galley(pos2(x_valor, meio - altura_valor / 2.0), valor, texto());
    for i in 0..3 {
        let altura = 4.0 + 3.0 * i as f32;
        let x = x_valor - 17.0 + 4.5 * i as f32;
        let barra = Rect::from_min_max(pos2(x, meio + 5.0 - altura), pos2(x + 3.0, meio + 5.0));
        pintor.rect_filled(barra, 1, if i < acesas { cor } else { branco(0x40) });
    }
    let area = Rect::from_min_max(pos2(x_valor - 18.0, rect.min.y), pos2(fim, rect.max.y));
    ui.interact(area, ui.id().with("ping"), Sense::hover()).on_hover_text(
        "Ping: o menor tempo de ida e volta até o servidor do jogo nos últimos 10 s, medido no TCP (do envio \
         do seu PC ao ACK do servidor). É a latência da rede; o número que o jogo mostra pode sair um pouco maior.",
    );
    x_valor - 18.0
}

/// Nome do questlog; sem ele, o código do NPC ou o id da entidade.
fn nome_do_alvo(alvo: &Alvo) -> String {
    if !alvo.nome.is_empty() {
        alvo.nome.clone()
    } else if alvo.codigo != 0 {
        format!("NPC {}", alvo.codigo)
    } else if alvo.chefe {
        format!("Chefe #{}", alvo.entidade)
    } else {
        format!("Alvo #{}", alvo.entidade)
    }
}

/// "Ocultar nomes": os outros jogadores pelo nome da classe; o seu continua.
fn ocultar_nomes(placar: &mut Placar) {
    for tabela in [&mut placar.dano, &mut placar.dano_recebido, &mut placar.cura] {
        for j in tabela.jogadores.iter_mut().filter(|j| !j.voce) {
            j.nome = nome_oculto(j.classe);
        }
    }
}

fn nome_oculto(classe: &str) -> String {
    if classe.is_empty() { "Jogador".into() } else { classe.to_string() }
}

/// O texto do "Copiar resumo": alvo, duração e total, e os primeiros da aba DPS (mais você, se ficou
/// fora deles) com o DPS e a parte no dano.
fn resumo(placar: &Placar, em_linhas: bool) -> String {
    let dano = &placar.dano;
    let alvo = placar.alvo.as_ref().map(|a| a.nome.as_str()).filter(|nome| !nome.is_empty()).unwrap_or("Luta");
    let cabeca = format!("{alvo} · {} · total {}", minutos_e_segundos(placar.duracao), compacto(dano.total));
    let voce = dano.jogadores.iter().position(|j| j.voce);
    let (primeiros, voce_abaixo) = linhas_mostradas(dano.jogadores.len(), voce, LIMITE_DE_LINHAS);
    let linhas: Vec<String> = primeiros
        .chain(voce_abaixo)
        .map(|i| {
            let j = &dano.jogadores[i];
            format!("{}. {} {}/s {}", i + 1, j.nome, compacto(j.por_segundo), p(j.porcentagem, 1))
        })
        .collect();
    if em_linhas { format!("{cabeca}\n{}", linhas.join("\n")) } else { format!("{cabeca} | {}", linhas.join(" · ")) }
}

/// Tamanho da aba recolhida, em pontos: o texto com folga.
fn tamanho_da_aba(ctx: &egui::Context) -> Vec2 {
    let largura = TEXTO_DA_ABA
        .iter()
        .map(|t| ctx.fonts_mut(|f| f.layout_no_wrap(t.to_string(), fonte(12.0, true), texto()).size()))
        .fold(Vec2::ZERO, |maior, t| maior.max(t));
    (largura + vec2(24.0, 12.0)).ceil()
}

/// Para a amostra das configurações: a sua linha na aba ativa ou, sem ela, um esboço com o seu nome
/// e o level e GS guardados.
fn amostra(tabela: &Tabela, eu: Option<String>, perfis: &IndexMap<String, PerfilJogador>) -> LinhaJogador {
    if let Some(j) = tabela.jogadores.iter().find(|j| j.voce) {
        return j.clone();
    }
    let nome = eu.unwrap_or_else(|| "Você".into());
    let perfil = perfis.get(&nome).copied().unwrap_or_default();
    LinhaJogador {
        nome,
        nivel: perfil.nivel,
        nivel_lembrado: perfil.nivel > 0,
        poder: perfil.poder,
        poder_lembrado: perfil.poder > 0,
        voce: true,
        ..LinhaJogador::default()
    }
}

/// A lista mostra os 10 primeiros; quem está abaixo (você incluído) só aparece na sua linha à parte.
const LIMITE_DE_LINHAS: usize = 10;

/// Índices das linhas mostradas: as `limite` primeiras e, à parte, a sua se ficou abaixo delas.
fn linhas_mostradas(total: usize, voce: Option<usize>, limite: usize) -> (std::ops::Range<usize>, Option<usize>) {
    (0..total.min(limite), voce.filter(|&i| i >= limite))
}

/// Traço discreto entre os 10 primeiros e a sua linha, quando você está mais abaixo.
fn separador(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().hline(rect.x_range().shrink(6.0), y, Stroke::new(1.0_f32, branco(0x33)));
}

const PROCURANDO: &str = "Procurando o servidor do jogo...";

/// `entrada`/`saida`: segmentos TCP capturados desde que a captura abriu, há `aberta`; `com_jogo`:
/// há quanto tempo a janela do jogo apareceu. Espera antes de acusar algo, para não piscar aviso na
/// abertura nem na tela de login.
fn procurando(entrada: u64, saida: u64, aberta: Duration, com_jogo: Option<Duration>) -> &'static str {
    if aberta >= Duration::from_secs(15) && saida >= 20 && entrada == 0 {
        "O firewall está barrando o que chega da internet: libere o Axon no Firewall do Windows ou no antivírus."
    } else if com_jogo.is_some_and(|t| t >= Duration::from_secs(120)) {
        "Jogo aberto e servidor não encontrado: VPN, ExitLag e similares podem esconder o tráfego do jogo."
    } else {
        PROCURANDO
    }
}

fn iniciar_captura(sessao: &Arc<Mutex<Sessao>>) -> (Option<CapturaSocketBruto>, Option<String>) {
    let alimentar = sessao.clone();
    match CapturaSocketBruto::iniciar(Arc::new(move |seg, hora| travar(&alimentar).ao_segmento(&seg, hora))) {
        Ok(captura) => (Some(captura), None),
        Err(erro) => (None, Some(format!("Captura parada: {erro}"))),
    }
}

/// Só no build de debug: `--replay captura.pcapng` enche a janela com uma captura, sem o jogo aberto.
fn arquivo_replay() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::args().skip_while(|a| a != "--replay").nth(1).map(PathBuf::from)
}

/// `separar_lutas` (--lutas): as lutas acabam como ao vivo e enchem o histórico; sem ele, a captura
/// inteira é uma luta só e o histórico só ganha luta com o "Zerar". `ate` (--ate S): só os S
/// primeiros segundos da captura, para ver a janela no meio de uma luta.
fn reproduzir(sessao: Arc<Mutex<Sessao>>, arquivo: PathBuf, separar_lutas: bool, ate: Option<f64>) {
    std::thread::spawn(move || {
        let quadros = match nucleo::captura::pcapng::ler(&arquivo) {
            Ok(quadros) => quadros,
            Err(erro) => return eprintln!("{erro}"),
        };
        if !separar_lutas {
            let mut s = travar(&sessao);
            s.medidor.inatividade = i64::MAX;
            s.medidor.fim_pelo_combate = false;
        }
        let fim = quadros.first().zip(ate).map_or(Hora::MAX, |(q, s)| q.hora + (s * TICKS_POR_SEGUNDO as f64) as Hora);
        for q in quadros.iter().take_while(|q| q.hora <= fim) {
            if let Some(seg) = nucleo::captura::segmento::SegmentoTcp::extrair(&q.dados, q.tipo_enlace) {
                travar(&sessao).ao_segmento(&seg, q.hora);
            }
        }
    });
}

fn travar(sessao: &Mutex<Sessao>) -> MutexGuard<'_, Sessao> {
    sessao.lock().unwrap_or_else(PoisonError::into_inner)
}

fn arquivo_memoria() -> PathBuf {
    catalogo::pasta_dados().join("jogadores.json")
}

fn carregar_memoria(medidor: &mut Medidor) {
    let Ok(texto) = std::fs::read_to_string(arquivo_memoria()) else { return };
    // Memória corrompida: começa sem ela.
    if let Ok(salva) = serde_json::from_str::<MemoriaSalva>(texto.trim_start_matches('\u{feff}')) {
        medidor.carregar_memoria(salva.eu, salva.perfis);
    }
}

/// Classe, "Nv x" e "GS y", cada um em pedaços (texto, cor): "?" = ainda não chegou; "~" = da
/// memória (visto antes, pode estar velho). "GS" é o número que o jogo mostra como Power.
fn segmentos_do_perfil(j: &LinhaJogador) -> [Vec<(String, Color32)>; 3] {
    let (normal, apagado) = (branco(0xCC), branco(0x77));
    let valor = |valor: i32, lembrado: bool| match (valor, lembrado) {
        (..=0, _) => ("?".to_string(), apagado),
        (_, true) => (format!("~{valor}"), apagado),
        (_, false) => (valor.to_string(), normal),
    };
    let classe = if j.classe.is_empty() { ("Classe ?".to_string(), apagado) } else { (j.classe.to_string(), normal) };
    [
        vec![classe],
        vec![("Nv ".to_string(), normal), valor(j.nivel, j.nivel_lembrado)],
        vec![("GS ".to_string(), normal), valor(j.poder, j.poder_lembrado)],
    ]
}

/// Classe, Nv e GS separados por " · ", com as cores de cada pedaço.
fn linha_perfil(j: &LinhaJogador) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (i, pedacos) in segmentos_do_perfil(j).into_iter().enumerate() {
        if i > 0 {
            trecho(&mut job, "  ·  ", 10.0, false, branco(0xCC));
        }
        for (texto_pedaco, cor) in pedacos {
            trecho(&mut job, &texto_pedaco, 10.0, false, cor);
        }
    }
    job
}

/// Botão do WPF: texto claro, fundo transparente, realce ao passar o mouse; a aba ativa fica
/// com fundo e em semibold.
fn botao(ui: &mut Ui, rotulo: &str, ativo: bool) -> egui::Response {
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(rotulo.to_string(), fonte(12.0, ativo), branco(0xCC)));
    let (rect, resposta) = ui.allocate_exact_size(galley.size() + vec2(14.0, 2.0), Sense::click());
    let fundo = if resposta.hovered() {
        Some(branco(0x33))
    } else if ativo {
        Some(branco(0x44))
    } else {
        None
    };
    if let Some(cor) = fundo {
        ui.painter().rect_filled(rect, 3, cor);
    }
    ui.painter().galley(rect.min + vec2(7.0, 1.0), galley, branco(0xCC));
    resposta.on_hover_cursor(CursorIcon::PointingHand)
}

/// Os três números de cada linha (como no medidor do TK), alinhados à direita em colunas de largura
/// fixa pelo pior caso de cada uma: a linha não dança quando os números crescem no meio da luta.
/// Número desligado na configuração não ocupa espaço.
struct Tres {
    larguras: [f32; 3],
    visiveis: [bool; 3],
}

const PIOR_CASO: [&str; 3] = ["999,99M", "999,9K/s", "100,0%"];
const ENTRE_NUMEROS: f32 = 12.0;
const MARGEM_DIREITA: f32 = 8.0;

impl Tres {
    fn medir(ui: &Ui, visiveis: [bool; 3]) -> Self {
        let larguras = PIOR_CASO.map(|pior| {
            ui.fonts_mut(|f| f.layout_no_wrap(pior.to_string(), fonte(12.0, true), texto()).size().x.ceil())
        });
        Self { larguras, visiveis }
    }

    fn largura_total(&self) -> f32 {
        let (soma, quantos) = (0..3)
            .filter(|&i| self.visiveis[i])
            .fold((0.0, 0), |(soma, quantos), i| (soma + self.larguras[i], quantos + 1));
        if quantos == 0 { 0.0 } else { soma + ENTRE_NUMEROS * (quantos - 1) as f32 + MARGEM_DIREITA }
    }

    /// Distância da borda direita da linha até a borda direita de cada número.
    fn direitas(&self) -> [f32; 3] {
        let mut direitas = [MARGEM_DIREITA; 3];
        let mut acumulado = MARGEM_DIREITA;
        for i in (0..3).rev() {
            direitas[i] = acumulado;
            if self.visiveis[i] {
                acumulado += self.larguras[i] + ENTRE_NUMEROS;
            }
        }
        direitas
    }

    fn pintar(&self, pintor: &Painter, linha: Rect, celulas: [Arc<Galley>; 3], sombra: bool) {
        for ((celula, direita), visivel) in celulas.into_iter().zip(self.direitas()).zip(self.visiveis) {
            if !visivel {
                continue;
            }
            let posicao = pos2(linha.max.x - direita - celula.size().x, linha.center().y - celula.size().y / 2.0);
            if sombra {
                visual::com_sombra(pintor, posicao, celula);
            } else {
                pintor.galley(posicao, celula, texto());
            }
        }
    }
}

/// Total, por segundo e a parte no total de quem está sendo medido (do jogador no grupo, ou da
/// skill no jogador).
fn textos(total: f64, por_segundo: f64, parte: f64) -> [String; 3] {
    [compacto(total), format!("{}/s", compacto(por_segundo)), p(parte, 1)]
}

/// Total em semibold branco, por segundo em branco, % em amarelo.
fn numeros_da_linha(ui: &Ui, textos: [String; 3], tamanho: f32) -> [Arc<Galley>; 3] {
    let cores = [(true, texto()), (false, branco(0xE6)), (true, visual::AMARELO)];
    let [a, b, c] = textos;
    let montar_um = |t: String, (negrito, cor): (bool, Color32)| {
        montar(ui, LayoutJob::single_section(t, TextFormat::simple(fonte(tamanho, negrito), cor)))
    };
    [montar_um(a, cores[0]), montar_um(b, cores[1]), montar_um(c, cores[2])]
}

/// Os números que saíram da linha na 0.8.0 (eram colunas): vão para o mouse e para a ficha.
fn numeros_extras(aba: Aba, golpes: i32, criticos: i32, aparos: i32, total: f64, maximo: f64) -> String {
    let critico = p(razao(criticos, golpes), 0);
    let maximo = compacto(maximo);
    match aba {
        Aba::Tank => format!("PARRY {}  ·  CRIT {critico}  ·  MAX {maximo}", p(razao(aparos, golpes), 0)),
        Aba::Dps | Aba::Healer => {
            let media = if golpes > 0 { total / f64::from(golpes) } else { 0.0 };
            format!("CRIT {critico}  ·  AVG {}  ·  MAX {maximo}", compacto(media))
        }
    }
}

fn configurar_estilo(ctx: &egui::Context) -> Result<(), String> {
    let pasta = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let mut fontes = egui::FontDefinitions::empty();
    let mut carregar = |nome: &str, arquivo: &str| match std::fs::read(pasta.join(arquivo)) {
        Ok(dados) => {
            fontes.font_data.insert(nome.to_string(), Arc::new(FontData::from_owned(dados)));
            true
        }
        Err(_) => false,
    };
    let regular = carregar("segoe", "segoeui.ttf");
    let semibold = carregar("segoe-semibold", "seguisb.ttf");
    // Segoe UI Symbol cobre ☠ ▸ ▾ ✕ ⚙ ♛, que a Segoe UI não tem.
    let simbolos = carregar("simbolos", "seguisym.ttf");
    if !regular {
        return Err(format!("fonte Segoe UI não encontrada em {}", pasta.display()));
    }

    let mut normal = vec!["segoe".to_string()];
    if simbolos {
        normal.push("simbolos".into());
    }
    let mut negrito = normal.clone();
    if semibold {
        negrito.insert(0, "segoe-semibold".into());
    }
    fontes.families.insert(FontFamily::Proportional, normal.clone());
    fontes.families.insert(FontFamily::Monospace, normal);
    fontes.families.insert(FontFamily::Name(SEMIBOLD.into()), negrito);
    ctx.set_fonts(fontes);

    ctx.set_theme(egui::Theme::Dark);
    // Texto não seleciona: senão o título e o status roubam o arraste da janela.
    ctx.all_styles_mut(|estilo| estilo.interaction.selectable_labels = false);
    // O zoom é o da config (alça e tela de configurações); o Ctrl+= do egui brigaria com ele.
    ctx.options_mut(|opcoes| opcoes.zoom_with_keyboard = false);
    Ok(())
}

/// Arraste próprio em vez do ViewportCommand::StartDrag: o winit só aceita um StartDrag novo depois
/// de receber o WM_EXITSIZEMOVE do anterior (winit 0.30, `handle_os_dragging`), e um arraste que
/// não chega a entrar no laço de mover do Windows deixaria todos os seguintes sem efeito. Aqui a
/// janela segue o cursor a cada quadro, sem estado fora deste arraste.
fn cursor_e_canto(janela: isize) -> Option<([i32; 2], [i32; 4])> {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};

    if janela == 0 {
        return None;
    }
    let mut cursor = POINT { x: 0, y: 0 };
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let ok = unsafe { GetCursorPos(&mut cursor) != 0 && GetWindowRect(janela as _, &mut r) != 0 };
    ok.then_some(([cursor.x, cursor.y], [r.left, r.top, r.right - r.left, r.bottom - r.top]))
}

fn seguir_cursor(janela: isize, inicio_cursor: [i32; 2], inicio_canto: [i32; 4]) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    };

    let mut cursor = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut cursor) } == 0 {
        return;
    }
    let (x, y) = (inicio_canto[0] + cursor.x - inicio_cursor[0], inicio_canto[1] + cursor.y - inicio_cursor[1]);
    // Não sai de cima do jogo.
    let (x, y) = jogo::dentro(x, y, inicio_canto[2], inicio_canto[3]);
    unsafe { SetWindowPos(janela as _, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
}

/// Sem isto, clicar no overlay tira o foco do teclado do jogo. Conferido a cada quadro: o winit
/// recalcula o estilo da janela quando ela aparece e apaga o bit aplicado na criação. Devolve o
/// HWND (0 sem janela), que o recolher usa para achar o monitor.
fn manter_sem_ativar(janela: &impl raw_window_handle::HasWindowHandle) -> isize {
    use raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE};

    let Ok(handle) = janela.window_handle() else { return 0 };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else { return 0 };
    let hwnd = win32.hwnd.get();
    let janela = hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        let estilo = GetWindowLongPtrW(janela, GWL_EXSTYLE);
        if estilo & WS_EX_NOACTIVATE as isize == 0 {
            SetWindowLongPtrW(janela, GWL_EXSTYLE, estilo | WS_EX_NOACTIVATE as isize);
        }
    }
    hwnd
}

/// Caixa de mensagem do Windows (o build de release não tem console).
pub fn avisar(mensagem: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let (texto, titulo) = (crate::bandeja::utf16(mensagem), crate::bandeja::utf16("Axon"));
    unsafe { MessageBoxW(std::ptr::null_mut(), texto.as_ptr(), titulo.as_ptr(), MB_OK | MB_ICONERROR) };
}

/// O PNG original tem 256×256: mipmap para não serrilhar em 18 px.
fn opcoes_de_textura() -> egui::TextureOptions {
    egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR }
}

fn decodificar_png(caminho: &Path) -> Option<egui::ColorImage> {
    decodificar(std::io::BufReader::new(std::fs::File::open(caminho).ok()?))
}

/// PNG em RGBA para textura (os ícones do CDN são RGBA; o resto é convertido).
fn decodificar(leitor: impl std::io::BufRead + std::io::Seek) -> Option<egui::ColorImage> {
    let mut decodificador = png::Decoder::new(leitor);
    decodificador.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut leitor = decodificador.read_info().ok()?;
    let mut buffer = vec![0; leitor.output_buffer_size()?];
    let info = leitor.next_frame(&mut buffer).ok()?;
    let bytes = &buffer[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect(),
        png::ColorType::GrayscaleAlpha => bytes.as_chunks::<2>().0.iter().flat_map(|&[g, a]| [g, g, g, a]).collect(),
        png::ColorType::Grayscale => bytes.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(egui::ColorImage::from_rgba_unmultiplied([info.width as usize, info.height as usize], &rgba))
}

fn cor_da_classe(classe: &str) -> Color32 {
    match classe {
        "Gladiator" => Color32::from_rgb(0xC7, 0x9C, 0x6E),
        "Templar" => Color32::from_rgb(0xF5, 0x8C, 0xBA),
        "Assassin" => Color32::from_rgb(0xFF, 0xF5, 0x69),
        "Ranger" => Color32::from_rgb(0xAB, 0xD4, 0x73),
        "Sorcerer" => Color32::from_rgb(0x69, 0xCC, 0xF0),
        "Elementalist" | "Spirit" => Color32::from_rgb(0x94, 0x82, 0xC9),
        "Cleric" => Color32::from_rgb(0xE8, 0xE8, 0xE8),
        "Chanter" => Color32::from_rgb(0x3E, 0x9B, 0xFF),
        "Brawler" => Color32::from_rgb(0xFF, 0x7D, 0x0A),
        _ => Color32::from_rgb(0xA0, 0xA0, 0xA0),
    }
}

fn texto() -> Color32 {
    Color32::from_rgb(0xE8, 0xE6, 0xE3)
}

fn branco(alfa: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, alfa)
}

fn fonte(tamanho: f32, negrito: bool) -> FontId {
    FontId::new(tamanho, if negrito { FontFamily::Name(SEMIBOLD.into()) } else { FontFamily::Proportional })
}

fn trecho(job: &mut LayoutJob, texto: &str, tamanho: f32, negrito: bool, cor: Color32) {
    job.append(texto, 0.0, TextFormat::simple(fonte(tamanho, negrito), cor));
}

/// Uma linha só, cortada com reticências.
fn uma_linha(largura: f32) -> TextWrapping {
    TextWrapping { max_width: largura, max_rows: 1, break_anywhere: true, overflow_character: Some('…') }
}

fn montar(ui: &Ui, job: LayoutJob) -> Arc<Galley> {
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Uma linha da área de eventos: um evento de horário fixo ou um chefe de campo morto.
struct LinhaEvento {
    nome: String,
    icone: IconeDaLinha,
    estado: eventos::Estado,
    /// O início (ou a hora de renascer): "21:00", ou "qui 21:00" se não for hoje.
    horario: String,
    dica: String,
}

enum IconeDaLinha {
    /// Nome do ícone na CDN do jogo e o recorte dele (cache em disco, os eventos).
    Jogo(String, [f32; 4]),
    /// O mesmo, só na memória (retrato de chefe de campo).
    Web(String, [f32; 4]),
    /// Os resets: a seta de recomeçar e, na linha recolhida, o nome.
    Recomecar,
    /// Chefe sem retrato no questlog: a moldura vazia e, na linha recolhida, o nome.
    Moldura,
}

/// Quanto o skeleton de uma imagem pulsa antes de virar a moldura parada.
const ESPERA_DA_IMAGEM: Duration = Duration::from_secs(10);

/// Cinza do skeleton, pulsando devagar.
fn pulso(ui: &Ui) -> Color32 {
    let t = ui.input(|i| i.time) as f32;
    branco((22.0 + 20.0 * (0.5 + 0.5 * (t * 3.0).sin())) as u8)
}

/// "fecha em 02:41" com o evento aberto; fechado, só o que falta.
fn texto_da_contagem(estado: eventos::Estado) -> String {
    match estado {
        eventos::Estado::Aberto(s) => format!("fecha em {}", eventos::contagem(s)),
        eventos::Estado::Fechado(s) => eventos::contagem(s),
    }
}

/// "fecha em 02:41" em verde com o evento aberto; fechado, só o que falta, em dourado até 10 min.
fn contagem_do_evento(job: &mut LayoutJob, estado: eventos::Estado) {
    match estado {
        eventos::Estado::Aberto(s) => {
            let verde = Color32::from_rgb(0x5B, 0xD1, 0x6B);
            trecho(job, "fecha em ", 11.0, false, verde);
            trecho(job, &eventos::contagem(s), 11.0, true, verde);
        }
        eventos::Estado::Fechado(s) => {
            let cor = if s <= 600 { visual::DOURADO } else { branco(0xCC) };
            trecho(job, &eventos::contagem(s), 11.0, true, cor);
        }
    }
}

fn razao(parte: i32, todo: i32) -> f64 {
    if todo > 0 { f64::from(parte) / f64::from(todo) } else { 0.0 }
}

/// Dica das linhas da aba DPS: os golpes pelas costas (byte de direção do 0x3804), que não têm
/// coluna. Perfeito e duplo ficam de fora até serem conferidos na tela.
fn detalhe_dps(golpes: i32, costas: i32) -> String {
    format!("{golpes} golpes  ·  {} pelas costas", p(razao(costas, golpes), 0))
}

/// "HH:mm" no fuso do Windows (com horário de verão) de uma hora da captura, que vem em UTC.
fn hora_local(hora: Hora) -> String {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};

    // O FILETIME conta desde 1601 e a Hora desde 1970, os dois em ticks de 100 ns.
    let ticks = (hora + 116_444_736_000_000_000) as u64;
    let arquivo = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    let mut utc = unsafe { std::mem::zeroed::<SYSTEMTIME>() };
    let mut local = unsafe { std::mem::zeroed::<SYSTEMTIME>() };
    let ok = unsafe {
        FileTimeToSystemTime(&arquivo, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) != 0
    };
    if ok { format!("{:02}:{:02}", local.wHour, local.wMinute) } else { "--:--".into() }
}

/// "mm:ss" do TimeSpan: o componente de minutos volta a 00 depois de 1 h.
fn minutos_e_segundos(ticks: i64) -> String {
    let s = ticks / TICKS_POR_SEGUNDO;
    format!("{:02}:{:02}", (s / 60) % 60, s % 60)
}

fn compacto(valor: f64) -> String {
    if valor >= 1_000_000.0 {
        format!("{}M", f(valor / 1_000_000.0, 2))
    } else if valor >= 10_000.0 {
        format!("{}K", f(valor / 1_000.0, 1))
    } else {
        n(valor, 0)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_mostra_os_10_primeiros_e_voce_abaixo_deles() {
        // Você entre os 10: só as 10 linhas.
        assert_eq!(linhas_mostradas(40, Some(3), 10), (0..10, None));
        // Você em 37º: as 10 e a sua (índice 36) à parte.
        assert_eq!(linhas_mostradas(40, Some(36), 10), (0..10, Some(36)));
        // Logo depois do corte.
        assert_eq!(linhas_mostradas(11, Some(10), 10), (0..10, Some(10)));
        // Ainda não reconhecido: só as 10.
        assert_eq!(linhas_mostradas(40, None, 10), (0..10, None));
        // Menos de 10 jogadores: todos.
        assert_eq!(linhas_mostradas(4, Some(2), 10), (0..4, None));
    }

    #[test]
    fn rodape_aponta_firewall_ou_trafego_escondido_so_depois_de_esperar() {
        let s = Duration::from_secs;
        // Abrindo: procurando, mesmo sem nada chegar ainda.
        assert_eq!(procurando(0, 500, s(5), None), PROCURANDO);
        // Só sai pacote: o firewall descarta o que chega.
        assert!(procurando(0, 500, s(20), None).contains("firewall"));
        // Pouco tráfego ainda não acusa.
        assert_eq!(procurando(0, 3, s(20), None), PROCURANDO);
        // Chega pacote e o jogo abriu há pouco (login, seleção de personagem).
        assert_eq!(procurando(900, 500, s(60), Some(s(30))), PROCURANDO);
        // Jogo aberto há mais de 2 min sem o servidor aparecer.
        assert!(procurando(900, 500, s(200), Some(s(150))).contains("VPN"));
    }

    fn placar_de_12() -> Placar {
        let jogador = |i: usize| LinhaJogador {
            nome: format!("J{i}"),
            classe: if i == 0 { "Cleric" } else { "" },
            voce: i == 11,
            por_segundo: 100.0 - i as f64,
            porcentagem: (100.0 - i as f64) / 1000.0,
            ..Default::default()
        };
        let mut placar = Placar { duracao: 83 * TICKS_POR_SEGUNDO, ..Default::default() };
        placar.alvo = Some(Alvo { nome: "Kromede".into(), ..Default::default() });
        placar.dano.total = 1_234_567.0;
        placar.dano.jogadores = (0..12).map(jogador).collect();
        placar
    }

    #[test]
    fn resumo_tem_alvo_tempo_total_e_os_10_primeiros_com_voce_abaixo() {
        let placar = placar_de_12();
        let cabeca = format!("Kromede · 01:23 · total {}", compacto(1_234_567.0));
        let linha = resumo(&placar, false);
        assert!(linha.starts_with(&format!("{cabeca} | 1. J0 100/s {}", p(0.1, 1))), "{linha}");
        // Você em 12º, fora dos 10: entra no fim com a posição de verdade, e o 11º fica de fora.
        assert!(linha.ends_with(&format!("10. J9 91/s {} · 12. J11 89/s {}", p(0.091, 1), p(0.089, 1))), "{linha}");
        assert!(!linha.contains("J10"));
        let linhas = resumo(&placar, true);
        assert_eq!(linhas.lines().count(), 12);
        assert_eq!(linhas.lines().next(), Some(cabeca.as_str()));
    }

    #[test]
    fn ocultar_nomes_troca_os_outros_pela_classe_e_guarda_o_seu() {
        let mut placar = placar_de_12();
        ocultar_nomes(&mut placar);
        let nomes: Vec<&str> = placar.dano.jogadores.iter().map(|j| j.nome.as_str()).collect();
        assert_eq!((nomes[0], nomes[1], nomes[11]), ("Cleric", "Jogador", "J11"));
    }
}
