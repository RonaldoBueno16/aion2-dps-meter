//! A janela: título com duração e total, abas DPS | Tank | Healer, uma linha de duas partes por
//! jogador (nome em cima, "Classe · Nv · GS" embaixo; na aba DPS, a tabela à direita), skills ao
//! expandir e status. Mais: configurações, recolher para a borda e a alça que muda o tamanho.

mod configuracoes;
mod recolher;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, FontData, FontFamily, FontId, Galley, Rect, RichText, Sense, Stroke,
    StrokeKind, TextFormat, TextureHandle, Ui, Vec2, ViewportCommand, pos2, vec2,
};
use indexmap::IndexMap;
use nucleo::TICKS_POR_SEGUNDO;
use nucleo::captura::socket_bruto::CapturaSocketBruto;
use nucleo::formato::{f, n, p};
use nucleo::medicao::catalogo::{self, CatalogoSkills};
use nucleo::medicao::dados_jogo;
use nucleo::medicao::medidor::{LinhaJogador, LinhaSkill, Medidor, PerfilJogador, Placar, Tabela};
use nucleo::medicao::sessao::Sessao;
use recolher::{Dobra, Lado};
use serde::{Deserialize, Serialize};

use crate::bandeja::Bandeja;
use crate::config::{self, Config};
use crate::jogo;

/// 470 e não os 390 do WPF: a tabela da aba DPS precisa de ~280 px ao lado do nome.
pub const LARGURA: f32 = 470.0;
const INTERVALO: Duration = Duration::from_millis(500);
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
    /// Cabeçalho e pior caso (para a largura) de cada coluna da tabela da aba.
    fn colunas(self) -> &'static [(&'static str, &'static str); 5] {
        match self {
            Aba::Dps => &COLUNAS_DPS,
            Aba::Tank => &COLUNAS_TANK,
            Aba::Healer => &COLUNAS_HEALER,
        }
    }
}

const COLUNAS_DPS: [(&str, &str); 5] =
    [("DPS", "999,9K"), ("Damage(%)", "99,99M (100%)"), ("CRIT", "100%"), ("AVG", "999,9K"), ("MAX", "999,9K")];
/// PARRY no lugar do AVG: o golpe médio recebido depende de qual monstro bateu em quem e não diz
/// nada do tank; a fração de golpes aparados é o único sinal de mitigação que o pacote traz.
const COLUNAS_TANK: [(&str, &str); 5] =
    [("DTPS", "999,9K"), ("Taken(%)", "99,99M (100%)"), ("PARRY", "100%"), ("CRIT", "100%"), ("MAX", "999,9K")];
const COLUNAS_HEALER: [(&str, &str); 5] =
    [("HPS", "999,9K"), ("Heal(%)", "99,99M (100%)"), ("CRIT", "100%"), ("AVG", "999,9K"), ("MAX", "999,9K")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tela {
    Medidor,
    Configuracoes,
}

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
    catalogo: &'static CatalogoSkills,
    aba: Aba,
    expandidos: HashSet<(Aba, u32)>,
    // Placar lido a cada 500 ms: com o mouse em cima o egui redesenha a ~60 fps, e cada leitura
    // segura o Mutex que as threads de captura usam.
    placar: Placar,
    fluxo: Option<String>,
    lido_em: Instant,
    salvo_em: Instant,
    icones: HashMap<PathBuf, Option<TextureHandle>>,
    /// Altura do conteúdo (em pontos) e escala (pixels por ponto) com que o tamanho da janela foi
    /// pedido por último. A escala e não o zoom da config: ela muda também com o DPI do monitor.
    altura: f32,
    escala_aplicada: f32,
    /// --replay (só debug): a captura antiga não grava memória nem config, e a luta nunca zera.
    replay: bool,
    /// Só no debug (--expandir): abre todas as linhas, para conferir o desenho sem clicar.
    expandir_tudo: bool,
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
        cc.egui_ctx.set_zoom_factor(config.zoom);

        let sessao = Arc::new(Mutex::new(Sessao::default()));
        carregar_memoria(&mut travar(&sessao).medidor);
        let replay = arquivo_replay();
        // Ao vivo, o overlay só aparece por cima do jogo; o replay de debug roda sem o jogo.
        jogo::seguir(replay.is_none());
        let (captura, erro_captura) = match &replay {
            Some(arquivo) => {
                reproduzir(sessao.clone(), arquivo.clone());
                (None, None)
            }
            None => {
                travar(&sessao).medidor.inatividade = i64::from(config.inatividade) * TICKS_POR_SEGUNDO;
                iniciar_captura(&sessao)
            }
        };

        let mut overlay = Self {
            sessao,
            captura,
            erro_captura,
            catalogo,
            aba: if tem("--tank") { Aba::Tank } else { Aba::Dps },
            expandidos: HashSet::new(),
            placar: Placar::default(),
            fluxo: None,
            lido_em: Instant::now(),
            salvo_em: Instant::now(),
            icones: HashMap::new(),
            altura: 0.0,
            escala_aplicada: 0.0,
            replay: replay.is_some(),
            expandir_tudo: tem("--expandir"),
            config,
            tela: if tem("--config") { Tela::Configuracoes } else { Tela::Medidor },
            dobra: Dobra::Aberto,
            janela: manter_sem_ativar(cc),
            arraste: None,
            amostra: None,
            bandeja: None,
            teste_dobra: (tem("--recolher") || tem("--recolher-e-voltar"))
                .then(|| (Instant::now(), tem("--recolher-e-voltar"), false)),
        };
        overlay.bandeja = Bandeja::iniciar(overlay.janela);
        overlay.ler_placar();
        Ok(overlay)
    }

    fn ler_placar(&mut self) {
        let sessao = travar(&self.sessao);
        self.placar = sessao.medidor.obter_placar();
        self.fluxo = sessao.fluxo.clone();
        let memoria = (self.tela == Tela::Configuracoes).then(|| sessao.medidor.exportar_memoria());
        drop(sessao);
        self.lido_em = Instant::now();
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

    fn zerar(&mut self) {
        travar(&self.sessao).medidor.reiniciar();
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

    /// As colunas ligadas na configuração para a aba atual.
    fn colunas_ligadas(&mut self) -> &mut [bool; 5] {
        match self.aba {
            Aba::Dps => &mut self.config.colunas,
            Aba::Tank => &mut self.config.colunas_tank,
            Aba::Healer => &mut self.config.colunas_healer,
        }
    }

    fn conteudo(&mut self, ui: &mut Ui) {
        if self.tela == Tela::Configuracoes {
            self.tela_configuracoes(ui);
            return;
        }
        let tabela = self.tabela();
        self.cabecalho(ui, &tabela);
        ui.add_space(6.0);
        self.abas(ui);
        ui.add_space(6.0);
        self.linhas(ui, &tabela);
        ui.add_space(6.0);
        self.status(ui);
    }

    fn cabecalho(&mut self, ui: &mut Ui, tabela: &Tabela) {
        let titulo = if tabela.jogadores.is_empty() {
            "Axon".to_string()
        } else {
            format!("Axon  ·  {}  ·  {}", minutos_e_segundos(self.placar.duracao), compacto(tabela.total))
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(titulo).font(fonte(12.0, true)).color(texto()));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "✕", false).on_hover_text("Fechar o medidor").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                // A seta aponta para a borda para onde a janela vai.
                let seta = match recolher::lado_mais_perto(self.janela) {
                    Lado::Esquerda => "‹",
                    Lado::Direita => "›",
                };
                if botao(ui, seta, false).on_hover_text("Recolher para a borda da tela").clicked() {
                    self.dobra.recolher(self.janela);
                }
                if botao(ui, "⚙", false).on_hover_text("Configurações").clicked() {
                    self.tela = Tela::Configuracoes;
                    self.ler_placar();
                }
                if botao(ui, "Zerar", false).on_hover_text("Começa uma luta nova").clicked() {
                    self.zerar();
                }
            });
        });
    }

    fn abas(&mut self, ui: &mut Ui) {
        let abas = [
            (Aba::Dps, "DPS", "Dano causado em monstros"),
            (
                Aba::Tank,
                "Tank",
                "Dano recebido de monstros. PARRY: fração dos golpes que o jogador aparou. \"aggro N\": N monstros \
                 têm este jogador como último alvo (8 s); a ameaça em número fica no servidor e não chega ao jogo.",
            ),
            (
                Aba::Healer,
                "Healer",
                "Cura feita em jogadores. O pacote não separa a sobrecura, então o HPS pode incluir cura que \
                 passou do HP cheio. Leitura ainda não conferida numa luta com curandeiro.",
            ),
        ];
        ui.horizontal(|ui| {
            for (aba, nome, dica) in abas {
                if botao(ui, nome, self.aba == aba).on_hover_text(dica).clicked() {
                    self.aba = aba;
                    self.ler_placar();
                }
            }
        });
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
            ui.add(egui::Label::new(RichText::new(vazio).font(fonte(12.0, false)).color(texto().gamma_multiply(0.6))).wrap());
            return;
        }

        // A tabela da aba (DPS, Tank ou Healer), com as colunas ligadas na configuração.
        let ligadas = *self.colunas_ligadas();
        let colunas = Colunas::medir(ui, self.aba.colunas(), ligadas);
        if colunas.alguma() {
            colunas.cabecalho(ui);
        }

        let maior = tabela.jogadores[0].total;
        for j in &tabela.jogadores {
            self.linha_jogador(ui, j, maior, &colunas);
            if self.expandidos.contains(&(self.aba, j.id)) {
                // Todas as skills que aconteceram, com as mesmas colunas do jogador.
                for s in &j.skills {
                    self.linha_skill(ui, s, &colunas);
                }
            }
        }
    }

    /// Dica da linha na aba Tank: as contagens que não cabem na tabela.
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

    fn linha_jogador(&mut self, ui: &mut Ui, j: &LinhaJogador, maior: f64, colunas: &Colunas) {
        let largura = ui.available_width();
        let expandido = self.expandidos.contains(&(self.aba, j.id));
        let tank = self.aba == Aba::Tank;

        let celulas: Vec<Arc<Galley>> = celulas(self.aba, Numeros::from(j))
            .into_iter()
            .enumerate()
            .map(|(i, t)| montar(ui, LayoutJob::single_section(t, TextFormat::simple(fonte(12.0, i == 0), texto()))))
            .collect();
        let altura_direita = celulas
            .iter()
            .zip(colunas.visiveis)
            .filter(|(_, visivel)| *visivel)
            .fold(0.0_f32, |maior, (g, _)| maior.max(g.size().y));
        let max_esquerda = (largura - colunas.largura_total() - 6.0 - 8.0).max(40.0);

        // Selos da aba Tank, medidos à parte: o corte com "…" encurta o nome e eles ficam inteiros.
        // Aggro: quantos monstros têm este jogador como último alvo; a ameaça em número fica no servidor.
        let mut selos = LayoutJob::default();
        if tank && j.segurando_aggro > 0 {
            trecho(&mut selos, &format!("  aggro {}", j.segurando_aggro), 10.0, true, Color32::from_rgb(0xFF, 0xB5, 0x47));
        }
        if tank && j.mortes > 0 {
            trecho(&mut selos, &format!("  ☠{}", j.mortes), 10.0, j.voce, Color32::from_rgb(0xFF, 0x6B, 0x6B));
        }
        let selos = (!selos.sections.is_empty()).then(|| montar(ui, selos));
        let largura_selos = selos.as_ref().map_or(0.0, |g| g.size().x);

        let mut nome = LayoutJob::default();
        let seta = if expandido { "▾" } else { "▸" };
        let voce = if j.voce { " (você)" } else { "" };
        trecho(&mut nome, &format!("{seta} {}{voce}", j.nome), 12.0, j.voce, texto());
        nome.wrap = uma_linha((max_esquerda - largura_selos).max(40.0));
        let nome = montar(ui, nome);

        // Sem nenhum dado ligado na configuração, a linha do jogador fica só com o nome.
        let perfil = linha_perfil(j, self.config.perfil()).map(|mut job| {
            job.wrap = uma_linha(max_esquerda - 12.0);
            montar(ui, job)
        });
        let altura_perfil = perfil.as_ref().map_or(0.0, |g| g.size().y);
        let altura_esquerda = nome.size().y + altura_perfil + 4.0;

        let altura = altura_esquerda.max(altura_direita);
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, altura + 2.0), Sense::click());
        let linha = rect.shrink2(vec2(0.0, 1.0));
        let pintor = ui.painter();

        let fracao = if maior > 0.0 { (j.total / maior) as f32 } else { 0.0 };
        let barra = Rect::from_min_size(linha.min, vec2(linha.width() * fracao.clamp(0.0, 1.0), linha.height()));
        let cor = cor_da_classe(j.classe);
        pintor.rect_filled(barra, 3, Color32::from_rgba_unmultiplied(cor.r(), cor.g(), cor.b(), 0x66));

        let altura_nome = nome.size().y;
        let topo = linha.min.y + (linha.height() - altura_esquerda) / 2.0;
        let largura_nome = nome.size().x;
        pintor.galley(pos2(linha.min.x + 6.0, topo + 2.0), nome, texto());
        if let Some(selos) = selos {
            let y = topo + 2.0 + (altura_nome - selos.size().y) / 2.0 + 1.0;
            pintor.galley(pos2(linha.min.x + 6.0 + largura_nome, y), selos, texto());
        }
        if let Some(perfil) = perfil {
            pintor.galley(pos2(linha.min.x + 18.0, topo + 2.0 + altura_nome), perfil, texto());
        }
        colunas.pintar(pintor, linha, celulas);

        let mut resposta = resposta.on_hover_cursor(CursorIcon::PointingHand);
        if tank {
            resposta = resposta.on_hover_text(Self::detalhe_tank(j));
        }
        if resposta.clicked() && !self.expandidos.remove(&(self.aba, j.id)) {
            self.expandidos.insert((self.aba, j.id));
        }
    }

    /// Skill expandida: ícone e nome à esquerda, as mesmas colunas do jogador à direita.
    fn linha_skill(&mut self, ui: &mut Ui, s: &LinhaSkill, colunas: &Colunas) {
        let largura = ui.available_width();
        let textos = celulas(self.aba, Numeros::from(s));
        let formato = TextFormat::simple(fonte(11.0, false), texto().gamma_multiply(0.85));
        let celulas: Vec<Arc<Galley>> =
            textos.into_iter().map(|t| montar(ui, LayoutJob::single_section(t, formato.clone()))).collect();

        // Moldura fixa: a linha não pula quando o ícone termina de baixar.
        let inicio_nome = 18.0 + 18.0 + 6.0;
        let mut nome = LayoutJob::single_section(s.nome.clone(), TextFormat::simple(fonte(11.0, false), texto().gamma_multiply(0.9)));
        nome.wrap = uma_linha((largura - inicio_nome - 8.0 - colunas.largura_total()).max(40.0));
        let nome = montar(ui, nome);

        let altura = celulas.iter().fold(nome.size().y.max(18.0), |maior, g| maior.max(g.size().y));
        let (rect, _) = ui.allocate_exact_size(vec2(largura, altura + 2.0), Sense::hover());
        let linha = Rect::from_min_size(rect.min, vec2(largura, altura));

        self.icone(ui, s, linha);
        ui.painter().galley(pos2(linha.min.x + inicio_nome, linha.center().y - nome.size().y / 2.0), nome, texto());
        colunas.pintar(ui.painter(), linha, celulas);
    }

    /// Moldura fixa de 18 px com o ícone da skill (a linha não pula quando o ícone termina de baixar).
    fn icone(&mut self, ui: &Ui, s: &LinhaSkill, linha: Rect) {
        let moldura = Rect::from_min_size(pos2(linha.min.x + 18.0, linha.min.y), vec2(18.0, 18.0));
        ui.painter().rect_filled(moldura, 3, branco(0x22));
        if let Some(icone) = s.icone.as_deref().and_then(|c| self.textura(ui.ctx(), c)) {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            ui.painter().image(icone.id(), moldura, uv, Color32::WHITE);
        }
    }

    fn status(&mut self, ui: &mut Ui) {
        // O endereço do servidor não aparece: o rodapé só avisa o que ainda falta ou o que parou.
        let mut partes: Vec<String> = Vec::new();
        match &self.erro_captura {
            Some(erro) => partes.push(erro.clone()),
            None => {
                if self.fluxo.is_none() {
                    partes.push("Procurando o servidor do jogo...".into());
                }
                if self.catalogo.quantidade() == 0 {
                    partes.push("baixando nomes das skills...".into());
                }
            }
        }
        // Versão do build, para os amigos dizerem qual usam.
        partes.push(concat!("v", env!("CARGO_PKG_VERSION")).into());
        let status = partes.join("  ·  ");
        ui.add(egui::Label::new(RichText::new(status).font(fonte(10.0, false)).color(branco(0x99))).wrap());
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
        let alfa = if resposta.hovered() { 0xF2 } else { 0xD9 };
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
        // O PNG original tem 256×256: mipmap para não serrilhar em 18 px.
        let opcoes = egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
        let textura = decodificar_png(caminho).map(|imagem| ctx.load_texture(caminho.to_string_lossy(), imagem, opcoes));
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
        if (ctx.zoom_factor() - self.config.zoom).abs() > 0.001 {
            ctx.set_zoom_factor(self.config.zoom);
        }
        if self.lido_em.elapsed() >= INTERVALO {
            self.ler_placar();
        }
        if self.salvo_em.elapsed() >= SALVAR_A_CADA {
            self.salvar_memoria();
        }
        self.testar_dobra(ctx);

        let aba = tamanho_da_aba(ctx);
        let animando = self.dobra.quadro(ctx, aba, vec2(LARGURA, self.altura));
        if animando {
            // O deslize precisa de um quadro atrás do outro (a janela não recebe input que os peça).
            ctx.request_repaint();
        } else {
            // Sem isso o egui só redesenha com input, e o placar congelaria.
            ctx.request_repaint_after(INTERVALO);
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

            let quadro = egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(0x10, 0x14, 0x18, 0xD9))
                .stroke(Stroke::new(1.0_f32, branco(0x33)))
                .corner_radius(6)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.set_width(ui.available_width());
                    self.conteudo(ui);
                });

            // Altura pelo conteúdo (o SizeToContent do WPF): só manda o comando quando a altura ou a
            // escala mudam. Recolhendo ou voltando, quem manda no tamanho é a animação.
            let altura = quadro.response.rect.height().ceil();
            let escala = ctx.pixels_per_point();
            let escala_mudou = (self.escala_aplicada - escala).abs() > 0.001;
            if self.dobra.aberto() && ((altura - self.altura).abs() >= 1.0 || escala_mudou) {
                self.altura = altura;
                self.escala_aplicada = escala;
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(LARGURA, altura)));
            }
        });
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
        id: 0,
        nome,
        classe: "",
        nivel: perfil.nivel,
        nivel_lembrado: perfil.nivel > 0,
        poder: perfil.poder,
        poder_lembrado: perfil.poder > 0,
        voce: true,
        total: 0.0,
        por_segundo: 0.0,
        porcentagem: 0.0,
        golpes: 0,
        criticos: 0,
        aparos: 0,
        mortes: 0,
        segurando_aggro: 0,
        maximo: 0.0,
        skills: Vec::new(),
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

fn reproduzir(sessao: Arc<Mutex<Sessao>>, arquivo: PathBuf) {
    std::thread::spawn(move || {
        let quadros = match nucleo::captura::pcapng::ler(&arquivo) {
            Ok(quadros) => quadros,
            Err(erro) => return eprintln!("{erro}"),
        };
        travar(&sessao).medidor.inatividade = i64::MAX;
        for q in &quadros {
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

/// Os dados ligados na configuração, separados por " · "; None com os três desligados.
fn linha_perfil(j: &LinhaJogador, visiveis: [bool; 3]) -> Option<LayoutJob> {
    let mut job = LayoutJob::default();
    let mut vazio = true;
    for (pedacos, visivel) in segmentos_do_perfil(j).into_iter().zip(visiveis) {
        if !visivel {
            continue;
        }
        if !vazio {
            trecho(&mut job, "  ·  ", 10.0, false, branco(0xCC));
        }
        vazio = false;
        for (texto_pedaco, cor) in pedacos {
            trecho(&mut job, &texto_pedaco, 10.0, false, cor);
        }
    }
    (!vazio).then_some(job)
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

/// Colunas da tabela de uma aba. Largura fixa pelo pior caso de cada coluna, para a tabela não
/// dançar quando os números crescem no meio da luta. Coluna desligada não ocupa espaço.
struct Colunas {
    titulos: [&'static str; 5],
    larguras: [f32; 5],
    visiveis: [bool; 5],
}

const ESPACO_ENTRE_COLUNAS: f32 = 10.0;
const MARGEM_DIREITA: f32 = 6.0;

impl Colunas {
    fn medir(ui: &Ui, definicao: &[(&'static str, &'static str); 5], visiveis: [bool; 5]) -> Self {
        let largura = |amostra: &str, tamanho: f32, negrito: bool| {
            ui.fonts_mut(|f| f.layout_no_wrap(amostra.to_string(), fonte(tamanho, negrito), texto()).size().x)
        };
        let mut larguras = [0.0; 5];
        for (i, (titulo, pior)) in definicao.iter().enumerate() {
            larguras[i] = largura(titulo, 10.0, false).max(largura(pior, 12.0, i == 0)).ceil();
        }
        Self { titulos: definicao.map(|(titulo, _)| titulo), larguras, visiveis }
    }

    fn alguma(&self) -> bool {
        self.visiveis.contains(&true)
    }

    fn largura_total(&self) -> f32 {
        let (soma, quantas) = (0..5)
            .filter(|&i| self.visiveis[i])
            .fold((0.0, 0), |(soma, quantas), i| (soma + self.larguras[i], quantas + 1));
        if quantas == 0 { 0.0 } else { soma + ESPACO_ENTRE_COLUNAS * (quantas - 1) as f32 + MARGEM_DIREITA }
    }

    /// Distância da borda direita da linha até a borda direita de cada coluna (as desligadas não
    /// empurram as outras).
    fn direitas(&self) -> [f32; 5] {
        let mut direitas = [MARGEM_DIREITA; 5];
        let mut acumulado = MARGEM_DIREITA;
        for i in (0..5).rev() {
            direitas[i] = acumulado;
            if self.visiveis[i] {
                acumulado += self.larguras[i] + ESPACO_ENTRE_COLUNAS;
            }
        }
        direitas
    }

    /// Títulos das colunas, alinhados à direita como os números.
    fn cabecalho(&self, ui: &mut Ui) {
        let titulos: Vec<Arc<Galley>> = self
            .titulos
            .iter()
            .map(|titulo| ui.fonts_mut(|f| f.layout_no_wrap(titulo.to_string(), fonte(10.0, false), branco(0x99))))
            .collect();
        let altura = titulos.iter().fold(0.0_f32, |maior, g| maior.max(g.size().y));
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), altura + 2.0), Sense::hover());
        self.pintar(ui.painter(), Rect::from_min_size(rect.min, vec2(rect.width(), altura)), titulos);
    }

    /// Cada célula ligada alinhada à direita na sua coluna e centrada na altura da linha.
    fn pintar(&self, pintor: &egui::Painter, linha: Rect, celulas: Vec<Arc<Galley>>) {
        for ((celula, direita), visivel) in celulas.into_iter().zip(self.direitas()).zip(self.visiveis) {
            if !visivel {
                continue;
            }
            let posicao = pos2(linha.max.x - direita - celula.size().x, linha.center().y - celula.size().y / 2.0);
            pintor.galley(posicao, celula, texto());
        }
    }
}

/// Os números de uma linha da tabela, de um jogador ou de uma skill dele.
struct Numeros {
    por_segundo: f64,
    total: f64,
    porcentagem: f64,
    golpes: i32,
    criticos: i32,
    aparos: i32,
    maximo: f64,
}

impl From<&LinhaJogador> for Numeros {
    fn from(j: &LinhaJogador) -> Self {
        let LinhaJogador { por_segundo, total, porcentagem, golpes, criticos, aparos, maximo, .. } = *j;
        Self { por_segundo, total, porcentagem, golpes, criticos, aparos, maximo }
    }
}

impl From<&LinhaSkill> for Numeros {
    fn from(s: &LinhaSkill) -> Self {
        let LinhaSkill { por_segundo, total, porcentagem, golpes, criticos, aparos, maximo, .. } = *s;
        Self { por_segundo, total, porcentagem, golpes, criticos, aparos, maximo }
    }
}

/// As células na ordem das colunas da aba (Aba::colunas). O % é a parte no total de quem está
/// sendo medido (do jogador no grupo, ou da skill no jogador).
fn celulas(aba: Aba, n: Numeros) -> [String; 5] {
    let por_segundo = compacto(n.por_segundo);
    let parte = format!("{} ({})", compacto(n.total), p(n.porcentagem, 0));
    let critico = p(razao(n.criticos, n.golpes), 0);
    let maximo = compacto(n.maximo);
    match aba {
        Aba::Tank => [por_segundo, parte, p(razao(n.aparos, n.golpes), 0), critico, maximo],
        Aba::Dps | Aba::Healer => {
            let media = if n.golpes > 0 { n.total / f64::from(n.golpes) } else { 0.0 };
            [por_segundo, parte, critico, compacto(media), maximo]
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
    // Segoe UI Symbol cobre ☠ ▸ ▾ ✕ ⚙, que a Segoe UI não tem.
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
    let utf16 = |texto: &str| texto.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (texto, titulo) = (utf16(mensagem), utf16("Axon"));
    unsafe { MessageBoxW(std::ptr::null_mut(), texto.as_ptr(), titulo.as_ptr(), MB_OK | MB_ICONERROR) };
}

/// PNG em RGBA para textura (os ícones do CDN são RGBA; o resto é convertido).
fn decodificar_png(caminho: &Path) -> Option<egui::ColorImage> {
    let arquivo = std::io::BufReader::new(std::fs::File::open(caminho).ok()?);
    let mut decodificador = png::Decoder::new(arquivo);
    decodificador.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut leitor = decodificador.read_info().ok()?;
    let mut buffer = vec![0; leitor.output_buffer_size()?];
    let info = leitor.next_frame(&mut buffer).ok()?;
    let bytes = &buffer[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => bytes.chunks_exact(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
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

fn razao(parte: i32, todo: i32) -> f64 {
    if todo > 0 { f64::from(parte) / f64::from(todo) } else { 0.0 }
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
