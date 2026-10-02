using Aion2Meter.Core.Captura;
using Aion2Meter.Core.Protocolo;
using K4os.Compression.LZ4;

namespace Aion2Meter.Testes;

public class EnquadramentoTestes
{
    // Heartbeat de 11 bytes (0x0E + 1 - 4) e um 0x3804 real da captura de 2026-10-01.
    private static readonly byte[] Heartbeat = Convert.FromHexString("0E0036AABBCCDDEEFF0011");
    private static readonly byte[] Dano = Convert.FromHexString("210438CBE7020400A657575FE1003002073E095801000000AC57E8010100");

    [Fact]
    public void Corta_stream_entregue_byte_a_byte_e_descarta_lixo_antes_do_heartbeat()
    {
        byte[] stream = [.. Convert.FromHexString("123456"), .. Heartbeat, .. Dano, .. Heartbeat];
        var enquadrador = new Enquadrador();
        var pacotes = new List<byte[]>();

        foreach (byte b in stream) enquadrador.Adicionar([b], pacotes.Add);

        Assert.Equal(3, pacotes.Count);
        Assert.Equal(Heartbeat, pacotes[0]);
        Assert.Equal(Dano, pacotes[1]);
        Assert.Equal(3, enquadrador.BytesDescartados);
    }

    [Fact]
    public void Abre_bloco_comprimido_com_pacotes_dentro()
    {
        byte[] interno = [.. Dano, 0x00, 0x00, .. Heartbeat];
        var lz4 = new byte[LZ4Codec.MaximumOutputSize(interno.Length)];
        int n = LZ4Codec.Encode(interno, lz4);

        // [varint tamanho][FF FF][u32 tamanho descomprimido][LZ4]; tamanho total = varint + 1 - 4
        byte[] corpo = [0xFF, 0xFF, .. BitConverter.GetBytes(interno.Length), .. lz4.AsSpan(0, n)];
        int total = 1 + corpo.Length;
        byte[] pacote = [(byte)(total + 3), .. corpo];
        Assert.True(total + 3 < 0x80, "o teste assume varint de 1 byte");

        var desempacotador = new Desempacotador();
        var saida = new List<byte[]>();
        desempacotador.Expandir(pacote, saida.Add);

        Assert.Equal(1, desempacotador.BlocosComprimidos);
        Assert.Equal(0, desempacotador.TamanhosDivergentes);
        Assert.Equal(2, saida.Count);
        Assert.Equal(Dano, saida[0]);
        Assert.Equal(Heartbeat, saida[1]);
    }

    [Fact]
    public void Remonta_tcp_fora_de_ordem_e_ignora_retransmissao()
    {
        var montador = new MontadorTcp();
        var recebido = new List<byte>();
        int perdas = 0;
        void Enviar(uint seq, string texto) =>
            montador.Adicionar(seq, false, System.Text.Encoding.ASCII.GetBytes(texto), d => recebido.AddRange(d.ToArray()), () => perdas++);

        Enviar(1000, "abc");
        Enviar(1006, "ghi");   // chega antes de "def"
        Enviar(1003, "def");
        Enviar(1000, "abc");   // retransmissão
        Enviar(1007, "hijk");  // sobreposição parcial: só "jk" é novo

        Assert.Equal("abcdefghijk", System.Text.Encoding.ASCII.GetString(recebido.ToArray()));
        Assert.Equal(0, perdas);
        Assert.Equal(1, montador.Retransmissoes);
    }
}
