//! O WebSocket do Saimo Connect, escrito à mão (RFC 6455), como o da TV Box.
//!
//! São poucas peças: o aperto de mão (SHA-1 + base64 da chave do celular), o
//! quadro de texto que a TV manda e a leitura dos quadros mascarados que o
//! celular manda. Nada disso justifica trazer uma biblioteca.

use std::io::Read;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// Teto de um quadro do celular: as mensagens dele têm poucas centenas de bytes.
const TETO: u64 = 1024 * 1024;

/// O `Sec-WebSocket-Accept` para a chave que o celular mandou.
pub fn aceite(chave: &str) -> String {
    base64(&sha1(format!("{}{GUID}", chave.trim()).as_bytes()))
}

/// Um quadro de texto da TV para o celular (sem máscara, que é só do cliente).
pub fn quadro_de_texto(texto: &str) -> Vec<u8> {
    quadro(0x1, texto.as_bytes())
}

pub fn quadro(opcode: u8, dados: &[u8]) -> Vec<u8> {
    let mut saida = Vec::with_capacity(dados.len() + 10);
    saida.push(0x80 | opcode);
    let n = dados.len();
    if n <= 125 {
        saida.push(n as u8);
    } else if n <= 65535 {
        saida.push(126);
        saida.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        saida.push(127);
        saida.extend_from_slice(&(n as u64).to_be_bytes());
    }
    saida.extend_from_slice(dados);
    saida
}

/// O que chegou do celular.
#[derive(Debug, PartialEq)]
pub enum Mensagem {
    Texto(String),
    Ping(Vec<u8>),
    /// O celular encerrou; o conteúdo (o código de saída) volta no aceite.
    Fechar(Vec<u8>),
    /// Pong, binário: nada a fazer.
    Outro,
}

/// Lê as mensagens de um celular. Guarda a mensagem fragmentada entre uma
/// leitura e outra: um ping pode chegar no meio dela.
#[derive(Default)]
pub struct Leitor {
    juntando: Option<(u8, Vec<u8>)>,
}

impl Leitor {
    /// Lê uma mensagem inteira (juntando os pedaços de uma mensagem fragmentada).
    /// Erro quando a conexão caiu ou o quadro não faz sentido.
    pub fn ler(&mut self, entrada: &mut impl Read) -> std::io::Result<Mensagem> {
        let juntando = &mut self.juntando;
        loop {
            let mut cabeca = [0u8; 2];
            entrada.read_exact(&mut cabeca)?;
            let fim = cabeca[0] & 0x80 != 0;
            let opcode = cabeca[0] & 0x0F;
            let mascarado = cabeca[1] & 0x80 != 0;
            let mut tamanho = (cabeca[1] & 0x7F) as u64;
            if tamanho == 126 {
                let mut b = [0u8; 2];
                entrada.read_exact(&mut b)?;
                tamanho = u16::from_be_bytes(b) as u64;
            } else if tamanho == 127 {
                let mut b = [0u8; 8];
                entrada.read_exact(&mut b)?;
                tamanho = u64::from_be_bytes(b);
            }
            if tamanho > TETO {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "quadro grande demais"));
            }
            let mut mascara = [0u8; 4];
            if mascarado {
                entrada.read_exact(&mut mascara)?;
            }
            let mut dados = vec![0u8; tamanho as usize];
            entrada.read_exact(&mut dados)?;
            if mascarado {
                for (i, b) in dados.iter_mut().enumerate() {
                    *b ^= mascara[i % 4];
                }
            }
            match opcode {
                // Controle pode chegar no meio de uma mensagem fragmentada.
                0x8 => return Ok(Mensagem::Fechar(dados)),
                0x9 => return Ok(Mensagem::Ping(dados)),
                0xA => {
                    return Ok(Mensagem::Outro);
                }
                0x0 => {
                    let Some((_, acumulado)) = juntando.as_mut() else {
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "continuação solta"));
                    };
                    acumulado.extend_from_slice(&dados);
                    if acumulado.len() as u64 > TETO {
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "mensagem grande demais"));
                    }
                    if fim {
                        let (tipo, tudo) = juntando.take().unwrap();
                        return Ok(montar(tipo, tudo));
                    }
                }
                tipo => {
                    if fim {
                        return Ok(montar(tipo, dados));
                    }
                    *juntando = Some((tipo, dados));
                }
            }
        }
    }
}

fn montar(tipo: u8, dados: Vec<u8>) -> Mensagem {
    if tipo == 0x1 {
        Mensagem::Texto(String::from_utf8_lossy(&dados).into_owned())
    } else {
        Mensagem::Outro
    }
}

/// SHA-1 (FIPS 180-1). Só serve ao aperto de mão: não é para segurança.
pub fn sha1(dados: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = dados.to_vec();
    let bits = (dados.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for bloco in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([bloco[4 * i], bloco[4 * i + 1], bloco[4 * i + 2], bloco[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut saida = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        saida[4 * i..4 * i + 4].copy_from_slice(&v.to_be_bytes());
    }
    saida
}

pub fn base64(dados: &[u8]) -> String {
    const LETRAS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut saida = String::with_capacity(dados.len().div_ceil(3) * 4);
    for trio in dados.chunks(3) {
        let n = (trio[0] as u32) << 16 | (*trio.get(1).unwrap_or(&0) as u32) << 8 | *trio.get(2).unwrap_or(&0) as u32;
        saida.push(LETRAS[(n >> 18) as usize & 63] as char);
        saida.push(LETRAS[(n >> 12) as usize & 63] as char);
        saida.push(if trio.len() > 1 { LETRAS[(n >> 6) as usize & 63] as char } else { '=' });
        saida.push(if trio.len() > 2 { LETRAS[n as usize & 63] as char } else { '=' });
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aceite_do_exemplo_da_rfc() {
        // RFC 6455, seção 1.3.
        assert_eq!(aceite("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn sha1_e_base64_conhecidos() {
        assert_eq!(base64(&sha1(b"abc")), "qZk+NkcGgWq6PiVxeFDCbJzQ2J0=");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }

    fn mascarar(opcode_fim: u8, dados: &[u8]) -> Vec<u8> {
        let mascara = [0x37, 0xfa, 0x21, 0x3d];
        let mut q = vec![opcode_fim];
        if dados.len() <= 125 {
            q.push(0x80 | dados.len() as u8);
        } else {
            q.push(0x80 | 126);
            q.extend_from_slice(&(dados.len() as u16).to_be_bytes());
        }
        q.extend_from_slice(&mascara);
        q.extend(dados.iter().enumerate().map(|(i, b)| b ^ mascara[i % 4]));
        q
    }

    #[test]
    fn le_texto_mascarado_e_fragmentado() {
        let mut fluxo = mascarar(0x81, b"Hello");
        let longo = "x".repeat(300);
        fluxo.extend(mascarar(0x01, &longo.as_bytes()[..100]));
        fluxo.extend(mascarar(0x89, b"oi"));
        fluxo.extend(mascarar(0x80, &longo.as_bytes()[100..]));
        fluxo.extend(mascarar(0x88, b""));
        let mut fluxo_lido = std::io::Cursor::new(fluxo);
        let mut leitor = Leitor::default();
        assert_eq!(leitor.ler(&mut fluxo_lido).unwrap(), Mensagem::Texto("Hello".into()));
        // O ping no meio da mensagem fragmentada sai primeiro, e ela continua.
        assert_eq!(leitor.ler(&mut fluxo_lido).unwrap(), Mensagem::Ping(b"oi".to_vec()));
        assert_eq!(leitor.ler(&mut fluxo_lido).unwrap(), Mensagem::Texto(longo));
        assert_eq!(leitor.ler(&mut fluxo_lido).unwrap(), Mensagem::Fechar(Vec::new()));
        assert!(leitor.ler(&mut fluxo_lido).is_err());
    }

    #[test]
    fn quadro_longo_usa_tamanho_de_16_bits() {
        let q = quadro_de_texto(&"a".repeat(300));
        assert_eq!(q[0], 0x81);
        assert_eq!(q[1], 126);
        assert_eq!(u16::from_be_bytes([q[2], q[3]]), 300);
        assert_eq!(q.len(), 304);
    }
}
