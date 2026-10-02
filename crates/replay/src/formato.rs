//! Números como o .NET formata em pt-BR ("N", "P" e "F"). Os formatos padrão do .NET arredondam
//! o valor exato do double com empate para o par, igual ao `format!` do Rust (conferido com
//! 2,5 → "2", 3,5 → "4", 0,125 → "0,12" e 0,0125 em "P1" → "1,3%").

/// "N": separador de milhar e `casas` decimais.
pub fn n(valor: f64, casas: usize) -> String {
    agrupar(&format!("{valor:.casas$}"))
}

/// "N0" de inteiro.
pub fn n_int(valor: impl std::fmt::Display) -> String {
    agrupar(&valor.to_string())
}

/// "F": `casas` decimais, sem separador de milhar.
pub fn f(valor: f64, casas: usize) -> String {
    format!("{valor:.casas$}").replace('.', ",")
}

/// "P": porcentagem colada no número ("36,7%"). O .NET não multiplica por 100 em ponto
/// flutuante: arredonda com 2 casas a mais e desloca a vírgula.
pub fn p(valor: f64, casas: usize) -> String {
    let texto = format!("{:.*}", casas + 2, valor);
    let (sinal, resto) = texto.strip_prefix('-').map_or(("", texto.as_str()), |r| ("-", r));
    let (inteira, decimal) = resto.split_once('.').unwrap_or((resto, ""));
    let (duas, sobra) = decimal.split_at(2);
    let inteira = format!("{inteira}{duas}");
    let inteira = inteira.trim_start_matches('0');
    let inteira = if inteira.is_empty() { "0" } else { inteira };
    let numero = if sobra.is_empty() { inteira.to_string() } else { format!("{inteira}.{sobra}") };
    format!("{sinal}{}%", agrupar(&numero))
}

/// "1234567.5" → "1.234.567,5"
fn agrupar(texto: &str) -> String {
    let (sinal, resto) = texto.strip_prefix('-').map_or(("", texto), |r| ("-", r));
    let (inteira, decimal) = resto.split_once('.').map_or((resto, None), |(i, d)| (i, Some(d)));
    let mut saida = String::from(sinal);
    for (i, c) in inteira.chars().enumerate() {
        if i > 0 && (inteira.len() - i) % 3 == 0 {
            saida.push('.');
        }
        saida.push(c);
    }
    if let Some(d) = decimal {
        saida.push(',');
        saida.push_str(d);
    }
    saida
}

/// Bytes em hexadecimal maiúsculo, como o Convert.ToHexString.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn igual_ao_dotnet_pt_br() {
        // Saída do .NET 10 em pt-BR para os mesmos valores.
        assert_eq!(n(2.5, 0), "2");
        assert_eq!(n(3.5, 0), "4");
        assert_eq!(f(0.125, 2), "0,12");
        assert_eq!(f(0.375, 2), "0,38");
        assert_eq!(f(0.15, 1), "0,1");
        assert_eq!(n(0.25, 1), "0,2");
        assert_eq!(p(0.0125, 1), "1,3%");
        assert_eq!(p(0.005, 0), "1%");
        assert_eq!(n(1234567.5, 0), "1.234.568");
        assert_eq!(p(0.0, 1), "0,0%");
        assert_eq!(p(1.0, 0), "100%");
        assert_eq!(p(1.0, 1), "100,0%");
        assert_eq!(p(0.367, 1), "36,7%");
        assert_eq!(n_int(111916u64), "111.916");
    }
}
