using Aion2Meter.Core.Protocolo;

namespace Aion2Meter.Testes;

/// <summary>Pacotes reais da captura de 2026-10-01 (cliente Global, depois do patch das 19:10 UTC).</summary>
public class ParsersTestes
{
    private static byte[] Hex(string h) => Convert.FromHexString(h);

    [Fact]
    public void Dano_seletor_4()
    {
        Assert.True(Combate.TentarDano(Hex("210438CBE7020400A657575FE1003002073E095801000000AC57E8010100"), out var e, out _));
        Assert.Equal(46027u, e.AlvoId);
        Assert.Equal(11174u, e.AutorId);
        Assert.Equal(14770007u, e.Skill);
        Assert.Equal(232ul, e.Dano);
        Assert.False(e.Critico);
    }

    [Fact]
    public void Dano_seletor_6_com_direcao()
    {
        Assert.True(Combate.TentarDano(Hex("240438CBE7021600A657304DD7003102800002CB261A5401000000AC57EC050100"), out var e, out _));
        Assert.Equal(14110000u, e.Skill);
        Assert.Equal(748ul, e.Dano);
        Assert.True(e.Frente);
        Assert.False(e.Costas);
    }

    [Fact]
    public void Golpe_sem_dano_seletor_0_e_descartado()
    {
        Assert.False(Combate.TentarDano(Hex("1F0438CBE7020000A657509BD7002E024CAB385401000000AC570100"), out _, out var motivo));
        Assert.Contains("seletor", motivo);
    }

    [Fact]
    public void Hp_de_mob()
    {
        Assert.True(Combate.TentarHpRestante(Hex("14008DCBE7020201004E9A010000000000"), out uint id, out ulong hp));
        Assert.Equal(46027u, id);
        Assert.Equal(105038ul, hp);
    }

    [Fact]
    public void Spawn_de_armadilha_traz_dono_e_nome_do_dono()
    {
        var pacote = Hex(
            "C7014136DCC2035F000105596F736869AC902C000002007C12C600F88EC500F1" +
            "14477FAE5543F49701D922D9225B0700005B0700000000000000000000000000" +
            "008493010064000000F04902000100000000000000A08601000000000090D003" +
            "000201110181969800FFFFFFFFFFFFFFFF8075D52ABB030000DCC2030102007C" +
            "12C600F88EC500F11447070206A62B00006C0000000000610907547562617A61" +
            "6902000200000000000000000000000000000002CD0006040000D0002F010000" +
            "1800000000");

        Assert.True(Combate.TentarSpawnInvocacao(pacote, out uint entidade, out uint dono, out string nomeDono));
        Assert.Equal(57692u, entidade);
        Assert.Equal(11174u, dono);
        Assert.Equal("Yoshi", nomeDono);
    }

    [Fact]
    public void Morte_de_mob_traz_skill_matador_e_nome()
    {
        var pacote = Hex("28048DCBE702A0EDD500A657610905596F73686907547562617A6169020000000000000100");
        Assert.True(Combate.TentarMorte(pacote, out uint morto, out uint matador, out uint skill, out string nome));
        Assert.Equal(46027u, morto);
        Assert.Equal(11174u, matador);
        Assert.Equal(14020000u, skill);
        Assert.Equal("Yoshi", nome);
    }

    [Fact]
    public void Morte_de_armadilha_que_expirou_vem_sem_matador()
    {
        Assert.True(Combate.TentarMorte(Hex("1B048DDCC203000000000000000000000000800600000000"), out uint morto, out uint matador, out _, out string nome));
        Assert.Equal(57692u, morto);
        Assert.Equal(0u, matador);
        Assert.Equal("", nome);
    }

    [Fact]
    public void Golpe_de_mob_no_jogador()
    {
        Assert.True(Combate.TentarDano(Hex("240438A6570600CAAF03ACAA120002020000023BAB4A0701000000904EA6010100"), out var e, out _));
        Assert.Equal(11174u, e.AlvoId);
        Assert.Equal(55242u, e.AutorId);
        Assert.Equal(1223340u, e.Skill);
        Assert.Equal(166ul, e.Dano); // o HP do jogador caiu exatamente 166 (PROTOCOLO.md §6)
    }

    // 0x3645 de 2026-10-02: cabeçalho real + trecho real do bloco do level (os ~1.000 bytes de
    // equipamento do meio foram cortados). Conferidos no jogo: Dacura level 45 (o 32 logo depois do
    // nome não é o level), Nxhunter level 43 e power 909. O 1.040 do Dacura e o 366 do Auril vêm do
    // mesmo campo, sem conferência própria; o Auril cobre o bloco que começa pela tag 0xCE.
    [Theory]
    [InlineData("D60A4536AD7E0520A00107064461637572612000000002024096C06A8947",
                "0000000E0101CE5704000000610904CD008C000000CE0060F0FFFFD00036010000270248F4FFFF2D0000000000000010040000DE020C",
                16173u, "Dacura", 45, 1040)]
    [InlineData("BA0A4536CB6B15B0A4010705417572696C240000000202009227D39C46",
                "0000000E01019E8404000000610903CE0048F4FFFFD00043010000270248F4FFFF20000000000000006E0100002A0400",
                13771u, "Auril", 32, 366)]
    [InlineData("E50A4536E64B15B0A40107084E7868756E74657210000000020200D2",
                "0000000E01019D6404000000610904CD00AA000000CE0060F0FFFFD00033010000270248F4FFFF2B000000000000008D03000068040000",
                9702u, "Nxhunter", 43, 909)]
    public void Info_de_outro_jogador_traz_nome_level_e_power(string cabecalho, string bloco, uint idEsperado, string nomeEsperado, int nivelEsperado, int poderEsperado)
    {
        Assert.True(Combate.TentarInfoJogador(Hex(cabecalho + bloco), out uint id, out string nome, out int nivel, out int poder));
        Assert.Equal(idEsperado, id);
        Assert.Equal(nomeEsperado, nome);
        Assert.Equal(nivelEsperado, nivel);
        Assert.Equal(poderEsperado, poder);
    }

    [Fact]
    public void Info_de_outro_jogador_sem_bloco_fica_sem_level()
    {
        Assert.True(Combate.TentarInfoJogador(Hex("D60A4536AD7E0520A00107064461637572612000000002024096C06A8947"), out _, out string nome, out int nivel, out int poder));
        Assert.Equal("Dacura", nome);
        Assert.Equal(0, nivel);
        Assert.Equal(0, poder);
    }

    [Fact]
    public void Seu_personagem_no_login_traz_level_e_power()
    {
        Assert.True(Combate.TentarInfoPersonagem(Hex("8D0F3336AB575FA1C1283705596F73686961090F000000021F00000063010000630100001F000000"),
            out uint id, out string nome, out int nivel, out int poder));
        Assert.Equal(11179u, id);
        Assert.Equal("Yoshi", nome);
        Assert.Equal(31, nivel);
        Assert.Equal(355, poder);
    }

    [Fact]
    public void Atualizacao_de_power()
    {
        // 0x561C de 2026-10-02 03:58:20: o power do Yoshi foi de 355 para 361 (o jogo mostrava 361).
        Assert.True(Combate.TentarPoder(Hex("511C56AB576901000001018955950600010000000000000000EE00"), out uint id, out int poder));
        Assert.Equal(11179u, id);
        Assert.Equal(361, poder);
    }

    [Fact]
    public void Spawn_de_mob_nao_e_invocacao()
    {
        var inicio = Hex("6D413687D1030C20001D38290000020000B1C5008438C600680C4700809D4300E00194A70794A707");
        Assert.False(Combate.TentarSpawnInvocacao(inicio, out uint entidade, out uint dono, out _));
        Assert.NotEqual(0u, entidade);
        Assert.Equal(0u, dono);
    }
}
