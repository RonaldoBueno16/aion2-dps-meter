//! Ida e volta contra o compressor de referência (lz4_flex), nos casos que pegam decodificador ingênuo.

use nucleo::protocolo::lz4;

/// Gerador fixo (xorshift): os casos são os mesmos em toda execução.
struct Aleatorio(u64);

impl Aleatorio {
    fn proximo(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u8
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.proximo()).collect()
    }
}

fn ida_e_volta(caso: &str, original: &[u8]) {
    let comprimido = lz4_flex::block::compress(original);
    let mut saida = vec![0u8; original.len()];
    let escritos = lz4::descomprimir(&comprimido, &mut saida);
    assert_eq!(escritos, Some(original.len()), "{caso}: escreveu {escritos:?} de {}", original.len());
    assert!(saida == original, "{caso}: conteúdo diferente");
}

#[test]
fn aleatorio_64_kb_so_literais_longos() {
    ida_e_volta("aleatório 64 KB", &Aleatorio(42).bytes(65_536));
}

#[test]
fn zeros_100_kb_copia_sobreposta_com_distancia_1() {
    ida_e_volta("zeros 100 KB", &vec![0u8; 100_000]);
}

#[test]
fn padrao_de_3_bytes_sobreposicao_curta() {
    ida_e_volta("padrão de 3 bytes", &(0..50_000).map(|i| (i % 3) as u8).collect::<Vec<_>>());
}

#[test]
fn texto_repetido_matches_longos() {
    ida_e_volta("texto repetido", "Tempest Shot acertou o alvo; ".repeat(2000).as_bytes());
}

#[test]
fn misto() {
    let mut aleatorio = Aleatorio(7);
    let misto: Vec<u8> = (0..40_000).map(|i| if i % 97 < 60 { (i % 7) as u8 } else { aleatorio.proximo() }).collect();
    ida_e_volta("misto", &misto);
}

#[test]
fn pequeno() {
    ida_e_volta("pequeno", &Aleatorio(99).bytes(17));
}

#[test]
fn distancia_antes_do_inicio_e_invalida() {
    // token 0x10: 1 literal 'A', depois match com distância 5 (só há 1 byte escrito).
    let bloco = [0x10, b'A', 0x05, 0x00];
    assert_eq!(lz4::descomprimir(&bloco, &mut [0u8; 64]), None);
}

#[test]
fn destino_pequeno_e_invalido() {
    let comprimido = lz4_flex::block::compress(&[0u8; 1000]);
    assert_eq!(lz4::descomprimir(&comprimido, &mut [0u8; 10]), None);
}
