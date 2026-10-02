namespace Aion2Meter.Core.Protocolo;

/// <summary>Descompressor de bloco LZ4 cru (sem o cabeçalho do formato "frame").</summary>
public static class Lz4
{
    /// <summary>Retorna quantos bytes foram escritos em <paramref name="destino"/>, ou -1 se o bloco for inválido.</summary>
    public static int Descomprimir(ReadOnlySpan<byte> origem, Span<byte> destino)
    {
        int o = 0, d = 0;
        try
        {
            while (o < origem.Length)
            {
                int token = origem[o++];

                int literais = token >> 4;
                if (literais == 15)
                {
                    byte b;
                    do { b = origem[o++]; literais += b; } while (b == 255);
                }
                origem.Slice(o, literais).CopyTo(destino[d..]);
                o += literais;
                d += literais;

                // A última sequência do bloco só tem literais.
                if (o >= origem.Length) break;

                int distancia = origem[o] | (origem[o + 1] << 8);
                o += 2;
                if (distancia == 0 || distancia > d) return -1;

                int copia = token & 0x0F;
                if (copia == 15)
                {
                    byte b;
                    do { b = origem[o++]; copia += b; } while (b == 255);
                }
                copia += 4;
                if (d + copia > destino.Length) return -1;

                // Byte a byte de propósito: origem e destino da cópia podem se sobrepor.
                int m = d - distancia;
                for (int i = 0; i < copia; i++) destino[d++] = destino[m++];
            }
        }
        catch (IndexOutOfRangeException) { return -1; }
        catch (ArgumentException) { return -1; }
        return d;
    }
}
