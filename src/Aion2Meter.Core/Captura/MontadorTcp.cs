namespace Aion2Meter.Core.Captura;

/// <summary>
/// Remonta uma direção de um stream TCP pelo número de sequência: descarta
/// retransmissões, segura segmentos fora de ordem e, se a lacuna não fechar, avisa
/// que dados se perderam (o enquadrador precisa ressincronizar).
/// </summary>
public sealed class MontadorTcp
{
    private const int PendentesMaximo = 64;

    private readonly Dictionary<uint, byte[]> pendentes = [];
    private uint proximo;
    private bool iniciado;

    public int Retransmissoes { get; private set; }
    public int Lacunas { get; private set; }
    public long BytesEntregues { get; private set; }

    public void Adicionar(uint seq, bool syn, ReadOnlySpan<byte> dados, Action<ReadOnlySpan<byte>> aoDados, Action aoPerder)
    {
        if (syn)
        {
            proximo = seq + 1;
            iniciado = true;
            pendentes.Clear();
            return;
        }
        if (dados.IsEmpty) return;

        if (!iniciado)
        {
            proximo = seq;
            iniciado = true;
        }

        int delta = (int)(seq - proximo);
        if (delta > 0)
        {
            pendentes.TryAdd(seq, dados.ToArray());
            if (pendentes.Count > PendentesMaximo)
            {
                // A lacuna não vai fechar: pula para o segmento pendente mais antigo.
                Lacunas++;
                aoPerder();
                proximo = pendentes.Keys.MinBy(s => (int)(s - proximo));
                Escoar(aoDados);
            }
            return;
        }

        Entregar(dados, delta, aoDados);
        Escoar(aoDados);
    }

    /// <summary>delta &lt;= 0: o segmento começa em ou antes de <see cref="proximo"/>.</summary>
    private void Entregar(ReadOnlySpan<byte> dados, int delta, Action<ReadOnlySpan<byte>> aoDados)
    {
        int novos = dados.Length + delta;
        if (novos <= 0)
        {
            Retransmissoes++;
            return;
        }
        aoDados(dados[(-delta)..]);
        proximo += (uint)novos;
        BytesEntregues += novos;
    }

    private void Escoar(Action<ReadOnlySpan<byte>> aoDados)
    {
        bool avancou;
        do
        {
            avancou = false;
            foreach (var (seq, dados) in pendentes)
            {
                int delta = (int)(seq - proximo);
                if (delta > 0) continue;
                pendentes.Remove(seq);
                Entregar(dados, delta, aoDados);
                avancou = true;
                break;
            }
        } while (avancou);
    }
}
