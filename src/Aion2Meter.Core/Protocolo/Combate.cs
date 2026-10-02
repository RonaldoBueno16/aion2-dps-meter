using System.Text;

namespace Aion2Meter.Core.Protocolo;

public readonly record struct EventoDano(
    uint AlvoId,
    uint AutorId,
    uint Skill,
    ulong Dano,
    int TipoDano,
    bool Critico,
    bool Costas,
    bool Frente,
    bool Aparo,
    bool Perfeito,
    bool Duplo,
    bool Periodico);

/// <summary>Leitura dos pacotes de combate. Layout descrito em PROTOCOLO.md, seção 3.</summary>
public static class Combate
{
    private const int TipoCritico = 3;

    /// <summary>Dano direto, opcode 0x3804.</summary>
    public static bool TentarDano(byte[] pacote, out EventoDano evento, out string? motivo)
    {
        evento = default;
        motivo = null;
        try
        {
            var r = AbrirCorpo(pacote);
            uint alvo = (uint)r.LerVarInt();

            ulong seletor = r.LerVarInt();
            int variante = (int)(seletor & 0x0F);
            if (seletor > 255 || variante < 4 || variante > 7) { motivo = $"seletor {seletor}"; return false; }

            r.LerVarInt(); // desconhecido
            // autor == alvo vale: cura em si mesmo chega assim (quem decide é o Medidor).
            uint autor = (uint)r.LerVarInt();

            uint skill = r.LerU32();
            r.LerU8(); // desconhecido
            int tipo = (int)r.LerVarInt();

            byte flags = 0, direcao = 0;
            if (variante != 4)
            {
                flags = r.LerU8();
                r.LerU8(); // desconhecido
                direcao = r.LerU8();
            }
            r.Pular(8); // desconhecido

            r.LerVarInt(); // desconhecido
            ulong dano = r.LerVarInt();

            evento = new EventoDano(alvo, autor, skill, dano, tipo,
                Critico: tipo == TipoCritico,
                Costas: (direcao & 0x01) != 0,
                Frente: (direcao & 0x02) != 0,
                Aparo: (flags & 0x02) != 0,
                Perfeito: (flags & 0x04) != 0,
                Duplo: (flags & 0x08) != 0,
                Periodico: false);
            return true;
        }
        catch (FormatException ex)
        {
            motivo = ex.Message;
            return false;
        }
    }

    /// <summary>Dano periódico (DoT), opcode 0x3805.</summary>
    public static bool TentarDanoPeriodico(byte[] pacote, out EventoDano evento, out string? motivo)
    {
        evento = default;
        motivo = null;
        try
        {
            var r = AbrirCorpo(pacote);
            uint alvo = (uint)r.LerVarInt();
            byte efeito = r.LerU8();
            if ((efeito & 0x02) == 0) { motivo = $"efeito 0x{efeito:X2} sem bit de dano"; return false; }

            uint autor = (uint)r.LerVarInt();
            r.LerVarInt(); // desconhecido
            uint skill = r.LerU32() / 100;
            ulong dano = r.LerVarInt();

            evento = new EventoDano(alvo, autor, skill, dano, 0,
                false, false, false, false, false, false, Periodico: true);
            return true;
        }
        catch (FormatException ex)
        {
            motivo = ex.Message;
            return false;
        }
    }

    /// <summary>HP atual de uma entidade, opcode 0x8D00.</summary>
    public static bool TentarHpRestante(byte[] pacote, out uint entidadeId, out ulong hp)
    {
        entidadeId = 0;
        hp = 0;
        try
        {
            var r = AbrirCorpo(pacote);
            entidadeId = (uint)r.LerVarInt();
            r.LerVarInt(); // desconhecido
            r.LerVarInt(); // desconhecido
            r.LerVarInt(); // desconhecido
            hp = ((ulong)r.LerU32()) | ((ulong)r.LerU32() << 32);
            return true;
        }
        catch (FormatException)
        {
            return false;
        }
    }

    /// <summary>
    /// Outro jogador entrando no campo de visão, opcode 0x3645. Cabeçalho como em
    /// <see cref="TentarCabecalhoJogador"/>; o level fica ~1.000 bytes adiante, depois do
    /// equipamento, no bloco [u16 servidor][u8 n][n × (u16 tag, u32 valor)][u32 level][u32 0][u32 power].
    /// O u32 logo depois do nome NÃO é o level (deu 32 no Dacura, que é 45, e 12 na Vallaina, que é 32).
    /// Power conferido no jogo: Nxhunter 909 (level 43) e LaReini 579 (level 31).
    /// </summary>
    public static bool TentarInfoJogador(byte[] pacote, out uint entidadeId, out string nome, out int nivel, out int poder)
    {
        bool ok = TentarCabecalhoJogador(pacote, out entidadeId, out nome, out int fimDoNome);
        (nivel, poder) = fimDoNome > 0 ? ProcurarNivelEPoder(pacote, fimDoNome) : (0, 0);
        return ok;
    }

    /// <summary>
    /// O seu personagem, opcode 0x3633 (chega no login; não veio no teleporte). Depois do nome:
    /// [u16 servidor][u32 desconhecido][u8 desconhecido][u32 level][u32 power]. Conferido uma vez
    /// (level 31 e power 355 no login de 2026-10-02; o power subiu para 361 pelo 0x561C).
    /// </summary>
    public static bool TentarInfoPersonagem(byte[] pacote, out uint entidadeId, out string nome, out int nivel, out int poder)
    {
        nivel = poder = 0;
        bool ok = TentarCabecalhoJogador(pacote, out entidadeId, out nome, out int fimDoNome);
        if (fimDoNome > 0 && fimDoNome + 15 <= pacote.Length)
        {
            nivel = NivelValido(BitConverter.ToUInt32(pacote, fimDoNome + 7));
            poder = (int)Math.Min(BitConverter.ToUInt32(pacote, fimDoNome + 11), int.MaxValue);
        }
        return ok;
    }

    /// <summary>
    /// Power mudou, opcode 0x561C: [varint entidade][u32 power]... Visto uma vez, com o seu
    /// personagem (355 → 361 junto com o 0x561D, que traz o mesmo valor duas vezes sem a entidade).
    /// </summary>
    public static bool TentarPoder(byte[] pacote, out uint entidadeId, out int poder)
    {
        entidadeId = 0;
        poder = 0;
        try
        {
            var r = AbrirCorpo(pacote);
            entidadeId = (uint)r.LerVarInt();
            poder = (int)Math.Min(r.LerU32(), int.MaxValue);
            return entidadeId > 0 && poder > 0;
        }
        catch (FormatException)
        {
            return false;
        }
    }

    /// <summary>
    /// [varint entidade][4 bytes][u8 flags (bit 0 = tem nome)][varint tamanho][nome UTF-8].
    /// fimDoNome = posição logo depois do nome, ou 0 sem nome.
    /// </summary>
    private static bool TentarCabecalhoJogador(byte[] pacote, out uint entidadeId, out string nome, out int fimDoNome)
    {
        entidadeId = 0;
        nome = "";
        fimDoNome = 0;
        try
        {
            var r = AbrirCorpo(pacote);
            entidadeId = (uint)r.LerVarInt();
            r.Pular(4); // desconhecido
            if ((r.LerU8() & 0x01) == 0) return entidadeId > 0;

            int tamanho = (int)r.LerVarInt();
            if (tamanho < 1 || tamanho > 72) return entidadeId > 0;
            var bruto = r.LerBytes(tamanho);
            nome = new string(Encoding.UTF8.GetString(bruto).Where(c => !char.IsControl(c)).ToArray());
            fimDoNome = r.Posicao;
            return entidadeId > 0;
        }
        catch (FormatException)
        {
            return entidadeId > 0;
        }
    }

    /// <summary>
    /// Varredura pelo bloco [u16 servidor 1000..9999][u8 n 1..8][n × (u16 tag 1..0xFFF, u32)][u32 level 1..99],
    /// com o power 8 bytes depois do level. Achou um, e só um, em cada um dos 81 pacotes 0x3645 de
    /// 2026-10-02; a 1ª tag varia (0xCD ou 0xCE).
    /// </summary>
    private static (int Nivel, int Poder) ProcurarNivelEPoder(byte[] p, int desde)
    {
        for (int i = desde; i + 3 <= p.Length; i++)
        {
            ushort servidor = BitConverter.ToUInt16(p, i);
            int n = p[i + 2];
            if (servidor is < 1000 or > 9999 || n is < 1 or > 8) continue;
            int fim = i + 3 + n * 6;
            if (fim + 4 > p.Length) continue;

            bool tagsValidas = true;
            for (int k = 0; k < n && tagsValidas; k++)
                tagsValidas = BitConverter.ToUInt16(p, i + 3 + k * 6) is > 0 and < 0x1000;
            if (!tagsValidas) continue;

            int nivel = NivelValido(BitConverter.ToUInt32(p, fim));
            if (nivel == 0) continue;
            int poder = fim + 12 <= p.Length ? (int)Math.Min(BitConverter.ToUInt32(p, fim + 8), int.MaxValue) : 0;
            return (nivel, poder);
        }
        return (0, 0);
    }

    private static int NivelValido(uint valor) => valor is >= 1 and <= 99 ? (int)valor : 0;

    /// <summary>
    /// Morte de entidade, opcode 0x8D04:
    /// [varint morto][u32 skill que matou][varint matador][u16 servidor][u8 tam][nome do matador][u8 tam][legião]...
    /// Sem matador (invocação que expirou) vem tudo zerado.
    /// </summary>
    public static bool TentarMorte(byte[] pacote, out uint morto, out uint matador, out uint skill, out string nomeMatador)
    {
        morto = matador = skill = 0;
        nomeMatador = "";
        try
        {
            var r = AbrirCorpo(pacote);
            morto = (uint)r.LerVarInt();
            skill = r.LerU32();
            matador = (uint)r.LerVarInt();
            if (matador != 0 && r.Restante >= 3)
            {
                r.Pular(2); // servidor
                int tamanho = r.LerU8();
                if (tamanho is >= 1 and <= 72 && r.Restante >= tamanho)
                    nomeMatador = new string(Encoding.UTF8.GetString(r.LerBytes(tamanho)).Where(c => !char.IsControl(c)).ToArray());
            }
            return morto != 0;
        }
        catch (FormatException)
        {
            return morto != 0;
        }
    }

    private const byte TipoInvocacao = 0x5F;

    /// <summary>
    /// Spawn de invocação, pet ou armadilha (0x3641 com tipo 0x5F no byte baixo da máscara).
    /// A invocação dá dano com id próprio; o dono vem no bloco
    /// [u32 dono][u32 legião][u16 0][u16 servidor][u8 tamanho][nome da legião UTF-8],
    /// achado por varredura com todas as validações juntas (formato documentado pelo
    /// A2Tools em docs/summon-attribution.md). O nome logo depois da máscara é o do dono.
    /// Sem legião o bloco vem zerado e só sobra o nome do dono.
    /// </summary>
    public static bool TentarSpawnInvocacao(byte[] pacote, out uint entidadeId, out uint donoId, out string nomeDono)
    {
        entidadeId = 0;
        donoId = 0;
        nomeDono = "";
        try
        {
            var r = AbrirCorpo(pacote);
            entidadeId = (uint)r.LerVarInt();
            ushort mascara = r.LerU16();
            if ((mascara & 0xFF) != TipoInvocacao) return false;

            if ((r.LerU8() & 0x01) != 0)
            {
                int tamanho = (int)r.LerVarInt();
                if (tamanho is >= 1 and <= 72)
                    nomeDono = new string(Encoding.UTF8.GetString(r.LerBytes(tamanho)).Where(c => !char.IsControl(c)).ToArray());
            }

            donoId = ProcurarBlocoDono(pacote, r.Posicao, entidadeId);
            return donoId != 0 || nomeDono.Length > 0;
        }
        catch (FormatException)
        {
            return false;
        }
    }

    private static readonly UTF8Encoding Utf8Estrito = new(false, throwOnInvalidBytes: true);

    private static uint ProcurarBlocoDono(byte[] p, int desde, uint propriaEntidade)
    {
        for (int i = desde; i + 13 <= p.Length; i++)
        {
            uint dono = BitConverter.ToUInt32(p, i);
            if (dono == 0 || dono >= 1u << 24 || dono == propriaEntidade) continue;
            if (BitConverter.ToUInt16(p, i + 8) != 0) continue;
            ushort servidor = BitConverter.ToUInt16(p, i + 10);
            if (servidor is < 1000 or > 9999) continue;
            int tamanho = p[i + 12];
            if (tamanho is < 1 or > 48 || i + 13 + tamanho > p.Length) continue;
            try
            {
                string legiao = Utf8Estrito.GetString(p, i + 13, tamanho);
                if (legiao.Any(char.IsControl)) continue;
            }
            catch (DecoderFallbackException) { continue; }
            return dono;
        }
        return 0;
    }

    /// <summary>Posiciona o leitor depois do varint de tamanho e do opcode.</summary>
    private static LeitorPacote AbrirCorpo(byte[] pacote)
    {
        var r = new LeitorPacote(pacote);
        r.LerVarInt();
        r.LerU16();
        return r;
    }
}
