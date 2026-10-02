using Aion2Meter.Core.Protocolo;
using K4os.Compression.LZ4;

namespace Aion2Meter.Testes;

/// <summary>Ida e volta contra o compressor de referência (K4os), nos casos que pegam decodificador ingênuo.</summary>
public class Lz4Testes
{
    public static TheoryData<string, byte[]> Entradas()
    {
        var aleatorio = new Random(42);
        byte[] Aleatorios(int n) { var b = new byte[n]; aleatorio.NextBytes(b); return b; }

        var texto = System.Text.Encoding.UTF8.GetBytes(string.Concat(Enumerable.Repeat("Tempest Shot acertou o alvo; ", 2000)));
        var misto = new byte[40_000];
        for (int i = 0; i < misto.Length; i++) misto[i] = (byte)(i % 97 < 60 ? i % 7 : aleatorio.Next(256));

        return new TheoryData<string, byte[]>
        {
            { "aleatório 64 KB (só literais longos)", Aleatorios(65_536) },
            { "zeros 100 KB (cópia sobreposta com distância 1)", new byte[100_000] },
            { "padrão de 3 bytes (sobreposição curta)", Enumerable.Range(0, 50_000).Select(i => (byte)(i % 3)).ToArray() },
            { "texto repetido (matches longos)", texto },
            { "misto", misto },
            { "pequeno", Aleatorios(17) },
        };
    }

    [Theory]
    [MemberData(nameof(Entradas))]
    public void Descomprime_igual_ao_original(string caso, byte[] original)
    {
        foreach (var nivel in new[] { LZ4Level.L00_FAST, LZ4Level.L12_MAX })
        {
            var comprimido = new byte[LZ4Codec.MaximumOutputSize(original.Length)];
            int tamanho = LZ4Codec.Encode(original, comprimido, nivel);

            var saida = new byte[original.Length];
            int escritos = Lz4.Descomprimir(comprimido.AsSpan(0, tamanho), saida);

            Assert.True(escritos == original.Length, $"{caso} ({nivel}): escreveu {escritos} de {original.Length}");
            Assert.True(original.AsSpan().SequenceEqual(saida), $"{caso} ({nivel}): conteúdo diferente");
        }
    }

    [Fact]
    public void Distancia_antes_do_inicio_e_invalida()
    {
        // token 0x10: 1 literal 'A', depois match com distância 5 (só há 1 byte escrito).
        byte[] bloco = [0x10, (byte)'A', 0x05, 0x00];
        Assert.Equal(-1, Lz4.Descomprimir(bloco, new byte[64]));
    }

    [Fact]
    public void Destino_pequeno_e_invalido()
    {
        var original = new byte[1000];
        var comprimido = new byte[LZ4Codec.MaximumOutputSize(original.Length)];
        int tamanho = LZ4Codec.Encode(original, comprimido);
        Assert.Equal(-1, Lz4.Descomprimir(comprimido.AsSpan(0, tamanho), new byte[10]));
    }
}
