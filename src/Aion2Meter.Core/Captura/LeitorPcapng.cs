using System.Buffers.Binary;

namespace Aion2Meter.Core.Captura;

public readonly record struct QuadroCapturado(DateTime Hora, int TipoEnlace, byte[] Dados);

/// <summary>Leitor mínimo de pcapng little-endian (formato gerado pelo pktmon etl2pcap).</summary>
public static class LeitorPcapng
{
    private const uint BlocoSecao = 0x0A0D0D0A;
    private const uint BlocoInterface = 1;
    private const uint BlocoPacoteSimples = 3;
    private const uint BlocoPacoteAvancado = 6;
    private const uint MagicoLittleEndian = 0x1A2B3C4D;
    private const ushort OpcaoResolucaoTempo = 9;

    public static IEnumerable<QuadroCapturado> Ler(string caminho)
    {
        using var leitor = new BinaryReader(File.OpenRead(caminho));
        var interfaces = new List<(int Enlace, double SegundosPorUnidade)>();
        long tamanhoArquivo = leitor.BaseStream.Length;

        while (leitor.BaseStream.Position + 12 <= tamanhoArquivo)
        {
            uint tipo = leitor.ReadUInt32();
            uint total = leitor.ReadUInt32();
            if (total < 12 || leitor.BaseStream.Position - 8 + total > tamanhoArquivo)
                throw new InvalidDataException($"Bloco pcapng inválido na posição {leitor.BaseStream.Position - 8}");
            byte[] corpo = leitor.ReadBytes((int)total - 12);
            leitor.ReadUInt32(); // tamanho repetido no fim do bloco

            switch (tipo)
            {
                case BlocoSecao:
                    if (BinaryPrimitives.ReadUInt32LittleEndian(corpo) != MagicoLittleEndian)
                        throw new InvalidDataException("pcapng big-endian não suportado");
                    interfaces.Clear();
                    break;

                case BlocoInterface:
                    interfaces.Add((BinaryPrimitives.ReadUInt16LittleEndian(corpo), LerResolucao(corpo.AsSpan(8))));
                    break;

                case BlocoPacoteAvancado:
                {
                    int iface = (int)BinaryPrimitives.ReadUInt32LittleEndian(corpo);
                    ulong unidades = ((ulong)BinaryPrimitives.ReadUInt32LittleEndian(corpo.AsSpan(4)) << 32)
                                     | BinaryPrimitives.ReadUInt32LittleEndian(corpo.AsSpan(8));
                    int capturado = (int)BinaryPrimitives.ReadUInt32LittleEndian(corpo.AsSpan(12));
                    var (enlace, resolucao) = iface < interfaces.Count ? interfaces[iface] : (1, 1e-6);
                    var hora = DateTime.UnixEpoch.AddTicks((long)(unidades * resolucao * TimeSpan.TicksPerSecond));
                    yield return new QuadroCapturado(hora, enlace, corpo.AsSpan(20, capturado).ToArray());
                    break;
                }

                case BlocoPacoteSimples:
                {
                    int enlace = interfaces.Count > 0 ? interfaces[0].Enlace : 1;
                    yield return new QuadroCapturado(DateTime.MinValue, enlace, corpo.AsSpan(4).ToArray());
                    break;
                }
            }
        }
    }

    /// <summary>Opção if_tsresol: bit alto 0 = 10^-n segundos, bit alto 1 = 2^-n. Padrão: microssegundo.</summary>
    private static double LerResolucao(ReadOnlySpan<byte> opcoes)
    {
        int pos = 0;
        while (pos + 4 <= opcoes.Length)
        {
            ushort codigo = BinaryPrimitives.ReadUInt16LittleEndian(opcoes[pos..]);
            ushort tamanho = BinaryPrimitives.ReadUInt16LittleEndian(opcoes[(pos + 2)..]);
            if (codigo == 0) break;
            if (codigo == OpcaoResolucaoTempo && tamanho >= 1 && pos + 4 < opcoes.Length)
            {
                byte v = opcoes[pos + 4];
                return (v & 0x80) == 0 ? Math.Pow(10, -v) : Math.Pow(2, -(v & 0x7F));
            }
            pos += 4 + ((tamanho + 3) & ~3);
        }
        return 1e-6;
    }
}
