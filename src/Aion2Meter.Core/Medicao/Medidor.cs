using Aion2Meter.Core.Protocolo;

namespace Aion2Meter.Core.Medicao;

public sealed record LinhaSkill(uint Skill, string Nome, string? Icone, double Total, double Porcentagem, int Golpes, int Criticos, double Maximo);

/// <summary>NivelLembrado/PoderLembrado: valor da memória por nome (visto numa conexão anterior), não desta.</summary>
public sealed record LinhaJogador(
    uint Id, string Nome, string Classe, int Nivel, bool NivelLembrado, int Poder, bool PoderLembrado, bool Voce,
    double Total, double PorSegundo, double Porcentagem, int Golpes, int Criticos,
    int Aparos, int Mortes, int SegurandoAggro,
    IReadOnlyList<LinhaSkill> Skills);

public sealed record Tabela(double Total, IReadOnlyList<LinhaJogador> Jogadores);

/// <summary>Último level e power vistos de um nome; 0 = desconhecido.</summary>
public sealed record PerfilJogador(int Nivel, int Poder);

/// <summary>Uma luta vista de três lados: dano causado (DPS), dano recebido (Tank) e cura (Healer).</summary>
public sealed record Placar(TimeSpan Duracao, Tabela Dano, Tabela DanoRecebido, Tabela Cura);

/// <summary>
/// Soma por jogador e por skill. Cada evento traz autor, alvo e skill, então a atribuição
/// não depende da queda de HP do alvo (que mistura o dano do grupo todo). Classificação:
/// jogador → mob = dano causado; mob → jogador = dano recebido; jogador → jogador = cura,
/// se a skill for de cura (o resto, PvP e buffs, fica de fora).
/// Uma luta termina depois de <see cref="Inatividade"/> sem eventos; o próximo zera tudo.
/// </summary>
public sealed class Medidor
{
    /// <summary>
    /// Dano em mob: campo × fator = HP descontado do mob. Medido em 2026-10-01: mediana 18,82
    /// em 150 golpes isolados, 8 mobs (PROTOCOLO.md §6). Medido só com Ranger e mobs comuns;
    /// assumido igual para todos (e para chefes) até conferir numa captura em grupo.
    /// </summary>
    public const double FatorEscala = 18.82;

    /// <summary>
    /// Dano em jogador: o campo já é o HP do jogador (golpe de 166 → HP caiu 166, §6).
    /// Cura usa o mesmo fator por suposição: nenhuma captura teve curandeiro.
    /// </summary>
    public const double FatorEscalaJogador = 1.0;

    /// <summary>Mob que atacou alguém nesse intervalo conta como "segurando aggro" nesse jogador.</summary>
    private static readonly TimeSpan JanelaAggro = TimeSpan.FromSeconds(8);

    public TimeSpan Inatividade { get; set; } = TimeSpan.FromSeconds(15);

    private sealed class Acumulado
    {
        public double Total;
        public int Golpes, Criticos, Aparos, Mortes;
        public readonly Dictionary<uint, (double Total, int Golpes, int Criticos, double Maximo)> Skills = [];
    }

    private readonly Lock trava = new();
    private readonly Dictionary<uint, Acumulado> dano = [];
    private readonly Dictionary<uint, Acumulado> recebido = [];
    private readonly Dictionary<uint, Acumulado> curaCandidata = [];
    private readonly Dictionary<uint, (uint Alvo, DateTime Hora)> ultimoAlvoDoMob = [];
    private DateTime inicio, ultimo;
    private bool emLuta;

    // Estado que sobrevive entre lutas.
    private readonly Dictionary<uint, string> nomes = [];
    private readonly Dictionary<uint, int> niveis = [];
    private readonly Dictionary<uint, int> poderes = [];
    private readonly Dictionary<uint, Dictionary<uint, int>> prefixosDe = [];
    private uint? meuId;

    // Ids que são jogadores ou invocações deles: separa "jogador → mob" de "jogador → jogador".
    private readonly HashSet<uint> jogadoresConhecidos = [];

    // Invocações, pets e armadilhas agem com id próprio; o efeito vai para a linha do dono.
    private readonly Dictionary<uint, uint> donoDe = [];
    private readonly Dictionary<uint, string> nomeDonoDe = [];

    // Memória por nome, que sobrevive à troca de conexão e (pelo overlay) entre execuções: level e
    // power só chegam no login (você) ou quando o jogador entra na visão (outros). Com o overlay
    // aberto no meio da sessão, o nome que aparece num abate ou numa invocação puxa o último valor.
    private readonly Dictionary<string, PerfilJogador> memoria = [];
    private string? meuNome;

    /// <summary>Nível 0 = desconhecido (não apaga um nível já visto).</summary>
    public void DefinirJogador(uint id, string nome, int nivel, bool voce)
    {
        lock (trava)
        {
            if (nome.Length > 0) Nomear(id, nome);
            if (nivel > 0) niveis[id] = nivel;
            if (voce)
            {
                meuId = id;
                if (nome.Length > 0) meuNome = nome;
            }
            jogadoresConhecidos.Add(id);
            Lembrar(id);
        }
    }

    /// <summary>Power 0 = desconhecido (não apaga um valor já visto).</summary>
    public void DefinirPoder(uint id, int poder)
    {
        if (poder <= 0) return;
        lock (trava)
        {
            poderes[id] = poder;
            Lembrar(id);
        }
    }

    /// <summary>Carrega a memória salva (o overlay guarda em arquivo).</summary>
    public void CarregarMemoria(string? seuNome, IEnumerable<KeyValuePair<string, PerfilJogador>> perfis)
    {
        lock (trava)
        {
            meuNome ??= seuNome;
            foreach (var (nome, perfil) in perfis) memoria.TryAdd(nome, perfil);
        }
    }

    public (string? SeuNome, Dictionary<string, PerfilJogador> Perfis) ExportarMemoria()
    {
        lock (trava) return (meuNome, new Dictionary<string, PerfilJogador>(memoria));
    }

    private void Nomear(uint id, string nome)
    {
        nomes[id] = nome;
        // Sem 0x3633 nesta conexão, o seu nome guardado identifica você; um 0x3633 nunca é trocado.
        if (meuId is null && nome == meuNome && !EhInvocacao(id)) meuId = id;
    }

    private void Lembrar(uint id)
    {
        if (!nomes.TryGetValue(id, out var nome)) return;
        var antes = memoria.GetValueOrDefault(nome);
        memoria[nome] = new PerfilJogador(
            niveis.GetValueOrDefault(id, antes?.Nivel ?? 0),
            poderes.GetValueOrDefault(id, antes?.Poder ?? 0));
    }

    public void DefinirInvocacao(uint invocacao, uint dono, string nomeDono)
    {
        lock (trava)
        {
            jogadoresConhecidos.Add(invocacao);
            if (dono != 0)
            {
                jogadoresConhecidos.Add(dono);
                donoDe[invocacao] = dono;
                if (nomeDono.Length > 0 && !nomes.ContainsKey(dono)) Nomear(dono, nomeDono);
            }
            else if (nomeDono.Length > 0) nomeDonoDe[invocacao] = nomeDono;
        }
    }

    /// <summary>Ids são reaproveitados depois de troca de zona: spawn que não é invocação apaga o vínculo.</summary>
    public void EsquecerInvocacao(uint entidade)
    {
        lock (trava)
        {
            donoDe.Remove(entidade);
            nomeDonoDe.Remove(entidade);
            jogadoresConhecidos.Remove(entidade);
        }
    }

    private uint ResolverAutor(uint autor)
    {
        // Invocação de invocação existe (cadeia); o limite evita laço se os ids se repetirem.
        for (int saltos = 0; saltos < 16; saltos++)
        {
            if (donoDe.TryGetValue(autor, out uint dono)) { autor = dono; continue; }
            if (nomeDonoDe.TryGetValue(autor, out string? nome))
            {
                uint achado = nomes.FirstOrDefault(n => n.Value == nome).Key;
                if (achado != 0 && achado != autor) { autor = achado; continue; }
            }
            break;
        }
        return autor;
    }

    private bool EhInvocacao(uint id) => donoDe.ContainsKey(id) || nomeDonoDe.ContainsKey(id);

    public void Registrar(EventoDano e, DateTime hora)
    {
        if (e.Dano == 0) return;

        lock (trava)
        {
            bool skillDeClasse = e.Skill is >= 11_000_000 and < 20_000_000;
            if (skillDeClasse) jogadoresConhecidos.Add(e.AutorId);

            bool autorJogador = jogadoresConhecidos.Contains(e.AutorId) || DadosJogo.EhSkillDeJogador(e.Skill);
            bool alvoJogador = jogadoresConhecidos.Contains(e.AlvoId);

            if (autorJogador && (alvoJogador || DadosJogo.EhCura(e.Skill)))
            {
                // Jogador → jogador (inclui si mesmo): só vira cura se a skill for de cura,
                // decidido em ObterPlacar (a classificação pode chegar depois do evento).
                IniciarOuContinuarLuta(hora);
                uint autor = ResolverAutor(e.AutorId);
                ContarPrefixo(autor, e.Skill);
                Somar(curaCandidata, autor, e.Skill, e.Dano * FatorEscalaJogador, e);
            }
            else if (autorJogador)
            {
                IniciarOuContinuarLuta(hora);
                uint autor = ResolverAutor(e.AutorId);
                ContarPrefixo(autor, e.Skill);
                Somar(dano, autor, e.Skill, e.Dano * FatorEscala, e);
            }
            else if (alvoJogador && !EhInvocacao(e.AlvoId))
            {
                IniciarOuContinuarLuta(hora);
                var a = Somar(recebido, e.AlvoId, e.Skill, e.Dano * FatorEscalaJogador, e);
                if (e.Aparo) a.Aparos++;
                ultimoAlvoDoMob[e.AutorId] = (e.AlvoId, hora);
            }
            // Mob → mob e golpe em invocação ficam de fora.
        }
    }

    /// <summary>
    /// 0x8D04: quem morreu e quem matou. Se a skill que matou é de classe, o matador é jogador
    /// e o nome dele vale (mob que mata também pode trazer nome; invocação traz o do dono).
    /// </summary>
    public void RegistrarMorte(uint entidade, uint matador, uint skill, string nomeMatador, DateTime hora)
    {
        lock (trava)
        {
            bool matadorJogador = skill is >= 11_000_000 and < 20_000_000 || jogadoresConhecidos.Contains(matador);
            if (matador != 0 && matadorJogador && !EhInvocacao(matador) && nomeMatador.Length > 0)
            {
                Nomear(matador, nomeMatador);
                jogadoresConhecidos.Add(matador);
            }
            // Mob morto não ataca mais ninguém; jogador morto não segura mais o aggro de ninguém.
            ultimoAlvoDoMob.Remove(entidade);
            foreach (var mob in ultimoAlvoDoMob.Where(p => p.Value.Alvo == entidade).Select(p => p.Key).ToList())
                ultimoAlvoDoMob.Remove(mob);

            if (!jogadoresConhecidos.Contains(entidade) || EhInvocacao(entidade)) return;
            IniciarOuContinuarLuta(hora);
            if (!recebido.TryGetValue(entidade, out var a)) recebido[entidade] = a = new Acumulado();
            a.Mortes++;
        }
    }

    private void IniciarOuContinuarLuta(DateTime hora)
    {
        if (emLuta && hora - ultimo > Inatividade) LimparLuta();
        if (!emLuta)
        {
            inicio = hora;
            emLuta = true;
        }
        ultimo = hora;
    }

    private void LimparLuta()
    {
        dano.Clear();
        recebido.Clear();
        curaCandidata.Clear();
        ultimoAlvoDoMob.Clear();
        emLuta = false;
    }

    private void ContarPrefixo(uint jogador, uint skill)
    {
        if (!prefixosDe.TryGetValue(jogador, out var p)) prefixosDe[jogador] = p = [];
        uint prefixo = skill / 1_000_000;
        p[prefixo] = p.GetValueOrDefault(prefixo) + 1;
    }

    private static Acumulado Somar(Dictionary<uint, Acumulado> tabela, uint quem, uint skill, double valor, EventoDano e)
    {
        if (!tabela.TryGetValue(quem, out var a)) tabela[quem] = a = new Acumulado();
        a.Total += valor;
        a.Golpes++;
        if (e.Critico) a.Criticos++;

        uint chave = DadosJogo.SkillBase(skill);
        var s = a.Skills.GetValueOrDefault(chave);
        a.Skills[chave] = (s.Total + valor, s.Golpes + 1, s.Criticos + (e.Critico ? 1 : 0), Math.Max(s.Maximo, valor));
        return a;
    }

    public void Reiniciar()
    {
        lock (trava) LimparLuta();
    }

    /// <summary>
    /// Conexão nova com o servidor (login, troca de servidor ou canal): os ids de entidade mudam
    /// (o mesmo personagem foi #11174, #7301 e #11179), então nada indexado por id continua valendo.
    /// </summary>
    public void NovaConexao()
    {
        lock (trava)
        {
            LimparLuta();
            nomes.Clear();
            niveis.Clear();
            poderes.Clear();
            prefixosDe.Clear();
            jogadoresConhecidos.Clear();
            donoDe.Clear();
            nomeDonoDe.Clear();
            meuId = null; // a memória e o seu nome continuam: valem para a conexão nova
        }
    }

    public Placar ObterPlacar()
    {
        lock (trava)
        {
            var duracao = emLuta ? ultimo - inicio : TimeSpan.Zero;
            double segundos = Math.Max(1, duracao.TotalSeconds);

            // Cura: só as skills classificadas como cura; o resto (PvP, buff com valor) some.
            var cura = new Dictionary<uint, Acumulado>();
            foreach (var (quem, a) in curaCandidata)
            {
                var c = new Acumulado();
                foreach (var (skill, s) in a.Skills.Where(s => DadosJogo.EhCura(s.Key)))
                {
                    c.Skills[skill] = s;
                    c.Total += s.Total;
                    c.Golpes += s.Golpes;
                    c.Criticos += s.Criticos;
                }
                if (c.Golpes > 0) cura[quem] = c;
            }

            var segurando = ultimoAlvoDoMob.Values
                .Where(v => ultimo - v.Hora <= JanelaAggro)
                .GroupBy(v => v.Alvo)
                .ToDictionary(g => g.Key, g => g.Count());

            return new Placar(duracao,
                MontarTabela(dano, segundos, segurando),
                MontarTabela(recebido, segundos, segurando),
                MontarTabela(cura, segundos, segurando));
        }
    }

    private Tabela MontarTabela(Dictionary<uint, Acumulado> tabela, double segundos, Dictionary<uint, int> segurando)
    {
        double total = tabela.Values.Sum(a => a.Total);
        var jogadores = tabela
            .OrderByDescending(p => p.Value.Total)
            .ThenByDescending(p => p.Value.Mortes)
            .Select(p =>
            {
                var a = p.Value;
                var skills = a.Skills
                    .OrderByDescending(s => s.Value.Total)
                    .Select(s => new LinhaSkill(s.Key, DadosJogo.NomeSkill(s.Key), DadosJogo.IconeSkill(s.Key), s.Value.Total,
                        a.Total > 0 ? s.Value.Total / a.Total : 0, s.Value.Golpes, s.Value.Criticos, s.Value.Maximo))
                    .ToList();
                var lembrado = nomes.TryGetValue(p.Key, out var nome) ? memoria.GetValueOrDefault(nome) : null;
                bool nivelLembrado = !niveis.ContainsKey(p.Key) && lembrado?.Nivel > 0;
                bool poderLembrado = !poderes.ContainsKey(p.Key) && lembrado?.Poder > 0;
                return new LinhaJogador(
                    p.Key, nome ?? $"#{p.Key}", Classe(p.Key),
                    nivelLembrado ? lembrado!.Nivel : niveis.GetValueOrDefault(p.Key), nivelLembrado,
                    poderLembrado ? lembrado!.Poder : poderes.GetValueOrDefault(p.Key), poderLembrado,
                    p.Key == meuId,
                    a.Total, a.Total / segundos, total > 0 ? a.Total / total : 0,
                    a.Golpes, a.Criticos, a.Aparos, a.Mortes, segurando.GetValueOrDefault(p.Key),
                    skills);
            })
            .ToList();
        return new Tabela(total, jogadores);
    }

    private string Classe(uint jogador)
    {
        if (!prefixosDe.TryGetValue(jogador, out var p)) return "";
        // Prefixo 10 é o espírito do Elementalist: só vale como classe se não houver outra.
        uint prefixo = p
            .OrderByDescending(c => c.Key is >= 11 and < 20)
            .ThenByDescending(c => c.Value)
            .Select(c => c.Key).FirstOrDefault();
        return DadosJogo.Classe(prefixo * 1_000_000);
    }
}
