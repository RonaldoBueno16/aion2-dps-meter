namespace Aion2Meter.Core.Protocolo;

/// <summary>
/// Opcodes do cliente Global, lidos em little-endian (bytes no fio: 04 38 = 0x3804).
/// Origem: RATmeter, commit de 2026-10-01 07:33 UTC. Revalidar a cada patch do jogo.
/// </summary>
public static class Opcodes
{
    public const ushort Heartbeat = 0x3600;
    public const ushort HoraServidor = 0x3603;
    public const ushort InfoPersonagem = 0x3633;
    public const ushort VinculoSessao = 0x3620;
    public const ushort SpawnMob = 0x3641;
    public const ushort InfoOutrosJogadores = 0x3645;
    public const ushort AtributosJogador = 0x3649;
    public const ushort Dano = 0x3804;
    public const ushort DanoPeriodico = 0x3805;
    public const ushort BuffA = 0x382A;
    public const ushort BuffB = 0x382B;
    public const ushort CooldownSkill = 0x3847;
    public const ushort HpRestante = 0x8D00;
    public const ushort MorteEntidade = 0x8D04;
    public const ushort Grupo = 0x9702;
    public const ushort PoderJogador = 0x561C;
    public const ushort Comprimido = 0xFFFF;

    public static string Nome(ushort opcode) => opcode switch
    {
        Heartbeat => "Heartbeat",
        HoraServidor => "HoraServidor",
        InfoPersonagem => "InfoPersonagem",
        VinculoSessao => "VinculoSessao",
        SpawnMob => "SpawnMob",
        InfoOutrosJogadores => "InfoOutrosJogadores",
        AtributosJogador => "AtributosJogador",
        Dano => "Dano",
        DanoPeriodico => "DanoPeriodico",
        BuffA => "BuffA",
        BuffB => "BuffB",
        CooldownSkill => "CooldownSkill",
        HpRestante => "HpRestante",
        MorteEntidade => "MorteEntidade",
        Grupo => "Grupo",
        PoderJogador => "PoderJogador",
        Comprimido => "Comprimido",
        _ => "",
    };

    /// <summary>Lê o opcode logo depois do varint de tamanho.</summary>
    public static bool TentarLer(ReadOnlySpan<byte> pacote, out ushort opcode)
    {
        opcode = 0;
        if (!VarInt.TentarLer(pacote, 0, out _, out int n) || pacote.Length < n + 2) return false;
        opcode = (ushort)(pacote[n] | (pacote[n + 1] << 8));
        return true;
    }
}
