using System.Net;
using Aion2Meter.Core.Captura;
using Aion2Meter.Core.Medicao;

namespace Aion2Meter.Testes;

/// <summary>Troca de servidor: a conexão nova assume sozinha e o começo dela (seu personagem) é lido.</summary>
public class SessaoTestes
{
    private static readonly DateTime T0 = new(2026, 10, 2, 4, 10, 0, DateTimeKind.Utc);
    private static readonly IPAddress Cliente = IPAddress.Parse("192.168.0.2");

    private const string Heartbeat = "0E0036942EC2FAA0010000";
    // 0x3633 real do login, com o id trocado para 11174 (o do golpe) e o tamanho ajustado para 39 bytes.
    private const string Login = "2A3336A6575FA1C1283705596F73686961090F000000021F00000063010000630100001F000000";
    private const string Golpe = "210438CBE7020400A657575FE1003002073E095801000000AC57E8010100"; // 11174 → mob 46027

    private sealed class Conexao(string servidor, ushort portaLocal, ushort portaServidor = 13328)
    {
        private uint seq = 1000;
        public string Chave => $"{servidor}:{portaServidor} > {Cliente}:{portaLocal}";

        public SegmentoTcp Syn() => new(IPAddress.Parse(servidor), portaServidor, Cliente, portaLocal, seq - 1, true, []);

        public SegmentoTcp Dados(string hex)
        {
            var dados = Convert.FromHexString(hex);
            var seg = new SegmentoTcp(IPAddress.Parse(servidor), portaServidor, Cliente, portaLocal, seq, false, dados);
            seq += (uint)dados.Length;
            return seg;
        }
    }

    [Fact]
    public void Conexao_nova_assume_mesmo_com_a_antiga_viva_e_le_o_login()
    {
        var sessao = new Sessao();
        var antiga = new Conexao("193.202.112.171", 62225);
        var nova = new Conexao("193.202.112.195", 50000);
        var t = T0;

        for (int i = 0; i < 3; i++) sessao.AoSegmento(antiga.Dados(Heartbeat), t = t.AddMilliseconds(50));
        sessao.AoSegmento(antiga.Dados(Golpe), t = t.AddMilliseconds(50)); // id da sessão velha
        Assert.Equal(antiga.Chave, sessao.Fluxo);

        // O login chega antes de a conexão nova ser reconhecida, e a antiga segue mandando heartbeat.
        sessao.AoSegmento(nova.Syn(), t = t.AddMilliseconds(10));
        sessao.AoSegmento(nova.Dados(Login), t = t.AddMilliseconds(10));
        for (int i = 0; i < 3; i++)
        {
            sessao.AoSegmento(antiga.Dados(Heartbeat), t = t.AddMilliseconds(25));
            sessao.AoSegmento(nova.Dados(Heartbeat), t = t.AddMilliseconds(25));
        }
        sessao.AoSegmento(nova.Dados(Golpe), t = t.AddMilliseconds(50));
        sessao.AoSegmento(antiga.Dados(Heartbeat), t = t.AddMilliseconds(25)); // não volta para a antiga

        Assert.Equal(nova.Chave, sessao.Fluxo);
        var yoshi = Assert.Single(sessao.Medidor.ObterPlacar().Dano.Jogadores); // nada da sessão velha
        Assert.Equal("Yoshi", yoshi.Nome);
        Assert.True(yoshi.Voce);
        Assert.Equal(31, yoshi.Nivel);
        Assert.Equal(355, yoshi.Poder);
        Assert.Equal(1, yoshi.Golpes);
    }

    [Fact]
    public void Conexao_nova_de_outro_programa_nao_rouba_o_fluxo()
    {
        var sessao = new Sessao();
        var jogo = new Conexao("193.202.112.171", 62225);
        var outroPrograma = new Conexao("10.0.0.9", 50001, portaServidor: 27015); // por acaso com 0E 00 36 nos dados
        var t = T0;

        for (int i = 0; i < 3; i++) sessao.AoSegmento(jogo.Dados(Heartbeat), t = t.AddMilliseconds(50));
        sessao.AoSegmento(jogo.Dados(Golpe), t = t.AddMilliseconds(50));
        sessao.AoSegmento(outroPrograma.Syn(), t = t.AddMilliseconds(10));
        for (int i = 0; i < 3; i++)
        {
            sessao.AoSegmento(jogo.Dados(Heartbeat), t = t.AddMilliseconds(25));
            sessao.AoSegmento(outroPrograma.Dados(Heartbeat), t = t.AddMilliseconds(25));
        }

        Assert.Equal(jogo.Chave, sessao.Fluxo);
        Assert.Equal(1, Assert.Single(sessao.Medidor.ObterPlacar().Dano.Jogadores).Golpes); // luta intacta
    }

    [Fact]
    public void Fluxo_sem_syn_so_assume_depois_de_5s_de_silencio_do_atual()
    {
        var sessao = new Sessao();
        var atual = new Conexao("193.202.112.171", 62225);
        var outra = new Conexao("193.202.112.195", 50000); // já existia quando o medidor abriu
        var t = T0;

        for (int i = 0; i < 3; i++) sessao.AoSegmento(atual.Dados(Heartbeat), t = t.AddMilliseconds(50));
        for (int i = 0; i < 3; i++) sessao.AoSegmento(outra.Dados(Heartbeat), t = t.AddMilliseconds(50));
        Assert.Equal(atual.Chave, sessao.Fluxo);

        sessao.AoSegmento(outra.Dados(Heartbeat), t.AddSeconds(6));
        Assert.Equal(outra.Chave, sessao.Fluxo);
    }
}
