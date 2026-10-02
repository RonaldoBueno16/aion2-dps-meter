using System.Text.Json;

namespace Aion2Meter.Core.Medicao;

/// <summary>
/// Classificação de skills e nomes. Nome e ícone em português vêm do <see cref="Catalogo"/>;
/// o skills.json em inglês (opcional) é só reserva para o que o catálogo não tiver.
/// </summary>
public static class DadosJogo
{
    // Os 2 primeiros dígitos do código da skill dizem a classe (8 dígitos: 14340000 = Ranger).
    // Nomes de classe ficam em inglês por escolha do usuário (2026-10-02), mesmo com o cliente em português.
    private static readonly Dictionary<int, string> Classes = new()
    {
        [10] = "Spirit",
        [11] = "Gladiator",
        [12] = "Templar",
        [13] = "Assassin",
        [14] = "Ranger",
        [15] = "Sorcerer",
        [16] = "Elementalist",
        [17] = "Cleric",
        [18] = "Chanter",
        [19] = "Brawler",
    };

    // Curas que chegam pelo mesmo opcode do dano e não podem entrar na soma.
    private static readonly HashSet<uint> Curas =
        [18120000, 18170000, 16770000, 16190000, 17120000, 17800000, 17100000, 17410000];

    private static Dictionary<string, string> nomes = [];

    public static CatalogoSkills? Catalogo { get; set; }

    public static void CarregarNomes(string caminho)
    {
        if (File.Exists(caminho))
            nomes = JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(caminho)) ?? [];
    }

    /// <summary>Procura dados/skills.json a partir da pasta do executável, subindo até a raiz.</summary>
    public static string? CarregarNomesPadrao()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            string caminho = Path.Combine(dir.FullName, "dados", "skills.json");
            if (!File.Exists(caminho)) continue;
            CarregarNomes(caminho);
            return caminho;
        }
        return null;
    }

    public static int NomesCarregados => nomes.Count;

    /// <summary>Skill de jogador (8 dígitos, prefixo 10 a 19), pedra Theo (3.0xx.xxx) ou pet (1xx.xxx).
    /// Fica de fora skill de NPC (1.000.000 a 9.999.999 fora da faixa Theo).</summary>
    public static bool EhSkillDeJogador(uint skill) =>
        skill is >= 10_000_000 and < 20_000_000
            or >= 3_000_000 and < 3_100_000
            or >= 100_000 and < 200_000;

    /// <summary>Lista fixa (curas conhecidas do RATmeter) ou marcador de cura no catálogo pt-BR.</summary>
    public static bool EhCura(uint skill) =>
        Curas.Contains(skill) || Curas.Contains(SkillBase(skill))
        || (EhSkillDeJogador(skill) && Catalogo?.EhCura(SkillBase(skill)) == true);

    public static string Classe(uint skill) =>
        skill is >= 10_000_000 and < 20_000_000 && Classes.TryGetValue((int)(skill / 1_000_000), out var c) ? c : "";

    /// <summary>
    /// Skill de jogador: português do catálogo; senão inglês exato ou da skill base; senão o código.
    /// Skill de monstro: o catálogo pt-BR não tem, então "Golpe de monstro (código)" em vez de inglês.
    /// </summary>
    public static string NomeSkill(uint skill) =>
        !EhSkillDeJogador(skill) ? $"Golpe de monstro ({skill})"
        : Catalogo?.Obter(SkillBase(skill))?.Nome is { } pt ? pt
        : nomes.TryGetValue(skill.ToString(), out var n) ? n
        : nomes.TryGetValue((skill / 10_000 * 10_000).ToString(), out var b) ? b
        : $"Skill {skill}";

    /// <summary>Caminho local do ícone (PNG), ou null enquanto não baixou.</summary>
    public static string? IconeSkill(uint skill) =>
        Catalogo is { } c && EhSkillDeJogador(skill) ? c.CaminhoIcone(c.Obter(SkillBase(skill))?.Icone) : null;

    /// <summary>Agrupa variantes da mesma skill (14030010, 14030020... viram 14030000).</summary>
    public static uint SkillBase(uint skill) =>
        skill is >= 10_000_000 and < 20_000_000 ? skill / 10_000 * 10_000 : skill;
}
