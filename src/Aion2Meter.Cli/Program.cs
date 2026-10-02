// Dois modos com o mesmo núcleo do medidor:
//   captura.pcapng [--hex N] [--op 3804] [--procurar-quedas] [--entidade ID]
//       reprocessa uma captura: sincronização, LZ4, opcodes, conferência com HP e placar.
//   ao-vivo [segundos]
//       captura por raw socket (precisa de administrador) e imprime o placar.
using System.Globalization;
using Aion2Meter.Core.Captura;
using Aion2Meter.Core.Medicao;
using Aion2Meter.Core.Protocolo;

Console.OutputEncoding = System.Text.Encoding.UTF8;
var ptBr = CultureInfo.GetCultureInfo("pt-BR");
string? arquivoNomes = DadosJogo.CarregarNomesPadrao();
using var catalogo = new CatalogoSkills();
DadosJogo.Catalogo = catalogo;
catalogo.Pronto.Wait(TimeSpan.FromSeconds(30));
Console.WriteLine($"Nomes de skills: {catalogo.Quantidade} em português, {DadosJogo.NomesCarregados} em inglês de reserva ({arquivoNomes ?? "dados/skills.json não encontrado"})");

if (args.FirstOrDefault() == "ao-vivo")
{
    int segundos = int.Parse(args.ElementAtOrDefault(1) ?? "60");
    var sessao = new Sessao();
    using var captura = new CapturaSocketBruto(sessao.AoSegmento);
    captura.Iniciar();
    Console.WriteLine($"Capturando em {string.Join(", ", captura.Enderecos)} por {segundos} s");
    var fim = DateTime.UtcNow.AddSeconds(segundos);
    while (DateTime.UtcNow < fim)
    {
        Thread.Sleep(5000);
        var p = sessao.Medidor.ObterPlacar().Dano;
        Console.WriteLine($"[{DateTime.Now:HH:mm:ss}] IP recebidos {captura.Recebidos}, TCP {captura.SegmentosTcp}, " +
                          $"fluxo {sessao.Fluxo ?? "(procurando)"}, pacotes {sessao.Pacotes}, lacunas {sessao.Lacunas}, " +
                          $"dessinc. {sessao.Dessincronizacoes}, jogadores {p.Jogadores.Count}");
    }
    Placares.Imprimir(sessao.Medidor.ObterPlacar(), ptBr);
    return;
}

string? ValorDe(string opcao) => args.SkipWhile(a => a != opcao).Skip(1).FirstOrDefault();
string caminho = args.FirstOrDefault(a => !a.StartsWith("--") && a != ValorDe("--hex") && a != ValorDe("--op") && a != ValorDe("--entidade")) ?? "captura.pcapng";
int amostrasHex = int.Parse(ValorDe("--hex") ?? "0");
ushort opAmostra = Convert.ToUInt16(ValorDe("--op") ?? "3804", 16);
FluxoTcp.ProcurarQuedas = args.Contains("--procurar-quedas");
if (ValorDe("--entidade") is { } entidade) FluxoTcp.Entidade = uint.Parse(entidade);

var streams = new Dictionary<string, FluxoTcp>();
int quadros = 0, tcp = 0;

foreach (var quadro in LeitorPcapng.Ler(caminho))
{
    quadros++;
    if (!SegmentoTcp.TentarExtrair(quadro.Dados, quadro.TipoEnlace, out var seg)) continue;
    tcp++;
    if (!streams.TryGetValue(seg.Chave, out var s)) streams[seg.Chave] = s = new FluxoTcp(seg.Chave, opAmostra);
    s.Hora = quadro.Hora;
    s.Montador.Adicionar(seg.Seq, seg.Syn, seg.Dados,
        dados => s.Enquadrador.Adicionar(dados, s.AoPacoteBruto),
        s.Enquadrador.Reiniciar);
}

Console.WriteLine($"Arquivo: {Path.GetFullPath(caminho)}");
Console.WriteLine($"Quadros: {quadros} | TCP/IPv4: {tcp} | streams: {streams.Count}");

foreach (var s in streams.Values.OrderByDescending(s => s.Montador.BytesEntregues))
    s.Relatar(ptBr, amostrasHex);

// Mesmo arquivo pelo caminho do medidor ao vivo (Sessao + Medidor), sem fim de luta por inatividade.
var replay = new Sessao { Medidor = { Inatividade = TimeSpan.MaxValue } };
foreach (var quadro in LeitorPcapng.Ler(caminho))
    if (SegmentoTcp.TentarExtrair(quadro.Dados, quadro.TipoEnlace, out var seg))
        replay.AoSegmento(seg, quadro.Hora);
Console.WriteLine();
Console.WriteLine($"=== Placar pelo pipeline do medidor (fluxo {replay.Fluxo})");
Placares.Imprimir(replay.Medidor.ObterPlacar(), ptBr);

static class Placares
{
    public static void Imprimir(Placar placar, CultureInfo ptBr)
    {
        Console.WriteLine(string.Format(ptBr, "Luta de {0:N1} s", placar.Duracao.TotalSeconds));
        Imprimir("DPS (dano causado)", "DPS", placar.Dano, ptBr);
        Imprimir("Tank (dano recebido)", "DTPS", placar.DanoRecebido, ptBr);
        Imprimir("Healer (cura)", "HPS", placar.Cura, ptBr);
    }

    private static void Imprimir(string titulo, string porSegundo, Tabela tabela, CultureInfo ptBr)
    {
        Console.WriteLine(string.Format(ptBr, "--- {0}: total {1:N0}", titulo, tabela.Total));
        foreach (var j in tabela.Jogadores)
        {
            Console.WriteLine(string.Format(ptBr, "{0,-22} {1,-12} {2,14:N0} {3,10:N0} {4} {5,6:P1}  golpes {6}, crítico {7:P0}{8}{9}{10}{11}",
                j.Nome, (j.Classe + (j.Nivel > 0 ? $" Nv {j.Nivel}" : "") + (j.Poder > 0 ? $" Power {j.Poder}" : "")).Trim(), j.Total, j.PorSegundo, porSegundo, j.Porcentagem, j.Golpes,
                j.Golpes > 0 ? (double)j.Criticos / j.Golpes : 0,
                j.Aparos > 0 ? $", aparos {j.Aparos}" : "", j.Mortes > 0 ? $", mortes {j.Mortes}" : "",
                j.SegurandoAggro > 0 ? $", segurando {j.SegurandoAggro} mob(s)" : "", j.Voce ? "  (você)" : ""));
            foreach (var s in j.Skills.Take(10))
                Console.WriteLine(string.Format(ptBr, "    {0,-28} {1,14:N0} {2,6:P1}  {3,4}x  máx {4:N0}",
                    s.Nome, s.Total, s.Porcentagem, s.Golpes, s.Maximo));
        }
    }
}

sealed class FluxoTcp(string chave, ushort opAmostra)
{
    public readonly MontadorTcp Montador = new();
    public readonly Enquadrador Enquadrador = new();
    public readonly Desempacotador Desempacotador = new();
    public DateTime Hora;

    private int pacotesBrutos, pacotes;
    private readonly Dictionary<ushort, int> opcodes = [];
    private readonly Dictionary<string, int> falhas = [];
    private readonly List<(DateTime Hora, EventoDano Evento)> eventos = [];
    private readonly List<byte[]> amostras = [];
    private readonly Dictionary<uint, string> personagens = [];

    public void AoPacoteBruto(byte[] bruto)
    {
        pacotesBrutos++;
        Desempacotador.Expandir(bruto, AoPacote);
    }

    private void AoPacote(byte[] pacote)
    {
        pacotes++;
        if (!Opcodes.TentarLer(pacote, out ushort op)) return;
        opcodes[op] = opcodes.GetValueOrDefault(op) + 1;
        if (op == opAmostra && amostras.Count < 50) amostras.Add(pacote);
        Guardar(pacote);
        if (Entidade is { } ent) MostrarSeEnvolve(ent, op, pacote);

        switch (op)
        {
            case Opcodes.Dano:
                Registrar(Combate.TentarDano(pacote, out var dano, out var motivo), dano, "Dano: " + motivo);
                break;
            case Opcodes.DanoPeriodico:
                Registrar(Combate.TentarDanoPeriodico(pacote, out var dot, out var motivoDot), dot, "DoT: " + motivoDot);
                break;
            case Opcodes.InfoPersonagem:
                if (Combate.TentarInfoPersonagem(pacote, out uint id, out string nome, out _, out _)) personagens[id] = nome;
                break;
            case Opcodes.HpRestante:
                if (Combate.TentarHpRestante(pacote, out uint alvo, out ulong hp)) ConferirHp(alvo, hp);
                break;
        }
    }

    private void Registrar(bool ok, EventoDano evento, string motivo)
    {
        if (ok)
        {
            eventos.Add((Hora, evento));
            if (!eventosDesdeUltimoHp.TryGetValue(evento.AlvoId, out var lista))
                eventosDesdeUltimoHp[evento.AlvoId] = lista = [];
            lista.Add(evento);
        }
        else falhas[motivo] = falhas.GetValueOrDefault(motivo) + 1;
    }

    // Conferência do campo de dano: a queda de HP entre dois 0x8D00 do mesmo alvo
    // comparada com a soma dos 0x3804/0x3805 nesse alvo entre eles, na ordem do stream.
    public static bool ProcurarQuedas;
    private readonly Dictionary<uint, ulong> ultimoHp = [];
    private readonly Dictionary<uint, List<EventoDano>> eventosDesdeUltimoHp = [];
    private readonly List<(uint Alvo, long QuedaHp, ulong Dano, List<EventoDano> Eventos)> conferencias = [];

    private void ConferirHp(uint alvo, ulong hp)
    {
        var lista = eventosDesdeUltimoHp.GetValueOrDefault(alvo) ?? [];
        if (ultimoHp.TryGetValue(alvo, out ulong anterior))
        {
            ulong dano = (ulong)lista.Sum(e => (double)e.Dano);
            long queda = (long)anterior - (long)hp;
            if (queda != 0 || dano != 0) conferencias.Add((alvo, queda, dano, lista));
            if (ProcurarQuedas && queda > 0 && procurasQueda < 15) ProcurarQueda(alvo, queda, inicioIntervalo.GetValueOrDefault(alvo));
        }
        ultimoHp[alvo] = hp;
        eventosDesdeUltimoHp[alvo] = [];
        inicioIntervalo[alvo] = historico.Count;
    }

    // Diagnóstico: pacotes de combate e HP que citam uma entidade, em ordem (dano recebido, cura, HP do jogador).
    public static uint? Entidade;
    private DateTime? inicioDiag;

    private void MostrarSeEnvolve(uint entidade, ushort op, byte[] pacote)
    {
        if (op is not (Opcodes.Dano or Opcodes.DanoPeriodico or Opcodes.HpRestante or Opcodes.MorteEntidade)) return;
        var r = new LeitorPacote(pacote);
        uint primeiro;
        try { r.LerVarInt(); r.LerU16(); primeiro = (uint)r.LerVarInt(); } catch (FormatException) { return; }

        string detalhe = "";
        if (op == Opcodes.Dano)
        {
            bool ok = Combate.TentarDano(pacote, out var e, out var motivo);
            if (primeiro != entidade && !(ok && e.AutorId == entidade)) return;
            detalhe = ok ? $"autor {e.AutorId} -> alvo {e.AlvoId} skill {e.Skill} dano {e.Dano} tipo {e.TipoDano}{(e.Aparo ? " APARO" : "")}{(e.Perfeito ? " PERFEITO" : "")}" : $"(não leu: {motivo})";
        }
        else if (op == Opcodes.DanoPeriodico)
        {
            bool ok = Combate.TentarDanoPeriodico(pacote, out var e, out var motivo);
            if (primeiro != entidade && !(ok && e.AutorId == entidade)) return;
            detalhe = ok ? $"DoT autor {e.AutorId} -> alvo {e.AlvoId} skill {e.Skill} dano {e.Dano}" : $"(DoT não leu: {motivo})";
        }
        else if (primeiro != entidade) return;

        inicioDiag ??= Hora;
        Console.WriteLine($"  [{(Hora - inicioDiag.Value).TotalSeconds,6:F2}s] 0x{op:X4} {detalhe}");
        Console.WriteLine($"           {Convert.ToHexString(pacote)}");
    }

    // Diagnóstico: em que pacotes do intervalo aparece o valor exato da queda de HP?
    private readonly List<byte[]> historico = [];
    private readonly Dictionary<uint, int> inicioIntervalo = [];
    private int procurasQueda;

    public void Guardar(byte[] pacote) => historico.Add(pacote);

    private void ProcurarQueda(uint alvo, long queda, int desde)
    {
        procurasQueda++;
        Span<byte> varint = stackalloc byte[10];
        int n = 0;
        for (ulong v = (ulong)queda; ; v >>= 7)
        {
            varint[n++] = (byte)(v < 0x80 ? v : (v & 0x7F) | 0x80);
            if (v < 0x80) break;
        }
        byte[] padraoVarint = varint[..n].ToArray();
        byte[] padraoU32 = BitConverter.GetBytes((uint)queda);

        Console.WriteLine($"  [queda] alvo {alvo} caiu {queda} (varint {Convert.ToHexString(padraoVarint)}), pacotes no intervalo: {historico.Count - desde}");
        for (int i = desde; i < historico.Count; i++)
        {
            var p = historico[i];
            int posV = p.AsSpan().IndexOf(padraoVarint);
            int posU = p.AsSpan().IndexOf(padraoU32);
            Opcodes.TentarLer(p, out ushort op);
            if (posV >= 0 || posU >= 0)
                Console.WriteLine($"     0x{op:X4} varint@{posV} u32@{posU}: {Convert.ToHexString(p)}");
            else if (op == Opcodes.Dano)
                Console.WriteLine($"     0x{op:X4} (sem o valor): {Convert.ToHexString(p)}");
        }
    }

    private void RelatarConferenciaHp()
    {
        if (conferencias.Count == 0) return;
        int iguais = conferencias.Count(c => c.QuedaHp == (long)c.Dano);
        Console.WriteLine($"  Conferência HP x dano: {iguais}/{conferencias.Count} intervalos com queda de HP igual à soma do dano");

        // HP de jogador tem outro formato e dá valores absurdos: só intervalos com HP de mob plausível.
        const long QuedaMaximaPlausivel = 1_000_000_000;
        var validos = conferencias.Where(c => c.QuedaHp > 0 && c.QuedaHp < QuedaMaximaPlausivel && c.Dano > 0).ToList();
        var semDano = conferencias.Count(c => c.QuedaHp > 0 && c.Dano == 0);
        var semQueda = conferencias.Count(c => c.QuedaHp <= 0 && c.Dano > 0);
        Console.WriteLine($"  Razão queda/dano em {validos.Count} intervalos (queda sem dano: {semDano}, dano sem queda: {semQueda})");

        static string Resumo(IEnumerable<double> razoes)
        {
            var r = razoes.Order().ToList();
            return $"n={r.Count,3} min={r[0]:F3} mediana={r[r.Count / 2]:F3} máx={r[^1]:F3}";
        }

        foreach (var g in validos.GroupBy(c => c.Alvo).OrderByDescending(g => g.Count()))
            Console.WriteLine($"    alvo {g.Key,6}: {Resumo(g.Select(c => (double)c.QuedaHp / c.Dano))}");

        var golpeUnico = validos.Where(c => c.Eventos.Count == 1).ToList();
        Console.WriteLine($"  Intervalos com um golpe só ({golpeUnico.Count}), por jogador (fator igual para todos?):");
        foreach (var g in golpeUnico.GroupBy(c => c.Eventos[0].AutorId).OrderByDescending(g => g.Count()))
            Console.WriteLine($"    autor {g.Key,6}: {Resumo(g.Select(c => (double)c.QuedaHp / c.Dano))}");

        Console.WriteLine($"  Intervalos com um golpe só ({golpeUnico.Count}), por skill:");
        foreach (var g in golpeUnico.GroupBy(c => c.Eventos[0].Skill).OrderByDescending(g => g.Count()))
            Console.WriteLine($"    skill {g.Key,9}: {Resumo(g.Select(c => (double)c.QuedaHp / c.Dano))}" +
                              $"  tipos {string.Join(",", g.Select(c => c.Eventos[0].TipoDano).Distinct())}");

        var hpVistos = conferencias.Select(c => c.Alvo).ToHashSet();
        var alvosSemHp = eventos.Select(e => e.Evento.AlvoId).Distinct().Where(a => !hpVistos.Contains(a)).ToList();
        Console.WriteLine($"  Alvos com dano mas sem nenhum 0x8D00: {alvosSemHp.Count} ({string.Join(", ", alvosSemHp)})");
    }

    public void Relatar(CultureInfo ptBr, int amostrasHex)
    {
        Console.WriteLine();
        Console.WriteLine($"=== Stream {chave}");
        Console.WriteLine($"  TCP: {Montador.BytesEntregues:N0} bytes, retransmissões {Montador.Retransmissoes}, lacunas {Montador.Lacunas}");
        Console.WriteLine($"  Enquadramento: sincronizado={Enquadrador.Sincronizado}, dessincronizações {Enquadrador.Dessincronizacoes}, bytes descartados {Enquadrador.BytesDescartados:N0}");
        Console.WriteLine($"  Pacotes: {pacotesBrutos} no fio, {pacotes} depois de abrir {Desempacotador.BlocosComprimidos} blocos LZ4 " +
                          $"(falhas {Desempacotador.FalhasDescompressao}, tamanho divergente {Desempacotador.TamanhosDivergentes}, bytes sobrando {Desempacotador.BytesSobrando})");
        if (pacotes == 0) return;

        Console.WriteLine("  Opcodes mais frequentes:");
        foreach (var (op, n) in opcodes.OrderByDescending(p => p.Value).Take(25))
            Console.WriteLine($"    0x{op:X4} {Opcodes.Nome(op),-20} {n,6}");

        foreach (var (id, nome) in personagens)
            Console.WriteLine($"  InfoPersonagem: id={id} nome=\"{nome}\"");

        Console.WriteLine($"  Eventos de dano: {eventos.Count} válidos");
        foreach (var (motivo, n) in falhas.OrderByDescending(p => p.Value).Take(10))
            Console.WriteLine($"    descartado {n,5}x  {motivo}");

        foreach (var grupo in eventos.GroupBy(e => e.Evento.AutorId).OrderByDescending(g => g.Sum(e => (double)e.Evento.Dano)))
        {
            var lista = grupo.ToList();
            double total = lista.Sum(e => (double)e.Evento.Dano);
            double segundos = Math.Max(1, (lista[^1].Hora - lista[0].Hora).TotalSeconds);
            int criticos = lista.Count(e => e.Evento.Critico);
            string rotulo = personagens.TryGetValue(grupo.Key, out var nome) ? $" ({nome}, você)" : "";
            Console.WriteLine(string.Format(ptBr,
                "  Autor {0}{1}: {2} golpes, dano {3:N0} em {4:N1} s = {5:N0} DPS, crítico {6:P0}, alvos {7}",
                grupo.Key, rotulo, lista.Count, total, segundos, total / segundos,
                (double)criticos / lista.Count, lista.Select(e => e.Evento.AlvoId).Distinct().Count()));

            foreach (var skill in lista.GroupBy(e => e.Evento.Skill).OrderByDescending(g => g.Sum(e => (double)e.Evento.Dano)).Take(8))
            {
                Console.WriteLine(string.Format(ptBr, "      skill {0,10}: {1,4}x  {2,14:N0}  (média {3:N0}, máx {4:N0}){5}",
                    skill.Key, skill.Count(), skill.Sum(e => (double)e.Evento.Dano),
                    skill.Average(e => (double)e.Evento.Dano), skill.Max(e => e.Evento.Dano),
                    skill.First().Evento.Periodico ? " DoT" : ""));
                // Diagnóstico do crítico: dentro da mesma skill, o tipo 3 deve ter média bem maior que o tipo 2.
                Console.WriteLine("        por tipo_dano: " + string.Join("; ", skill.GroupBy(e => e.Evento.TipoDano).OrderBy(t => t.Key)
                    .Select(t => string.Format(ptBr, "{0}: {1}x média {2:N0}", t.Key, t.Count(), t.Average(e => (double)e.Evento.Dano)))));
            }
        }

        RelatarConferenciaHp();

        foreach (var amostra in amostras.Take(amostrasHex))
            Console.WriteLine($"  hex 0x{opAmostra:X4}: " + Convert.ToHexString(amostra));
    }
}
