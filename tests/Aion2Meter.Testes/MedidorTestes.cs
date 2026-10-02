using Aion2Meter.Core.Medicao;
using Aion2Meter.Core.Protocolo;

namespace Aion2Meter.Testes;

public class MedidorTestes
{
    private static readonly DateTime T0 = new(2026, 10, 1, 22, 0, 0, DateTimeKind.Utc);
    private const uint Mob = 46027;

    private static EventoDano Golpe(uint autor, uint skill, ulong dano, uint alvo = Mob, bool critico = false, bool aparo = false) =>
        new(AlvoId: alvo, AutorId: autor, Skill: skill, Dano: dano, TipoDano: critico ? 3 : 2,
            Critico: critico, Costas: false, Frente: false, Aparo: aparo, Perfeito: false, Duplo: false, Periodico: false);

    [Fact]
    public void Separa_por_jogador_e_por_skill_e_soma_invocacao_no_dono()
    {
        var m = new Medidor();
        m.DefinirInvocacao(57692, 11174, "Yoshi");

        m.Registrar(Golpe(11174, 14340000, 100), T0);
        m.Registrar(Golpe(11174, 14030010, 50, critico: true), T0.AddSeconds(1)); // variante de 14030000
        m.Registrar(Golpe(57692, 14170001, 200), T0.AddSeconds(2));                // armadilha do Yoshi
        m.Registrar(Golpe(22222, 11010000, 300), T0.AddSeconds(4));                // outro jogador
        m.Registrar(Golpe(33333, 1223340, 999, alvo: 44444), T0.AddSeconds(4));    // mob em mob: fora

        var p = m.ObterPlacar();
        var dano = p.Dano;

        Assert.Equal(2, dano.Jogadores.Count);
        Assert.Equal(TimeSpan.FromSeconds(4), p.Duracao);

        var yoshi = dano.Jogadores.Single(j => j.Id == 11174);
        Assert.Equal("Yoshi", yoshi.Nome);
        Assert.Equal("Ranger", yoshi.Classe);
        Assert.Equal(350 * Medidor.FatorEscala, yoshi.Total, 6);
        Assert.Equal(3, yoshi.Golpes);
        Assert.Equal(1, yoshi.Criticos);
        Assert.Equal(3, yoshi.Skills.Count);
        Assert.Equal(14170000u, yoshi.Skills[0].Skill);
        Assert.Equal(14030000u, yoshi.Skills.Single(s => s.Golpes == 1 && s.Criticos == 1).Skill);

        var outro = dano.Jogadores.Single(j => j.Id == 22222);
        Assert.Equal("Gladiator", outro.Classe);
        Assert.Equal(300.0 / 650, outro.Porcentagem, 6);
        Assert.Equal(300 * Medidor.FatorEscala / 4, outro.PorSegundo, 6);

        Assert.Empty(p.DanoRecebido.Jogadores);
        Assert.Empty(p.Cura.Jogadores);
    }

    [Fact]
    public void Golpe_de_mob_em_jogador_vai_para_dano_recebido_e_marca_aggro()
    {
        var m = new Medidor();
        m.Registrar(Golpe(11174, 14340000, 100, alvo: 55242), T0);                       // Yoshi bate no mob
        m.Registrar(Golpe(55242, 1223340, 166, alvo: 11174, aparo: true), T0.AddSeconds(1)); // mob revida
        m.Registrar(Golpe(55242, 1223340, 191, alvo: 11174), T0.AddSeconds(2));

        var tank = m.ObterPlacar().DanoRecebido.Jogadores.Single();
        Assert.Equal(11174u, tank.Id);
        Assert.Equal((166 + 191) * Medidor.FatorEscalaJogador, tank.Total, 6);
        Assert.Equal(2, tank.Golpes);
        Assert.Equal(1, tank.Aparos);
        Assert.Equal(1, tank.SegurandoAggro);
    }

    [Fact]
    public void Cura_em_aliado_e_em_si_mesmo_vai_para_healer_e_nao_para_dano()
    {
        var m = new Medidor();
        m.Registrar(Golpe(11174, 14340000, 100), T0);                                 // Yoshi vira jogador conhecido
        m.Registrar(Golpe(30303, 17010000, 50), T0.AddSeconds(1));                    // Clérigo bate no mob
        m.Registrar(Golpe(30303, 17120000, 400, alvo: 11174), T0.AddSeconds(2));      // Fulgor Restaurador no Yoshi
        m.Registrar(Golpe(30303, 17120000, 380, alvo: 30303), T0.AddSeconds(3));      // e em si mesmo
        m.Registrar(Golpe(30303, 17990000, 999, alvo: 11174), T0.AddSeconds(3));      // jogador → jogador sem ser cura: fora

        var p = m.ObterPlacar();
        var clerigo = p.Cura.Jogadores.Single();
        Assert.Equal(30303u, clerigo.Id);
        Assert.Equal("Cleric", clerigo.Classe);
        Assert.Equal(780 * Medidor.FatorEscalaJogador, clerigo.Total, 6);
        Assert.Equal(17120000u, clerigo.Skills.Single().Skill);

        Assert.Equal(50 * Medidor.FatorEscala, p.Dano.Jogadores.Single(j => j.Id == 30303).Total, 6);
        Assert.Empty(p.DanoRecebido.Jogadores);
    }

    [Fact]
    public void Morte_de_jogador_conta_no_tank_e_nome_do_matador_jogador_e_aproveitado()
    {
        var m = new Medidor();
        m.Registrar(Golpe(11174, 14340000, 100, alvo: 55242), T0);
        m.RegistrarMorte(55242, 11174, 14020000, "Yoshi", T0.AddSeconds(1)); // Yoshi mata o mob
        m.Registrar(Golpe(60000, 1223340, 500, alvo: 11174), T0.AddSeconds(2));
        m.RegistrarMorte(11174, 60000, 1223340, "Lobo", T0.AddSeconds(3));   // mob mata o Yoshi: "Lobo" não vira jogador

        var p = m.ObterPlacar();
        Assert.Equal("Yoshi", p.Dano.Jogadores.Single().Nome);
        var tank = p.DanoRecebido.Jogadores.Single();
        Assert.Equal(1, tank.Mortes);
        Assert.Equal(0, tank.SegurandoAggro); // morto não segura aggro
    }

    [Fact]
    public void Level_e_power_aparecem_na_linha_e_zero_nao_apaga()
    {
        var m = new Medidor();
        m.DefinirJogador(11174, "Yoshi", 30, voce: true);
        m.DefinirJogador(11174, "Yoshi", 0, voce: true); // pacote sem level: mantém o 30
        m.DefinirPoder(11174, 355);
        m.DefinirPoder(11174, 361);                       // 0x561C depois do login
        m.DefinirPoder(11174, 0);
        m.Registrar(Golpe(11174, 14340000, 100), T0);
        m.Registrar(Golpe(22222, 11010000, 100), T0);

        var dano = m.ObterPlacar().Dano.Jogadores;
        var yoshi = dano.Single(j => j.Id == 11174);
        Assert.Equal(30, yoshi.Nivel);
        Assert.Equal(361, yoshi.Poder);
        Assert.True(yoshi.Voce);
        var outro = dano.Single(j => j.Id == 22222);
        Assert.Equal(0, outro.Nivel);
        Assert.Equal(0, outro.Poder);
    }

    [Fact]
    public void Overlay_aberto_no_meio_da_sessao_reconhece_voce_pelo_nome_guardado()
    {
        var m = new Medidor();
        m.CarregarMemoria("Yoshi", [new("Yoshi", new PerfilJogador(31, 375))]);
        m.NovaConexao(); // a memória sobrevive à troca de conexão

        m.Registrar(Golpe(11174, 14340000, 100), T0);
        m.RegistrarMorte(Mob, 11174, 14020000, "Yoshi", T0.AddSeconds(1)); // nome chega pelo abate

        var yoshi = m.ObterPlacar().Dano.Jogadores.Single();
        Assert.Equal("Yoshi", yoshi.Nome);
        Assert.True(yoshi.Voce);
        Assert.Equal(31, yoshi.Nivel);
        Assert.True(yoshi.NivelLembrado);
        Assert.Equal(375, yoshi.Poder);
        Assert.True(yoshi.PoderLembrado);
    }

    [Fact]
    public void Valor_desta_conexao_vence_a_memoria_e_atualiza_a_memoria()
    {
        var m = new Medidor();
        m.CarregarMemoria(null, [new("Dacura", new PerfilJogador(44, 1000))]);
        m.DefinirJogador(16173, "Dacura", 45, voce: false); // 0x3645 desta conexão
        m.Registrar(Golpe(16173, 17010000, 100), T0);

        var dacura = m.ObterPlacar().Dano.Jogadores.Single();
        Assert.Equal(45, dacura.Nivel);
        Assert.False(dacura.NivelLembrado);
        Assert.Equal(1000, dacura.Poder);      // power ainda não chegou nesta conexão
        Assert.True(dacura.PoderLembrado);
        Assert.Equal(new PerfilJogador(45, 1000), m.ExportarMemoria().Perfis["Dacura"]);
    }

    [Fact]
    public void Login_desta_conexao_nao_e_trocado_por_nome_guardado()
    {
        var m = new Medidor();
        m.CarregarMemoria("Yoshi", []);
        m.DefinirJogador(500, "Yoshi", 31, voce: true); // 0x3633
        m.Registrar(Golpe(500, 14340000, 100), T0);
        m.Registrar(Golpe(600, 14340000, 100), T0);
        m.RegistrarMorte(Mob, 600, 14020000, "Yoshi", T0.AddSeconds(1)); // outro id com o mesmo nome

        Assert.True(m.ObterPlacar().Dano.Jogadores.Single(j => j.Id == 500).Voce);
        Assert.False(m.ObterPlacar().Dano.Jogadores.Single(j => j.Id == 600).Voce);
    }

    [Fact]
    public void Luta_nova_depois_de_inatividade()
    {
        var m = new Medidor { Inatividade = TimeSpan.FromSeconds(15) };
        m.Registrar(Golpe(11174, 14340000, 100), T0);
        m.Registrar(Golpe(11174, 14340000, 100), T0.AddSeconds(16));

        var p = m.ObterPlacar();
        Assert.Equal(1, p.Dano.Jogadores[0].Golpes);
        Assert.Equal(TimeSpan.Zero, p.Duracao);
    }
}
