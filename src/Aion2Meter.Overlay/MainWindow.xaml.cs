using System.Globalization;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using Aion2Meter.Core.Captura;
using Aion2Meter.Core.Medicao;

namespace Aion2Meter.Overlay;

public partial class MainWindow : Window
{
    private static readonly CultureInfo PtBr = CultureInfo.GetCultureInfo("pt-BR");

    private static readonly Dictionary<string, Color> CorPorClasse = new()
    {
        ["Gladiator"] = Color.FromRgb(0xC7, 0x9C, 0x6E),
        ["Templar"] = Color.FromRgb(0xF5, 0x8C, 0xBA),
        ["Assassin"] = Color.FromRgb(0xFF, 0xF5, 0x69),
        ["Ranger"] = Color.FromRgb(0xAB, 0xD4, 0x73),
        ["Sorcerer"] = Color.FromRgb(0x69, 0xCC, 0xF0),
        ["Elementalist"] = Color.FromRgb(0x94, 0x82, 0xC9),
        ["Spirit"] = Color.FromRgb(0x94, 0x82, 0xC9),
        ["Cleric"] = Color.FromRgb(0xE8, 0xE8, 0xE8),
        ["Chanter"] = Color.FromRgb(0x3E, 0x9B, 0xFF),
        ["Brawler"] = Color.FromRgb(0xFF, 0x7D, 0x0A),
    };

    private static readonly Color CorAggro = Color.FromRgb(0xFF, 0xB5, 0x47);
    private static readonly Color CorMorte = Color.FromRgb(0xFF, 0x6B, 0x6B);

    private enum Aba { Dps, Tank, Healer }

    private readonly Sessao sessao = new();
    private readonly CapturaSocketBruto? captura;
    private readonly DispatcherTimer relogio = new() { Interval = TimeSpan.FromMilliseconds(500) };
    private readonly HashSet<(Aba, uint)> expandidos = [];
    private readonly string? erroCaptura;
    private readonly CatalogoSkills catalogo = new();
    private readonly Dictionary<string, ImageSource> icones = [];
    private Aba aba = Aba.Dps;

    // Último level e power de cada nome (inclusive o seu), para o overlay aberto no meio da sessão.
    private static readonly string ArquivoMemoria = System.IO.Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Aion2Meter", "jogadores.json");

    private sealed record MemoriaSalva(string? Eu, Dictionary<string, PerfilJogador> Perfis);

    private int ticksDesdeSalvar;

    // Versão do build (Directory.Build.props ou a tag da release), para os amigos dizerem qual usam.
    private static readonly string Versao =
        typeof(MainWindow).Assembly.GetCustomAttributes(typeof(System.Reflection.AssemblyInformationalVersionAttribute), false)
            .OfType<System.Reflection.AssemblyInformationalVersionAttribute>().FirstOrDefault()?.InformationalVersion.Split('+')[0] ?? "?";

    public MainWindow()
    {
        InitializeComponent();
        DadosJogo.CarregarNomesPadrao();
        DadosJogo.Catalogo = catalogo;
        CarregarMemoria();

        try
        {
            captura = new CapturaSocketBruto(sessao.AoSegmento);
            captura.Iniciar();
        }
        catch (Exception ex)
        {
            captura = null;
            erroCaptura = "Captura parada: " + ex.Message;
        }

        relogio.Tick += (_, _) =>
        {
            Atualizar();
            if (++ticksDesdeSalvar >= 60) SalvarMemoria(); // a cada 30 s
        };
        relogio.Start();
        Atualizar();
    }

    private void Atualizar()
    {
        var placar = sessao.Medidor.ObterPlacar();
        var tabela = aba switch { Aba.Tank => placar.DanoRecebido, Aba.Healer => placar.Cura, _ => placar.Dano };

        Titulo.Text = tabela.Jogadores.Count == 0
            ? "AION2 Medidor"
            : $"AION2  ·  {placar.Duracao:mm\\:ss}  ·  {Compacto(tabela.Total)}";

        foreach (var (botao, dela) in new[] { (AbaDps, Aba.Dps), (AbaTank, Aba.Tank), (AbaHealer, Aba.Healer) })
        {
            botao.Background = dela == aba ? new SolidColorBrush(Color.FromArgb(0x44, 0xFF, 0xFF, 0xFF)) : Brushes.Transparent;
            botao.FontWeight = dela == aba ? FontWeights.SemiBold : FontWeights.Normal;
        }

        Status.Text = (erroCaptura
                      ?? (sessao.Fluxo is null ? "Procurando o servidor do jogo..." : $"Servidor {sessao.Fluxo.Split(' ')[0]}")
                      + (catalogo.Quantidade == 0 ? "  ·  baixando nomes das skills..." : ""))
                      + "  ·  v" + Versao;

        Linhas.Children.Clear();
        if (tabela.Jogadores.Count == 0)
        {
            Linhas.Children.Add(new TextBlock
            {
                Text = aba switch
                {
                    Aba.Tank => "Nenhum golpe de monstro em jogador ainda.",
                    Aba.Healer => "Nenhuma cura vista ainda.",
                    _ => "Sem dano ainda.",
                },
                Opacity = 0.6,
            });
            return;
        }

        double maior = tabela.Jogadores[0].Total;
        foreach (var j in tabela.Jogadores)
        {
            Linhas.Children.Add(CriarLinhaJogador(j, maior));
            if (!expandidos.Contains((aba, j.Id))) continue;

            Linhas.Children.Add(new TextBlock
            {
                Text = Detalhe(j),
                FontSize = 10, Opacity = 0.7, Margin = new Thickness(18, 0, 0, 2), TextWrapping = TextWrapping.Wrap,
            });
            foreach (var s in j.Skills.Take(8))
                Linhas.Children.Add(CriarLinhaSkill(s));
        }
    }

    private string Detalhe(LinhaJogador j)
    {
        if (aba != Aba.Tank)
            return string.Format(PtBr, "{0} {1}  ·  crítico {2:P0}",
                j.Golpes, aba == Aba.Healer ? "curas" : "golpes", j.Golpes > 0 ? (double)j.Criticos / j.Golpes : 0);

        string texto = string.Format(PtBr, "{0} golpes recebidos  ·  {1} aparados ({2:P0})  ·  {3} {4}",
            j.Golpes, j.Aparos, j.Golpes > 0 ? (double)j.Aparos / j.Golpes : 0, j.Mortes, j.Mortes == 1 ? "morte" : "mortes");
        if (j.SegurandoAggro > 0)
            texto += string.Format(PtBr, "  ·  alvo de {0} {1}", j.SegurandoAggro, j.SegurandoAggro == 1 ? "monstro" : "monstros");
        return texto;
    }

    private UIElement CriarLinhaJogador(LinhaJogador j, double maior)
    {
        var cor = CorPorClasse.GetValueOrDefault(j.Classe, Color.FromRgb(0xA0, 0xA0, 0xA0));
        double fracao = maior > 0 ? j.Total / maior : 0;

        var linha = new Grid { Margin = new Thickness(0, 1, 0, 1), Cursor = Cursors.Hand, Background = Brushes.Transparent };
        linha.ColumnDefinitions.Add(new ColumnDefinition());
        linha.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var barra = new Grid();
        Grid.SetColumnSpan(barra, 2);
        barra.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(fracao, GridUnitType.Star) });
        barra.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1 - fracao, GridUnitType.Star) });
        barra.Children.Add(new Border { Background = new SolidColorBrush(Color.FromArgb(0x66, cor.R, cor.G, cor.B)), CornerRadius = new CornerRadius(3) });
        linha.Children.Add(barra);

        var esquerda = new StackPanel { Margin = new Thickness(6, 2, 8, 2), VerticalAlignment = VerticalAlignment.Center };
        var nome = new TextBlock
        {
            FontWeight = j.Voce ? FontWeights.SemiBold : FontWeights.Normal,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        nome.Inlines.Add($"{(expandidos.Contains((aba, j.Id)) ? "▾" : "▸")} {j.Nome}{(j.Voce ? " (você)" : "")}");
        // Aggro: quantos monstros têm este jogador como último alvo. A ameaça em número fica no servidor.
        if (aba == Aba.Tank && j.SegurandoAggro > 0)
            nome.Inlines.Add(new Run($"  aggro {j.SegurandoAggro}") { FontSize = 10, FontWeight = FontWeights.SemiBold, Foreground = new SolidColorBrush(CorAggro) });
        if (aba == Aba.Tank && j.Mortes > 0)
            nome.Inlines.Add(new Run($"  ☠{j.Mortes}") { FontSize = 10, Foreground = new SolidColorBrush(CorMorte) });
        esquerda.Children.Add(nome);
        esquerda.Children.Add(CriarLinhaPerfil(j));
        linha.Children.Add(esquerda);

        var numeros = new TextBlock { VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 6, 0) };
        numeros.Inlines.Add(new Run($"{Compacto(j.PorSegundo)} {aba switch { Aba.Tank => "DTPS", Aba.Healer => "HPS", _ => "DPS" }}") { FontWeight = FontWeights.SemiBold });
        numeros.Inlines.Add(string.Format(PtBr, "  {0}  {1:P0}", Compacto(j.Total), j.Porcentagem));
        Grid.SetColumn(numeros, 1);
        linha.Children.Add(numeros);

        // MouseDown e não MouseUp: senão o DragMove da janela captura o clique.
        linha.MouseLeftButtonDown += (_, e) =>
        {
            if (!expandidos.Add((aba, j.Id))) expandidos.Remove((aba, j.Id));
            e.Handled = true;
            Atualizar();
        };
        return linha;
    }

    // Classe, level e power sempre: "?" = ainda não chegou; "~" = da memória (visto antes, pode estar velho).
    private static TextBlock CriarLinhaPerfil(LinhaJogador j)
    {
        var apagado = new SolidColorBrush(Color.FromArgb(0x77, 0xFF, 0xFF, 0xFF));
        var linha = new TextBlock { FontSize = 10, Margin = new Thickness(12, 0, 0, 0), Foreground = new SolidColorBrush(Color.FromArgb(0xCC, 0xFF, 0xFF, 0xFF)) };
        linha.Inlines.Add(j.Classe.Length > 0 ? new Run(j.Classe) : new Run("Classe ?") { Foreground = apagado });
        linha.Inlines.Add("  ·  Nv ");
        linha.Inlines.Add(Valor(j.Nivel, j.NivelLembrado));
        linha.Inlines.Add("  ·  Power ");
        linha.Inlines.Add(Valor(j.Poder, j.PoderLembrado));
        return linha;

        Run Valor(int valor, bool lembrado) =>
            valor <= 0 ? new Run("?") { Foreground = apagado }
            : lembrado ? new Run("~" + valor.ToString(CultureInfo.InvariantCulture)) { Foreground = apagado }
            : new Run(valor.ToString(CultureInfo.InvariantCulture));
    }

    private void CarregarMemoria()
    {
        try
        {
            if (!System.IO.File.Exists(ArquivoMemoria)) return;
            var salva = JsonSerializer.Deserialize<MemoriaSalva>(System.IO.File.ReadAllText(ArquivoMemoria));
            if (salva is not null) sessao.Medidor.CarregarMemoria(salva.Eu, salva.Perfis ?? []);
        }
        catch (Exception ex) when (ex is System.IO.IOException or JsonException or UnauthorizedAccessException)
        {
            // Memória corrompida ou inacessível: começa sem ela.
        }
    }

    private void SalvarMemoria()
    {
        ticksDesdeSalvar = 0;
        var (eu, perfis) = sessao.Medidor.ExportarMemoria();
        if (perfis.Count == 0) return;
        try
        {
            System.IO.Directory.CreateDirectory(System.IO.Path.GetDirectoryName(ArquivoMemoria)!);
            string temporario = ArquivoMemoria + ".tmp";
            System.IO.File.WriteAllText(temporario, JsonSerializer.Serialize(new MemoriaSalva(eu, perfis)));
            System.IO.File.Move(temporario, ArquivoMemoria, overwrite: true);
        }
        catch (Exception ex) when (ex is System.IO.IOException or UnauthorizedAccessException)
        {
            // Sem disco agora: tenta de novo no próximo ciclo.
        }
    }

    private UIElement CriarLinhaSkill(LinhaSkill s)
    {
        var linha = new Grid { Margin = new Thickness(18, 0, 6, 2) };
        linha.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        linha.ColumnDefinitions.Add(new ColumnDefinition());
        linha.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        // Moldura fixa: a linha não pula quando o ícone termina de baixar.
        var icone = new Border { Width = 18, Height = 18, Margin = new Thickness(0, 0, 6, 0), CornerRadius = new CornerRadius(3), Background = new SolidColorBrush(Color.FromArgb(0x22, 0xFF, 0xFF, 0xFF)) };
        if (CarregarIcone(s.Icone) is { } imagem)
            icone.Child = new Image { Source = imagem, Stretch = Stretch.UniformToFill };
        linha.Children.Add(icone);

        var nome = new TextBlock { Text = s.Nome, FontSize = 11, Opacity = 0.9, VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
        Grid.SetColumn(nome, 1);
        linha.Children.Add(nome);

        var numeros = new TextBlock
        {
            Text = string.Format(PtBr, "{0}  {1:P0}  {2}x", Compacto(s.Total), s.Porcentagem, s.Golpes),
            FontSize = 11, Opacity = 0.85, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(8, 0, 0, 0),
        };
        Grid.SetColumn(numeros, 2);
        linha.Children.Add(numeros);
        return linha;
    }

    private ImageSource? CarregarIcone(string? caminho)
    {
        if (caminho is null) return null;
        if (icones.TryGetValue(caminho, out var pronta)) return pronta;
        try
        {
            var imagem = new BitmapImage();
            imagem.BeginInit();
            imagem.UriSource = new Uri(caminho);
            imagem.DecodePixelWidth = 36; // o PNG original tem ~100 KB; decodifica pequeno
            imagem.CacheOption = BitmapCacheOption.OnLoad;
            imagem.EndInit();
            imagem.Freeze();
            icones[caminho] = imagem;
            return imagem;
        }
        catch (Exception ex) when (ex is System.IO.IOException or NotSupportedException or UriFormatException)
        {
            return null;
        }
    }

    private static string Compacto(double valor) => valor switch
    {
        >= 1_000_000 => (valor / 1_000_000).ToString("0.00", PtBr) + "M",
        >= 10_000 => (valor / 1_000).ToString("0.0", PtBr) + "K",
        _ => valor.ToString("N0", PtBr),
    };

    private void AoArrastar(object sender, MouseButtonEventArgs e)
    {
        if (e.ButtonState == MouseButtonState.Pressed) DragMove();
    }

    private void AoTrocarAba(object sender, RoutedEventArgs e)
    {
        aba = sender == AbaTank ? Aba.Tank : sender == AbaHealer ? Aba.Healer : Aba.Dps;
        Atualizar();
    }

    private void AoZerar(object sender, RoutedEventArgs e)
    {
        sessao.Medidor.Reiniciar();
        expandidos.Clear();
        Atualizar();
    }

    private void AoFechar(object sender, RoutedEventArgs e) => Close();

    // Sem isto, clicar no overlay tira o foco do teclado do jogo.
    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        const int GwlExStyle = -20;
        const nint WsExNoActivate = 0x08000000;
        var janela = new WindowInteropHelper(this).Handle;
        SetWindowLongPtr(janela, GwlExStyle, GetWindowLongPtr(janela, GwlExStyle) | WsExNoActivate);
    }

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    private static extern nint GetWindowLongPtr(nint janela, int indice);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")]
    private static extern nint SetWindowLongPtr(nint janela, int indice, nint valor);

    protected override void OnClosed(EventArgs e)
    {
        relogio.Stop();
        captura?.Dispose();
        SalvarMemoria();
        catalogo.Dispose();
        base.OnClosed(e);
    }
}
