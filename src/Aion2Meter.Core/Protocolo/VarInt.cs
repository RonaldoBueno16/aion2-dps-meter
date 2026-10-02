namespace Aion2Meter.Core.Protocolo;

/// <summary>Varint estilo protobuf: 7 bits por byte, bit 0x80 indica que há mais bytes.</summary>
public static class VarInt
{
    /// <summary>
    /// Lê um varint a partir de <paramref name="posicao"/>. Retorna false quando os bytes
    /// acabam antes do fim do varint ou quando ele passa de 64 bits.
    /// </summary>
    public static bool TentarLer(ReadOnlySpan<byte> dados, int posicao, out ulong valor, out int tamanho)
    {
        valor = 0;
        tamanho = 0;
        int deslocamento = 0;
        while (posicao + tamanho < dados.Length)
        {
            byte b = dados[posicao + tamanho++];
            valor |= (ulong)(b & 0x7F) << deslocamento;
            if ((b & 0x80) == 0) return true;
            deslocamento += 7;
            if (deslocamento >= 64) return false;
        }
        return false;
    }
}
