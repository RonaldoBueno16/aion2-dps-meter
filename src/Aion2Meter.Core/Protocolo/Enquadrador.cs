namespace Aion2Meter.Core.Protocolo;

/// <summary>
/// Corta um stream TCP já remontado em pacotes do jogo. Começa dessincronizado e só
/// confia no varint de tamanho depois de achar o heartbeat <c>0E 00 36</c>, porque a
/// captura pode começar no meio de um pacote.
/// </summary>
public sealed class Enquadrador
{
    private static readonly byte[] PadraoHeartbeat = [0x0E, 0x00, 0x36];
    private const int TamanhoMaximoPacote = 40 * 1024;

    private byte[] buffer = new byte[64 * 1024];
    private int ocupado;

    public bool Sincronizado { get; private set; }
    public int Dessincronizacoes { get; private set; }
    public long BytesDescartados { get; private set; }

    /// <summary>tamanho total do pacote = valor do varint + bytes do varint - 4</summary>
    public static long TamanhoTotal(ulong valorVarInt, int bytesVarInt) => (long)valorVarInt + bytesVarInt - 4;

    public void Adicionar(ReadOnlySpan<byte> dados, Action<byte[]> aoExtrair)
    {
        if (ocupado + dados.Length > buffer.Length)
            Array.Resize(ref buffer, Math.Max(buffer.Length * 2, ocupado + dados.Length));
        dados.CopyTo(buffer.AsSpan(ocupado));
        ocupado += dados.Length;
        Processar(aoExtrair);
    }

    /// <summary>Conexão nova (SYN): o primeiro byte que vier é o começo de um pacote.</summary>
    public void IniciarConexao()
    {
        ocupado = 0;
        Sincronizado = true;
    }

    /// <summary>Chamar quando o TCP perde dados: o que está no buffer não emenda com o que vem.</summary>
    public void Reiniciar()
    {
        BytesDescartados += ocupado;
        ocupado = 0;
        Sincronizado = false;
    }

    private void Processar(Action<byte[]> aoExtrair)
    {
        int inicio = 0;
        while (inicio < ocupado)
        {
            var janela = buffer.AsSpan(inicio, ocupado - inicio);

            if (!Sincronizado)
            {
                int indice = janela.IndexOf(PadraoHeartbeat);
                if (indice < 0)
                {
                    // Guarda os 2 últimos bytes: o padrão pode estar cortado entre dois segmentos.
                    int descartar = Math.Max(0, janela.Length - 2);
                    inicio += descartar;
                    BytesDescartados += descartar;
                    break;
                }
                inicio += indice;
                BytesDescartados += indice;
                Sincronizado = true;
                continue;
            }

            if (!VarInt.TentarLer(janela, 0, out ulong valor, out int n))
            {
                if (janela.Length >= 10) PerderSincronia(ref inicio);
                break;
            }

            long tamanho = TamanhoTotal(valor, n);
            if (tamanho <= 0 || tamanho > TamanhoMaximoPacote)
            {
                PerderSincronia(ref inicio);
                continue;
            }
            if (janela.Length < tamanho) break;

            aoExtrair(janela[..(int)tamanho].ToArray());
            inicio += (int)tamanho;
        }

        if (inicio > 0)
        {
            Buffer.BlockCopy(buffer, inicio, buffer, 0, ocupado - inicio);
            ocupado -= inicio;
        }
    }

    private void PerderSincronia(ref int inicio)
    {
        Sincronizado = false;
        Dessincronizacoes++;
        inicio++;
        BytesDescartados++;
    }
}
