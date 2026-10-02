using System.Buffers.Binary;

namespace Aion2Meter.Core.Protocolo;

/// <summary>Cursor little-endian com checagem de limite. Pacote truncado vira FormatException.</summary>
public sealed class LeitorPacote(byte[] dados, int posicao = 0)
{
    public int Posicao { get; private set; } = posicao;
    public int Restante => dados.Length - Posicao;

    public byte LerU8()
    {
        Exigir(1);
        return dados[Posicao++];
    }

    public ushort LerU16()
    {
        Exigir(2);
        var v = BinaryPrimitives.ReadUInt16LittleEndian(dados.AsSpan(Posicao));
        Posicao += 2;
        return v;
    }

    public uint LerU32()
    {
        Exigir(4);
        var v = BinaryPrimitives.ReadUInt32LittleEndian(dados.AsSpan(Posicao));
        Posicao += 4;
        return v;
    }

    public ulong LerVarInt()
    {
        if (!VarInt.TentarLer(dados, Posicao, out ulong v, out int n))
            throw new FormatException($"Varint inválido na posição {Posicao}");
        Posicao += n;
        return v;
    }

    public ReadOnlySpan<byte> LerBytes(int quantidade)
    {
        Exigir(quantidade);
        var trecho = dados.AsSpan(Posicao, quantidade);
        Posicao += quantidade;
        return trecho;
    }

    public void Pular(int quantidade) => LerBytes(quantidade);

    private void Exigir(int n)
    {
        if (Restante < n)
            throw new FormatException($"Pacote truncado: precisa de {n} bytes, restam {Restante} (posição {Posicao})");
    }
}
