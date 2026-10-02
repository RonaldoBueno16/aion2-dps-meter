using System.Buffers.Binary;

namespace Aion2Meter.Core.Protocolo;

/// <summary>
/// Abre os pacotes comprimidos e entrega os pacotes de dentro, na ordem.
/// Formato: [varint][flag 0xF? opcional][FF FF][u32 tamanho descomprimido][bloco LZ4].
/// O conteúdo descomprimido é uma sequência de pacotes no mesmo framing, que pode ter
/// outro bloco comprimido dentro.
/// </summary>
public sealed class Desempacotador
{
    private const int ProfundidadeMaxima = 8;
    private const int TamanhoDescomprimidoMaximo = 10_000_000;

    public int BlocosComprimidos { get; private set; }
    public int FalhasDescompressao { get; private set; }
    public int TamanhosDivergentes { get; private set; }
    public long BytesSobrando { get; private set; }

    public void Expandir(byte[] pacote, Action<byte[]> aoPacote) => Abrir(pacote, aoPacote, 0);

    private void Abrir(byte[] buffer, Action<byte[]> aoPacote, int profundidade)
    {
        int pos = 0;
        while (pos < buffer.Length)
        {
            // Bytes 00 entre quadros são preenchimento.
            if (buffer[pos] == 0) { pos++; continue; }

            if (!VarInt.TentarLer(buffer, pos, out ulong valor, out int n) || valor > 2_000_000) break;
            long tamanho = Enquadrador.TamanhoTotal(valor, n);
            if (tamanho <= 0) { pos++; continue; }
            if (pos + tamanho > buffer.Length) break;

            var quadro = buffer.AsSpan(pos, (int)tamanho);
            switch (TentarDescomprimir(quadro, n, out byte[]? interno))
            {
                case Resultado.Comprimido:
                    BlocosComprimidos++;
                    if (profundidade < ProfundidadeMaxima) Abrir(interno!, aoPacote, profundidade + 1);
                    break;
                case Resultado.Falhou:
                    FalhasDescompressao++;
                    break;
                default:
                    aoPacote(quadro.ToArray());
                    break;
            }
            pos += (int)tamanho;
        }
        BytesSobrando += buffer.Length - pos;
    }

    private enum Resultado { NaoComprimido, Comprimido, Falhou }

    private Resultado TentarDescomprimir(ReadOnlySpan<byte> quadro, int bytesVarInt, out byte[]? interno)
    {
        interno = null;
        int cabecalho = bytesVarInt;
        if (cabecalho < quadro.Length && (quadro[cabecalho] & 0xF0) == 0xF0 && quadro[cabecalho] != 0xFF)
            cabecalho++;

        if (quadro.Length < cabecalho + 6 || quadro[cabecalho] != 0xFF || quadro[cabecalho + 1] != 0xFF)
            return Resultado.NaoComprimido;

        int tamanhoReal = BinaryPrimitives.ReadInt32LittleEndian(quadro[(cabecalho + 2)..]);
        if (tamanhoReal <= 0 || tamanhoReal > TamanhoDescomprimidoMaximo) return Resultado.Falhou;

        var saida = new byte[tamanhoReal];
        int escritos = Lz4.Descomprimir(quadro[(cabecalho + 6)..], saida);
        if (escritos <= 0) return Resultado.Falhou;
        if (escritos != tamanhoReal)
        {
            TamanhosDivergentes++;
            Array.Resize(ref saida, escritos);
        }
        interno = saida;
        return Resultado.Comprimido;
    }
}
