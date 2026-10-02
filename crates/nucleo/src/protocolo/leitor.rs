//! Cursor little-endian com checagem de limite. Pacote truncado vira Err com o motivo.

use super::varint;

pub type Resultado<T> = Result<T, String>;

pub struct LeitorPacote<'a> {
    dados: &'a [u8],
    pub posicao: usize,
}

impl<'a> LeitorPacote<'a> {
    pub fn novo(dados: &'a [u8], posicao: usize) -> Self {
        Self { dados, posicao }
    }

    pub fn restante(&self) -> usize {
        self.dados.len() - self.posicao
    }

    pub fn ler_u8(&mut self) -> Resultado<u8> {
        Ok(self.ler_bytes(1)?[0])
    }

    pub fn ler_u16(&mut self) -> Resultado<u16> {
        let b = self.ler_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn ler_u32(&mut self) -> Resultado<u32> {
        let b = self.ler_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn ler_varint(&mut self) -> Resultado<u64> {
        let (v, n) = varint::ler(self.dados, self.posicao)
            .ok_or_else(|| format!("Varint inválido na posição {}", self.posicao))?;
        self.posicao += n;
        Ok(v)
    }

    pub fn ler_bytes(&mut self, quantidade: usize) -> Resultado<&'a [u8]> {
        if self.restante() < quantidade {
            return Err(format!(
                "Pacote truncado: precisa de {quantidade} bytes, restam {} (posição {})",
                self.restante(),
                self.posicao
            ));
        }
        let trecho = &self.dados[self.posicao..self.posicao + quantidade];
        self.posicao += quantidade;
        Ok(trecho)
    }

    pub fn pular(&mut self, quantidade: usize) -> Resultado<()> {
        self.ler_bytes(quantidade).map(|_| ())
    }
}
