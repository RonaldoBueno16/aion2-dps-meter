using Aion2Meter.Core.Captura;
using Aion2Meter.Core.Protocolo;

namespace Aion2Meter.Core.Medicao;

/// <summary>
/// Pipeline completo: segmento TCP → escolha do fluxo do servidor → remontagem →
/// enquadramento → descompressão → eventos no <see cref="Medidor"/>.
/// Não é thread-safe: alimentar de uma thread só (ou sob lock).
/// </summary>
public sealed class Sessao
{
    private static readonly byte[] PadraoHeartbeat = [0x0E, 0x00, 0x36];
    private const int AcertosParaTravar = 3;
    private static readonly TimeSpan SilencioParaTrocar = TimeSpan.FromSeconds(5);

    // O começo de uma conexão nova (seu personagem, jogadores por perto) chega antes de ela ser
    // reconhecida como a do jogo: os segmentos dos outros fluxos ficam guardados para reprocessar.
    private const int BytesGuardadosPorFluxo = 4 * 1024 * 1024;
    private static readonly TimeSpan EsquecerFluxoParado = TimeSpan.FromSeconds(30);

    private sealed class Guardado
    {
        public readonly List<(SegmentoTcp Seg, DateTime Hora)> Segmentos = [];
        public int Bytes;
        public bool Cheio;
        public bool ComecouComSyn;
        public DateTime Ultimo;
    }

    private readonly Dictionary<string, (int Acertos, DateTime Ultimo)> candidatos = [];
    private readonly Dictionary<string, Guardado> guardados = [];
    private readonly Desempacotador desempacotador = new();
    private MontadorTcp montador = new();
    private Enquadrador enquadrador = new();
    private DateTime horaAtual;
    private DateTime ultimaFaxina;
    private ushort portaServidor;

    public Medidor Medidor { get; } = new();

    /// <summary>Fluxo servidor → cliente em uso, no formato "ip:porta > ip:porta".</summary>
    public string? Fluxo { get; private set; }

    public long Pacotes { get; private set; }
    public int Lacunas => montador.Lacunas;
    public int Dessincronizacoes => enquadrador.Dessincronizacoes;

    /// <summary>Cada pacote do jogo já aberto, para diagnóstico.</summary>
    public event Action<byte[], DateTime>? PacoteDecodificado;

    public void AoSegmento(SegmentoTcp seg, DateTime hora)
    {
        if (seg.Chave != Fluxo) Guardar(seg, hora);
        if (DetectarFluxo(seg, hora)) return; // trocou: este segmento já entrou no reprocessamento
        if (seg.Chave == Fluxo) Processar(seg, hora);
    }

    private void Processar(SegmentoTcp seg, DateTime hora)
    {
        horaAtual = hora;
        // Depois do SYN o stream começa na fronteira de um pacote: não precisa esperar o heartbeat.
        if (seg.Syn) enquadrador.IniciarConexao();
        montador.Adicionar(seg.Seq, seg.Syn, seg.Dados, d => enquadrador.Adicionar(d, AoBruto), enquadrador.Reiniciar);
    }

    /// <summary>
    /// O servidor de jogo manda o heartbeat 0E 00 36 cerca de 20 vezes por segundo. Trava no
    /// fluxo com 3 heartbeats. Troca na hora quando o candidato é uma conexão aberta (SYN) depois
    /// que o medidor começou e na mesma porta de servidor do fluxo atual (troca de servidor: as duas
    /// conexões vistas usavam 13328). Qualquer outro candidato só troca com 5 s de silêncio do atual,
    /// senão um programa qualquer com 0E 00 36 nos dados zeraria a luta.
    /// </summary>
    private bool DetectarFluxo(SegmentoTcp seg, DateTime hora)
    {
        if (seg.Dados.AsSpan().IndexOf(PadraoHeartbeat) < 0) return false;

        var c = candidatos.GetValueOrDefault(seg.Chave);
        candidatos[seg.Chave] = (c.Acertos + 1, hora);
        if (c.Acertos + 1 < AcertosParaTravar || seg.Chave == Fluxo) return false;

        bool conexaoNova = guardados.GetValueOrDefault(seg.Chave)?.ComecouComSyn == true && seg.PortaOrigem == portaServidor;
        if (Fluxo is not null && !conexaoNova && hora - candidatos[Fluxo].Ultimo <= SilencioParaTrocar) return false;

        portaServidor = seg.PortaOrigem;
        TrocarPara(seg.Chave);
        return true;
    }

    private void TrocarPara(string chave)
    {
        Fluxo = chave;
        montador = new MontadorTcp();
        enquadrador = new Enquadrador();
        // Ids de entidade valem só dentro da conexão: o mesmo personagem volta com outro id.
        Medidor.NovaConexao();

        if (!guardados.Remove(chave, out var g)) return;
        foreach (var (seg, hora) in g.Segmentos) Processar(seg, hora);
    }

    private void Guardar(SegmentoTcp seg, DateTime hora)
    {
        // HTTPS e HTTP nunca são o jogo; sem isso um download enche a memória.
        if (seg.PortaOrigem is 443 or 80 || seg.PortaDestino is 443 or 80) return;

        if (!guardados.TryGetValue(seg.Chave, out var g))
            guardados[seg.Chave] = g = new Guardado { ComecouComSyn = seg.Syn };
        g.Ultimo = hora;
        if (!g.Cheio)
        {
            if (g.Bytes + seg.Dados.Length > BytesGuardadosPorFluxo)
            {
                g.Cheio = true;
            }
            else
            {
                g.Segmentos.Add((seg, hora));
                g.Bytes += seg.Dados.Length;
            }
        }

        if (hora - ultimaFaxina < TimeSpan.FromSeconds(5)) return;
        ultimaFaxina = hora;
        foreach (var parado in guardados.Where(p => hora - p.Value.Ultimo > EsquecerFluxoParado).Select(p => p.Key).ToList())
            guardados.Remove(parado);
    }

    private void AoBruto(byte[] bruto) => desempacotador.Expandir(bruto, AoPacote);

    private void AoPacote(byte[] pacote)
    {
        Pacotes++;
        PacoteDecodificado?.Invoke(pacote, horaAtual);
        if (!Opcodes.TentarLer(pacote, out ushort op)) return;

        switch (op)
        {
            case Opcodes.Dano:
                if (Combate.TentarDano(pacote, out var dano, out _)) Medidor.Registrar(dano, horaAtual);
                break;
            case Opcodes.DanoPeriodico:
                if (Combate.TentarDanoPeriodico(pacote, out var dot, out _)) Medidor.Registrar(dot, horaAtual);
                break;
            case Opcodes.InfoPersonagem:
                if (Combate.TentarInfoPersonagem(pacote, out uint eu, out string meuNome, out int meuNivel, out int meuPoder))
                {
                    Medidor.DefinirJogador(eu, meuNome, meuNivel, voce: true);
                    Medidor.DefinirPoder(eu, meuPoder);
                }
                break;
            case Opcodes.SpawnMob:
                if (Combate.TentarSpawnInvocacao(pacote, out uint invocacao, out uint dono, out string nomeDono))
                    Medidor.DefinirInvocacao(invocacao, dono, nomeDono);
                else if (invocacao != 0)
                    Medidor.EsquecerInvocacao(invocacao);
                break;
            case Opcodes.MorteEntidade:
                if (Combate.TentarMorte(pacote, out uint morto, out uint matador, out uint skillMorte, out string nomeMatador))
                    Medidor.RegistrarMorte(morto, matador, skillMorte, nomeMatador, horaAtual);
                break;
            case Opcodes.PoderJogador:
                if (Combate.TentarPoder(pacote, out uint quem, out int poder)) Medidor.DefinirPoder(quem, poder);
                break;
            case Opcodes.InfoOutrosJogadores:
                if (Combate.TentarInfoJogador(pacote, out uint id, out string nome, out int nivel, out int poderDele))
                {
                    Medidor.DefinirJogador(id, nome, nivel, voce: false);
                    Medidor.DefinirPoder(id, poderDele);
                }
                break;
        }
    }
}
