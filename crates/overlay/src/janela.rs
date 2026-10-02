//! A janela: título com duração e total, abas DPS | Tank | Healer, uma linha de duas partes por
//! jogador (nome e números em cima, "Classe · Nv · Power" embaixo), skills ao expandir e status.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Align, Color32, CursorIcon, FontData, FontFamily, FontId, Galley, Rect, RichText, Sense, Stroke, TextFormat,
    TextureHandle, Ui, Vec2, ViewportCommand, pos2, vec2,
};
use indexmap::IndexMap;
use nucleo::TICKS_POR_SEGUNDO;
use nucleo::captura::socket_bruto::CapturaSocketBruto;
use nucleo::formato::{f, n, p};
use nucleo::medicao::catalogo::{self, CatalogoSkills};
use nucleo::medicao::dados_jogo;
use nucleo::medicao::medidor::{LinhaJogador, LinhaSkill, Medidor, PerfilJogador, Placar, Tabela};
use nucleo::medicao::sessao::Sessao;
use serde::{Deserialize, Serialize};

pub const LARGURA: f32 = 390.0;
const INTERVALO: Duration = Duration::from_millis(500);
const SALVAR_A_CADA: Duration = Duration::from_secs(30);
const SEMIBOLD: &str = "semibold";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Aba {
    Dps,
    Tank,
    Healer,
}

impl Aba {
    fn por_segundo(self) -> &'static str {
        match self {
            Aba::Dps => "DPS",
            Aba::Tank => "DTPS",
            Aba::Healer => "HPS",
        }
    }
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
    altura: f32,
    /// Falso no --replay: captura antiga não atualiza a memória dos jogadores.
    salvar: bool,
    /// Só no debug (--expandir): abre todas as linhas, para conferir o desenho sem clicar.
    expandir_tudo: bool,
}

impl Overlay {
    pub fn novo(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        configurar_estilo(&cc.egui_ctx)?;
        dados_jogo::carregar_nomes_padrao();
        let catalogo = dados_jogo::definir_catalogo(CatalogoSkills::novo(None));

        let sessao = Arc::new(Mutex::new(Sessao::default()));
        carregar_memoria(&mut travar(&sessao).medidor);
        let replay = arquivo_replay();
        let opcoes_debug: Vec<String> = if cfg!(debug_assertions) { std::env::args().collect() } else { Vec::new() };
        let (captura, erro_captura) = match &replay {
            Some(arquivo) => {
                reproduzir(sessao.clone(), arquivo.clone());
                (None, None)
            }
            None => iniciar_captura(&sessao),
        };

        let mut overlay = Self {
            sessao,
            captura,
            erro_captura,
            catalogo,
            aba: if opcoes_debug.iter().any(|a| a == "--tank") { Aba::Tank } else { Aba::Dps },
            expandidos: HashSet::new(),
            placar: Placar::default(),
            fluxo: None,
            lido_em: Instant::now(),
            salvo_em: Instant::now(),
            icones: HashMap::new(),
            altura: 0.0,
            salvar: replay.is_none(),
            expandir_tudo: opcoes_debug.iter().any(|a| a == "--expandir"),
        };
        overlay.ler_placar();
        Ok(overlay)
    }

    fn ler_placar(&mut self) {
        let sessao = travar(&self.sessao);
        self.placar = sessao.medidor.obter_placar();
        self.fluxo = sessao.fluxo.clone();
        drop(sessao);
        self.lido_em = Instant::now();
        if self.expandir_tudo {
            for (aba, tabela) in [(Aba::Dps, &self.placar.dano), (Aba::Tank, &self.placar.dano_recebido), (Aba::Healer, &self.placar.cura)] {
                self.expandidos.extend(tabela.jogadores.iter().map(|j| (aba, j.id)));
            }
        }
    }

    fn salvar_memoria(&mut self) {
        self.salvo_em = Instant::now();
        if !self.salvar {
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

    fn zerar(&mut self) {
        travar(&self.sessao).medidor.reiniciar();
        self.expandidos.clear();
        self.ler_placar();
    }

    fn conteudo(&mut self, ui: &mut Ui) {
        let tabela = match self.aba {
            Aba::Dps => &self.placar.dano,
            Aba::Tank => &self.placar.dano_recebido,
            Aba::Healer => &self.placar.cura,
        }
        .clone();

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
            "AION2 Medidor".to_string()
        } else {
            format!("AION2  ·  {}  ·  {}", minutos_e_segundos(self.placar.duracao), compacto(tabela.total))
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(titulo).font(fonte(12.0, true)).color(texto()));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if botao(ui, "✕", false).on_hover_text("Fechar o medidor").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
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
                "Dano recebido de monstros. \"aggro N\": N monstros têm este jogador como último alvo (8 s). \
                 O número de ameaça fica no servidor e não chega ao jogo.",
            ),
            (Aba::Healer, "Healer", "Cura feita em jogadores. Leitura ainda não conferida numa luta com curandeiro."),
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
            let vazio = match self.aba {
                Aba::Tank => "Nenhum golpe de monstro em jogador ainda.",
                Aba::Healer => "Nenhuma cura vista ainda.",
                Aba::Dps => "Sem dano ainda.",
            };
            ui.label(RichText::new(vazio).font(fonte(12.0, false)).color(texto().gamma_multiply(0.6)));
            return;
        }

        let maior = tabela.jogadores[0].total;
        for j in &tabela.jogadores {
            self.linha_jogador(ui, j, maior);
            if !self.expandidos.contains(&(self.aba, j.id)) {
                continue;
            }

            let mut detalhe =
                LayoutJob::single_section(self.detalhe(j), TextFormat::simple(fonte(10.0, false), texto().gamma_multiply(0.7)));
            detalhe.wrap.max_width = ui.available_width() - 18.0;
            let detalhe = montar(ui, detalhe);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), detalhe.size().y + 2.0), Sense::hover());
            ui.painter().galley(rect.min + vec2(18.0, 0.0), detalhe, texto());
            for s in j.skills.iter().take(8) {
                self.linha_skill(ui, s);
            }
        }
    }

    fn detalhe(&self, j: &LinhaJogador) -> String {
        if self.aba != Aba::Tank {
            let golpes = if self.aba == Aba::Healer { "curas" } else { "golpes" };
            return format!("{} {golpes}  ·  crítico {}", j.golpes, p(razao(j.criticos, j.golpes), 0));
        }
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

    fn linha_jogador(&mut self, ui: &mut Ui, j: &LinhaJogador, maior: f64) {
        let largura = ui.available_width();
        let expandido = self.expandidos.contains(&(self.aba, j.id));
        let tank = self.aba == Aba::Tank;

        let mut direita = LayoutJob::default();
        trecho(&mut direita, &format!("{} {}", compacto(j.por_segundo), self.aba.por_segundo()), 12.0, true, texto());
        trecho(&mut direita, &format!("  {}  {}", compacto(j.total), p(j.porcentagem, 0)), 12.0, false, texto());
        let direita = montar(ui, direita);
        let max_esquerda = (largura - direita.size().x - 6.0 - 6.0 - 8.0).max(40.0);

        let mut nome = LayoutJob::default();
        let seta = if expandido { "▾" } else { "▸" };
        let voce = if j.voce { " (você)" } else { "" };
        trecho(&mut nome, &format!("{seta} {}{voce}", j.nome), 12.0, j.voce, texto());
        // Aggro: quantos monstros têm este jogador como último alvo. A ameaça em número fica no servidor.
        if tank && j.segurando_aggro > 0 {
            trecho(&mut nome, &format!("  aggro {}", j.segurando_aggro), 10.0, true, Color32::from_rgb(0xFF, 0xB5, 0x47));
        }
        if tank && j.mortes > 0 {
            trecho(&mut nome, &format!("  ☠{}", j.mortes), 10.0, j.voce, Color32::from_rgb(0xFF, 0x6B, 0x6B));
        }
        nome.wrap = uma_linha(max_esquerda);
        let nome = montar(ui, nome);

        let mut perfil = linha_perfil(j);
        perfil.wrap = uma_linha(max_esquerda - 12.0);
        let perfil = montar(ui, perfil);

        let altura = (nome.size().y + perfil.size().y + 4.0).max(direita.size().y);
        let (rect, resposta) = ui.allocate_exact_size(vec2(largura, altura + 2.0), Sense::click());
        let linha = rect.shrink2(vec2(0.0, 1.0));
        let pintor = ui.painter();

        let fracao = if maior > 0.0 { (j.total / maior) as f32 } else { 0.0 };
        let barra = Rect::from_min_size(linha.min, vec2(linha.width() * fracao.clamp(0.0, 1.0), linha.height()));
        let cor = cor_da_classe(j.classe);
        pintor.rect_filled(barra, 3, Color32::from_rgba_unmultiplied(cor.r(), cor.g(), cor.b(), 0x66));

        let altura_nome = nome.size().y;
        let topo = linha.min.y + (linha.height() - (altura_nome + perfil.size().y + 4.0)) / 2.0;
        pintor.galley(pos2(linha.min.x + 6.0, topo + 2.0), nome, texto());
        pintor.galley(pos2(linha.min.x + 18.0, topo + 2.0 + altura_nome), perfil, texto());
        let y_direita = linha.center().y - direita.size().y / 2.0;
        pintor.galley(pos2(linha.max.x - 6.0 - direita.size().x, y_direita), direita, texto());

        if resposta.on_hover_cursor(CursorIcon::PointingHand).clicked() && !self.expandidos.remove(&(self.aba, j.id)) {
            self.expandidos.insert((self.aba, j.id));
        }
    }

    fn linha_skill(&mut self, ui: &mut Ui, s: &LinhaSkill) {
        let largura = ui.available_width();
        let numeros = format!("{}  {}  {}x", compacto(s.total), p(s.porcentagem, 0), s.golpes);
        let numeros = montar(ui, LayoutJob::single_section(numeros, TextFormat::simple(fonte(11.0, false), texto().gamma_multiply(0.85))));

        // Moldura fixa: a linha não pula quando o ícone termina de baixar.
        let inicio_nome = 18.0 + 18.0 + 6.0;
        let mut nome = LayoutJob::single_section(s.nome.clone(), TextFormat::simple(fonte(11.0, false), texto().gamma_multiply(0.9)));
        nome.wrap = uma_linha((largura - inicio_nome - 8.0 - numeros.size().x - 6.0).max(40.0));
        let nome = montar(ui, nome);

        let altura = numeros.size().y.max(nome.size().y).max(18.0);
        let (rect, _) = ui.allocate_exact_size(vec2(largura, altura + 2.0), Sense::hover());
        let linha = Rect::from_min_size(rect.min, vec2(largura, altura));

        let moldura = Rect::from_min_size(pos2(linha.min.x + 18.0, linha.min.y), vec2(18.0, 18.0));
        ui.painter().rect_filled(moldura, 3, branco(0x22));
        if let Some(icone) = s.icone.as_deref().and_then(|c| self.textura(ui.ctx(), c)) {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            ui.painter().image(icone.id(), moldura, uv, Color32::WHITE);
        }
        let centro = linha.center().y;
        ui.painter().galley(pos2(linha.min.x + inicio_nome, centro - nome.size().y / 2.0), nome, texto());
        let x_numeros = linha.max.x - 6.0 - numeros.size().x;
        ui.painter().galley(pos2(x_numeros, centro - numeros.size().y / 2.0), numeros, texto());
    }

    fn status(&mut self, ui: &mut Ui) {
        let mut status = match &self.erro_captura {
            Some(erro) => erro.clone(),
            None => {
                let mut status = match &self.fluxo {
                    None => "Procurando o servidor do jogo...".to_string(),
                    Some(fluxo) => format!("Servidor {}", fluxo.split(' ').next().unwrap_or(fluxo)),
                };
                if self.catalogo.quantidade() == 0 {
                    status += "  ·  baixando nomes das skills...";
                }
                status
            }
        };
        // Versão do build, para os amigos dizerem qual usam.
        status += concat!("  ·  v", env!("CARGO_PKG_VERSION"));
        ui.add(egui::Label::new(RichText::new(status).font(fonte(10.0, false)).color(branco(0x99))).wrap());
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
}

impl eframe::App for Overlay {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        manter_sem_ativar(frame);
        if self.lido_em.elapsed() >= INTERVALO {
            self.ler_placar();
        }
        if self.salvo_em.elapsed() >= SALVAR_A_CADA {
            self.salvar_memoria();
        }
        // Sem isso o egui só redesenha com input, e o placar congelaria.
        ctx.request_repaint_after(INTERVALO);

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            // Arrasta a janela pelo fundo; botões e linhas ficam por cima e pegam o clique.
            let fundo = ui.interact(ui.max_rect(), ui.id().with("arrastar"), Sense::drag());
            if fundo.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
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

            // Altura pelo conteúdo (o SizeToContent do WPF): só manda o comando quando muda.
            let altura = quadro.response.rect.height().ceil();
            if (altura - self.altura).abs() >= 1.0 {
                self.altura = altura;
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(LARGURA, altura)));
            }
        });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.captura = None;
        self.salvar_memoria();
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

// Classe, level e power sempre: "?" = ainda não chegou; "~" = da memória (visto antes, pode estar velho).
fn linha_perfil(j: &LinhaJogador) -> LayoutJob {
    let (normal, apagado) = (branco(0xCC), branco(0x77));
    let mut job = LayoutJob::default();
    if j.classe.is_empty() {
        trecho(&mut job, "Classe ?", 10.0, false, apagado);
    } else {
        trecho(&mut job, j.classe, 10.0, false, normal);
    }
    let valor = |job: &mut LayoutJob, valor: i32, lembrado: bool| match (valor, lembrado) {
        (..=0, _) => trecho(job, "?", 10.0, false, apagado),
        (_, true) => trecho(job, &format!("~{valor}"), 10.0, false, apagado),
        (_, false) => trecho(job, &valor.to_string(), 10.0, false, normal),
    };
    trecho(&mut job, "  ·  Nv ", 10.0, false, normal);
    valor(&mut job, j.nivel, j.nivel_lembrado);
    trecho(&mut job, "  ·  Power ", 10.0, false, normal);
    valor(&mut job, j.poder, j.poder_lembrado);
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
    // Segoe UI Symbol cobre ☠ ▸ ▾ ✕, que a Segoe UI não tem.
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
    Ok(())
}

/// Sem isto, clicar no overlay tira o foco do teclado do jogo. Conferido a cada quadro: o winit
/// recalcula o estilo da janela quando ela aparece e apaga o bit aplicado na criação.
fn manter_sem_ativar(janela: &impl raw_window_handle::HasWindowHandle) {
    use raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE};

    let Ok(handle) = janela.window_handle() else { return };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else { return };
    let janela = win32.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    unsafe {
        let estilo = GetWindowLongPtrW(janela, GWL_EXSTYLE);
        if estilo & WS_EX_NOACTIVATE as isize == 0 {
            SetWindowLongPtrW(janela, GWL_EXSTYLE, estilo | WS_EX_NOACTIVATE as isize);
        }
    }
}

/// Caixa de mensagem do Windows (o build de release não tem console).
pub fn avisar(mensagem: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let utf16 = |texto: &str| texto.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (texto, titulo) = (utf16(mensagem), utf16("Aion2Meter"));
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
