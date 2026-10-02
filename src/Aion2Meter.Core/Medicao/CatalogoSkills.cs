using System.Collections.Concurrent;
using System.Text.Json;
using System.Threading.Channels;

namespace Aion2Meter.Core.Medicao;

/// <summary>
/// Nome em português e ícone de cada skill, buscados sob demanda e guardados em disco.
/// Nomes: questlog.gg, base comunitária montada a partir do cliente Global (idioma "pt"),
/// API não documentada: pode mudar sem aviso. Ícones: CDN oficial da NCSoft.
/// Uma requisição por vez, com intervalo, para não sobrecarregar ninguém.
/// </summary>
public sealed class CatalogoSkills : IDisposable
{
    /// <summary>Cura: null enquanto o detalhe da skill não foi consultado (a listagem não traz).</summary>
    public sealed record Info(string Nome, string? Icone, bool? Cura = null);

    private const string Api = "https://questlog.gg/aion-2/api/trpc/database.";
    private const string Cdn = "https://assets.playnccdn.com/static-aion2-gamedata/resources/";
    private const string Idioma = "pt";
    private static readonly string[] Classes =
        ["gladiator", "templar", "assassin", "ranger", "sorcerer", "elementalist", "cleric", "chanter", "brawler"];
    private static readonly TimeSpan IntervaloEntreRequisicoes = TimeSpan.FromMilliseconds(400);

    private readonly string pasta;
    private readonly string arquivoNomes;
    private readonly ConcurrentDictionary<uint, Info> infos = new();
    private readonly ConcurrentDictionary<string, byte> jaPedido = new();
    private readonly Channel<string> fila = Channel.CreateUnbounded<string>();
    private readonly HttpClient http = new() { Timeout = TimeSpan.FromSeconds(20) };
    private readonly CancellationTokenSource parar = new();
    private readonly TaskCompletionSource pronto = new(TaskCreationOptions.RunContinuationsAsynchronously);

    /// <summary>Conclui quando o cache foi lido ou a listagem inicial terminou (com ou sem rede).</summary>
    public Task Pronto => pronto.Task;

    public CatalogoSkills(string? pasta = null)
    {
        this.pasta = pasta ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Aion2Meter");
        Directory.CreateDirectory(Path.Combine(this.pasta, "icones"));
        arquivoNomes = Path.Combine(this.pasta, $"skills-{Idioma}.json");
        http.DefaultRequestHeaders.UserAgent.ParseAdd("Aion2Meter/0.1 (medidor de DPS pessoal)");

        try
        {
            if (File.Exists(arquivoNomes))
                foreach (var (id, info) in JsonSerializer.Deserialize<Dictionary<uint, Info>>(File.ReadAllText(arquivoNomes)) ?? [])
                    infos[id] = info;
        }
        catch (JsonException) { /* cache corrompido: refaz do zero */ }

        _ = Task.Run(Trabalhar);
    }

    public int Quantidade => infos.Count;

    /// <summary>Nome e ícone da skill base; null enquanto não chegou (pede a busca uma vez).</summary>
    public Info? Obter(uint skillBase)
    {
        if (infos.TryGetValue(skillBase, out var info)) return info;
        Pedir("skill:" + skillBase);
        return null;
    }

    /// <summary>
    /// Se a skill recupera PV: a descrição de cura usa o marcador SkillUIHPHeal (a de dano usa
    /// SkillUIMinDmgSum). null enquanto não consultou; pede o detalhe uma vez.
    /// </summary>
    public bool? EhCura(uint skillBase)
    {
        if (infos.TryGetValue(skillBase, out var info) && info.Cura is { } cura) return cura;
        Pedir("skill:" + skillBase);
        return null;
    }

    /// <summary>Caminho local do PNG do ícone; null enquanto não baixou (pede o download uma vez).</summary>
    public string? CaminhoIcone(string? icone)
    {
        if (string.IsNullOrEmpty(icone)) return null;
        string caminho = Path.Combine(pasta, "icones", icone + ".png");
        if (File.Exists(caminho)) return caminho;
        Pedir("icone:" + icone);
        return null;
    }

    private void Pedir(string item)
    {
        if (jaPedido.TryAdd(item, 0)) fila.Writer.TryWrite(item);
    }

    private async Task Trabalhar()
    {
        var ct = parar.Token;
        try
        {
            // Primeira execução: uma listagem por classe cobre quase todas as skills ativas.
            if (infos.IsEmpty)
            {
                foreach (var classe in Classes)
                {
                    await BaixarListagem(classe, ct);
                    await Task.Delay(IntervaloEntreRequisicoes, ct);
                }
                Salvar();
            }
            pronto.TrySetResult();

            await foreach (var item in fila.Reader.ReadAllAsync(ct))
            {
                try
                {
                    if (item.StartsWith("skill:")) { await BaixarSkill(uint.Parse(item[6..]), ct); Salvar(); }
                    else await BaixarIcone(item[6..], ct);
                }
                catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException or KeyNotFoundException or IOException)
                {
                    // Falha de rede ou formato: a skill fica com o nome de reserva até a próxima execução.
                }
                await Task.Delay(IntervaloEntreRequisicoes, ct);
            }
        }
        catch (OperationCanceledException) { }
        catch (Exception ex) when (ex is HttpRequestException or JsonException or IOException) { }
        finally { pronto.TrySetResult(); }
    }

    private async Task BaixarListagem(string classe, CancellationToken ct)
    {
        try
        {
            var dados = await Trpc("getSkills", new { language = Idioma, page = 1, mainCategory = classe }, ct);
            if (!dados.TryGetProperty("pageData", out var lista)) return;
            foreach (var s in lista.EnumerateArray()) Guardar(s);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException) { }
    }

    private async Task BaixarSkill(uint id, CancellationToken ct)
    {
        var dados = await Trpc("getSkill", new { id = id.ToString(), language = Idioma }, ct);
        Guardar(dados);
    }

    private void Guardar(JsonElement s)
    {
        if (!s.TryGetProperty("id", out var id) || !uint.TryParse(id.GetString(), out uint codigo)) return;
        if (!s.TryGetProperty("name", out var nome) || string.IsNullOrWhiteSpace(nome.GetString())) return;
        // "/assets/.../ICON_RA_SKILL_034.ICON_RA_SKILL_034" → "ICON_RA_SKILL_034"
        string? icone = s.TryGetProperty("icon", out var i) ? i.GetString()?.Split('.').Last() : null;
        bool? cura = s.TryGetProperty("descriptionData", out var desc)
            ? desc.GetRawText().Contains("SkillUIHPHeal", StringComparison.Ordinal)
            : infos.GetValueOrDefault(codigo)?.Cura;
        infos[codigo] = new Info(nome.GetString()!, icone, cura);
    }

    private async Task BaixarIcone(string icone, CancellationToken ct)
    {
        if (icone.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0) return;
        byte[] png = await http.GetByteArrayAsync(Cdn + icone + ".png", ct);
        string destino = Path.Combine(pasta, "icones", icone + ".png");
        await File.WriteAllBytesAsync(destino + ".tmp", png, ct);
        File.Move(destino + ".tmp", destino, overwrite: true);
    }

    private async Task<JsonElement> Trpc(string procedimento, object entrada, CancellationToken ct)
    {
        string url = Api + procedimento + "?input=" + Uri.EscapeDataString(JsonSerializer.Serialize(entrada));
        using var doc = JsonDocument.Parse(await http.GetStringAsync(url, ct));
        return doc.RootElement.GetProperty("result").GetProperty("data").Clone();
    }

    private void Salvar()
    {
        string tmp = arquivoNomes + ".tmp";
        File.WriteAllText(tmp, JsonSerializer.Serialize(infos.ToDictionary(p => p.Key, p => p.Value)));
        File.Move(tmp, arquivoNomes, overwrite: true);
    }

    public void Dispose()
    {
        parar.Cancel();
        http.Dispose();
    }
}
