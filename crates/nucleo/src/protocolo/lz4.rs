//! Descompressor de bloco LZ4 cru (sem o cabeçalho do formato "frame").

/// Devolve quantos bytes foram escritos em `destino`, ou None se o bloco for inválido.
pub fn descomprimir(origem: &[u8], destino: &mut [u8]) -> Option<usize> {
    let (mut o, mut d) = (0usize, 0usize);
    while o < origem.len() {
        let token = origem[o];
        o += 1;

        let mut literais = usize::from(token >> 4);
        if literais == 15 {
            loop {
                let b = *origem.get(o)?;
                o += 1;
                literais += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        let fonte = origem.get(o..o + literais)?;
        destino.get_mut(d..d + literais)?.copy_from_slice(fonte);
        o += literais;
        d += literais;

        // A última sequência do bloco só tem literais.
        if o >= origem.len() {
            break;
        }

        let distancia = usize::from(*origem.get(o)?) | usize::from(*origem.get(o + 1)?) << 8;
        o += 2;
        if distancia == 0 || distancia > d {
            return None;
        }

        let mut copia = usize::from(token & 0x0F);
        if copia == 15 {
            loop {
                let b = *origem.get(o)?;
                o += 1;
                copia += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        copia += 4;
        if d + copia > destino.len() {
            return None;
        }

        // Byte a byte de propósito: origem e destino da cópia podem se sobrepor.
        let inicio = d - distancia;
        for m in inicio..inicio + copia {
            destino[d] = destino[m];
            d += 1;
        }
    }
    Some(d)
}
