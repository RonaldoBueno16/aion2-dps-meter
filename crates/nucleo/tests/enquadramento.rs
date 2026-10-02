mod comum;

use comum::hex;
use nucleo::captura::montador::{Entrega, MontadorTcp};
use nucleo::protocolo::desempacotador::Desempacotador;
use nucleo::protocolo::enquadrador::Enquadrador;

// Heartbeat de 11 bytes (0x0E + 1 - 4) e um 0x3804 real da captura de 2026-10-01.
const HEARTBEAT: &str = "0E0036AABBCCDDEEFF0011";
const DANO: &str = "210438CBE7020400A657575FE1003002073E095801000000AC57E8010100";

#[test]
fn corta_stream_entregue_byte_a_byte_e_descarta_lixo_antes_do_heartbeat() {
    let stream = [hex("123456"), hex(HEARTBEAT), hex(DANO), hex(HEARTBEAT)].concat();
    let mut enquadrador = Enquadrador::default();
    let mut pacotes: Vec<Vec<u8>> = Vec::new();

    for b in stream {
        enquadrador.adicionar(&[b], &mut |p| pacotes.push(p.to_vec()));
    }

    assert_eq!(pacotes.len(), 3);
    assert_eq!(pacotes[0], hex(HEARTBEAT));
    assert_eq!(pacotes[1], hex(DANO));
    assert_eq!(enquadrador.bytes_descartados, 3);
}

#[test]
fn abre_bloco_comprimido_com_pacotes_dentro() {
    let interno = [hex(DANO), vec![0x00, 0x00], hex(HEARTBEAT)].concat();
    let lz4 = lz4_flex::block::compress(&interno);

    // [varint tamanho][FF FF][u32 tamanho descomprimido][LZ4]; tamanho total = varint + 1 - 4
    let corpo = [vec![0xFF, 0xFF], (interno.len() as i32).to_le_bytes().to_vec(), lz4].concat();
    let total = 1 + corpo.len();
    assert!(total + 3 < 0x80, "o teste assume varint de 1 byte");
    let pacote = [vec![(total + 3) as u8], corpo].concat();

    let mut desempacotador = Desempacotador::default();
    let mut saida: Vec<Vec<u8>> = Vec::new();
    desempacotador.expandir(&pacote, &mut |p| saida.push(p.to_vec()));

    assert_eq!(desempacotador.blocos_comprimidos, 1);
    assert_eq!(desempacotador.tamanhos_divergentes, 0);
    assert_eq!(saida.len(), 2);
    assert_eq!(saida[0], hex(DANO));
    assert_eq!(saida[1], hex(HEARTBEAT));
}

#[test]
fn remonta_tcp_fora_de_ordem_e_ignora_retransmissao() {
    let mut montador = MontadorTcp::default();
    let mut recebido: Vec<u8> = Vec::new();
    let mut perdas = 0;
    let mut enviar = |seq: u32, texto: &str| {
        montador.adicionar(seq, false, texto.as_bytes(), &mut |entrega| match entrega {
            Entrega::Dados(d) => recebido.extend_from_slice(d),
            Entrega::Perda => perdas += 1,
        });
    };

    enviar(1000, "abc");
    enviar(1006, "ghi"); // chega antes de "def"
    enviar(1003, "def");
    enviar(1000, "abc"); // retransmissão
    enviar(1007, "hijk"); // sobreposição parcial: só "jk" é novo

    assert_eq!(String::from_utf8(recebido).unwrap(), "abcdefghijk");
    assert_eq!(perdas, 0);
    assert_eq!(montador.retransmissoes, 1);
}
