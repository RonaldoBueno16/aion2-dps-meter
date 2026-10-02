using System.Buffers.Binary;
using System.Net;

namespace Aion2Meter.Core.Captura;

public readonly record struct SegmentoTcp(
    IPAddress Origem,
    ushort PortaOrigem,
    IPAddress Destino,
    ushort PortaDestino,
    uint Seq,
    bool Syn,
    byte[] Dados)
{
    public string Chave => $"{Origem}:{PortaOrigem} > {Destino}:{PortaDestino}";

    private const int EnlaceEthernet = 1;
    private const int EnlaceIpCru = 101;
    private const int EnlaceIpv4 = 228;

    /// <summary>Extrai um segmento TCP sobre IPv4 de um quadro capturado.</summary>
    public static bool TentarExtrair(ReadOnlySpan<byte> quadro, int tipoEnlace, out SegmentoTcp segmento)
    {
        segmento = default;
        int ip;
        switch (tipoEnlace)
        {
            case EnlaceEthernet:
                if (quadro.Length < 14) return false;
                int etherType = (quadro[12] << 8) | quadro[13];
                ip = 14;
                if (etherType == 0x8100 && quadro.Length >= 18)
                {
                    etherType = (quadro[16] << 8) | quadro[17];
                    ip = 18;
                }
                if (etherType != 0x0800) return false;
                break;
            case EnlaceIpCru:
            case EnlaceIpv4:
                ip = 0;
                break;
            default:
                return false;
        }

        if (quadro.Length < ip + 20 || (quadro[ip] >> 4) != 4 || quadro[ip + 9] != 6) return false;
        int cabecalhoIp = (quadro[ip] & 0x0F) * 4;
        int fimIp = ip + BinaryPrimitives.ReadUInt16BigEndian(quadro[(ip + 2)..]);
        // Placa com coalescência de recepção pode gravar tamanho 0 ou menor que o real.
        if (fimIp <= ip + cabecalhoIp || fimIp > quadro.Length) fimIp = quadro.Length;

        int tcp = ip + cabecalhoIp;
        if (fimIp < tcp + 20) return false;
        int cabecalhoTcp = (quadro[tcp + 12] >> 4) * 4;
        int inicioDados = tcp + cabecalhoTcp;
        if (inicioDados > fimIp) return false;

        segmento = new SegmentoTcp(
            new IPAddress(quadro.Slice(ip + 12, 4)),
            BinaryPrimitives.ReadUInt16BigEndian(quadro[tcp..]),
            new IPAddress(quadro.Slice(ip + 16, 4)),
            BinaryPrimitives.ReadUInt16BigEndian(quadro[(tcp + 2)..]),
            BinaryPrimitives.ReadUInt32BigEndian(quadro[(tcp + 4)..]),
            Syn: (quadro[tcp + 13] & 0x02) != 0,
            quadro[inicioDados..fimIp].ToArray());
        return true;
    }
}
