//! Gerador compacto de QR Code em Rust puro (ISO/IEC 18004, modo Byte,
//! versões 1 a 6, correção de erro nível L — até 134 bytes UTF-8).
//!
//! Sem dependências externas: garante compilação e testes offline em Windows,
//! Linux e macOS para exibir o endereço `http://<ip>:8999/addons` na tela.

use eframe::egui;

/// Matriz quadrada do QR Code (`true` = módulo escuro, `false` = claro).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatrizQr {
    pub lado: usize,
    pub modulos: Vec<bool>,
}

impl MatrizQr {
    /// Lê o módulo na linha `y` e coluna `x`.
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.modulos[y * self.lado + x]
    }

    /// Gera uma `egui::ColorImage` nítida (escala inteira por módulo + borda
    /// silenciosa de 4 módulos) pronta para virar `TextureHandle`.
    pub fn para_imagem(&self, escala: usize) -> egui::ColorImage {
        let escala = escala.max(2);
        let borda = 4usize;
        let total = (self.lado + borda * 2) * escala;
        let escuro = egui::Color32::from_rgb(16, 18, 28);
        let claro = egui::Color32::WHITE;
        let mut pixels = vec![claro; total * total];

        for y in 0..self.lado {
            for x in 0..self.lado {
                if !self.get(x, y) {
                    continue;
                }
                let px0 = (x + borda) * escala;
                let py0 = (y + borda) * escala;
                for dy in 0..escala {
                    let linha = (py0 + dy) * total;
                    for dx in 0..escala {
                        pixels[linha + px0 + dx] = escuro;
                    }
                }
            }
        }
        egui::ColorImage::new([total, total], pixels)
    }
}

/// Especificação de capacidade e blocos EC (Nível L) para versões 1..=6.
struct SpecVersao {
    versao: usize,
    cap_bytes: usize,
    blocos: usize,
    dados_por_bloco: usize,
    ec_por_bloco: usize,
}

const VERSOES_L: &[SpecVersao] = &[
    SpecVersao { versao: 1, cap_bytes: 17, blocos: 1, dados_por_bloco: 19, ec_por_bloco: 7 },
    SpecVersao { versao: 2, cap_bytes: 32, blocos: 1, dados_por_bloco: 34, ec_por_bloco: 10 },
    SpecVersao { versao: 3, cap_bytes: 53, blocos: 1, dados_por_bloco: 55, ec_por_bloco: 15 },
    SpecVersao { versao: 4, cap_bytes: 78, blocos: 1, dados_por_bloco: 80, ec_por_bloco: 20 },
    SpecVersao { versao: 5, cap_bytes: 106, blocos: 1, dados_por_bloco: 108, ec_por_bloco: 26 },
    SpecVersao { versao: 6, cap_bytes: 134, blocos: 2, dados_por_bloco: 68, ec_por_bloco: 18 },
];

/// Codifica `texto` em UTF-8 no menor QR Code (versões 1..=6, nível L).
pub fn gerar(texto: &str) -> Result<MatrizQr, String> {
    let bytes = texto.as_bytes();
    let spec = VERSOES_L
        .iter()
        .find(|s| bytes.len() <= s.cap_bytes)
        .ok_or_else(|| format!("Texto muito longo para QR Code ({} bytes > 134)", bytes.len()))?;

    let total_dados = spec.blocos * spec.dados_por_bloco;
    let dados = codificar_bytes(bytes, total_dados);
    let fluxo = intercalar_com_ec(&dados, spec);

    let lado = 17 + 4 * spec.versao;
    let mut base = vec![false; lado * lado];
    let mut funcao = vec![false; lado * lado];

    desenhar_localizadores(&mut base, &mut funcao, lado, spec.versao);
    desenhar_temporizacao(&mut base, &mut funcao, lado);
    // Módulo escuro fixo à esquerda da assinatura inferior.
    definir_funcao(&mut base, &mut funcao, lado, 8, lado - 8, true);
    // Reserva área de formato antes de preencher os bits de dados.
    reservar_formato(&mut funcao, lado);
    preencher_dados(&mut base, &funcao, lado, &fluxo);

    let mascara = escolher_melhor_mascara(&base, &funcao, lado);
    let mut final_mod = aplicar_mascara(&base, &funcao, lado, mascara);
    desenhar_formato(&mut final_mod, lado, mascara);

    Ok(MatrizQr { lado, modulos: final_mod })
}

fn definir_funcao(modulos: &mut [bool], funcao: &mut [bool], lado: usize, x: usize, y: usize, val: bool) {
    let idx = y * lado + x;
    modulos[idx] = val;
    funcao[idx] = true;
}

fn desenhar_localizador(modulos: &mut [bool], funcao: &mut [bool], lado: usize, cx: usize, cy: usize) {
    for dy in -4isize..=4 {
        for dx in -4isize..=4 {
            let x = cx as isize + dx;
            let y = cy as isize + dy;
            if x < 0 || y < 0 || x >= lado as isize || y >= lado as isize {
                continue;
            }
            let dist = dx.abs().max(dy.abs());
            let escuro = dist != 2 && dist != 4;
            definir_funcao(modulos, funcao, lado, x as usize, y as usize, escuro);
        }
    }
}

fn desenhar_alinhamento(modulos: &mut [bool], funcao: &mut [bool], lado: usize, cx: usize, cy: usize) {
    for dy in -2isize..=2 {
        for dx in -2isize..=2 {
            let x = (cx as isize + dx) as usize;
            let y = (cy as isize + dy) as usize;
            let escuro = dx.abs().max(dy.abs()) != 1;
            definir_funcao(modulos, funcao, lado, x, y, escuro);
        }
    }
}

fn desenhar_localizadores(modulos: &mut [bool], funcao: &mut [bool], lado: usize, versao: usize) {
    desenhar_localizador(modulos, funcao, lado, 3, 3);
    desenhar_localizador(modulos, funcao, lado, lado - 4, 3);
    desenhar_localizador(modulos, funcao, lado, 3, lado - 4);
    if versao >= 2 {
        // Versões 2..6 possuem exatamente 1 padrão de alinhamento em (lado-7, lado-7).
        desenhar_alinhamento(modulos, funcao, lado, lado - 7, lado - 7);
    }
}

fn desenhar_temporizacao(modulos: &mut [bool], funcao: &mut [bool], lado: usize) {
    for i in 8..(lado - 8) {
        let escuro = i % 2 == 0;
        definir_funcao(modulos, funcao, lado, i, 6, escuro);
        definir_funcao(modulos, funcao, lado, 6, i, escuro);
    }
}

fn reservar_formato(funcao: &mut [bool], lado: usize) {
    for i in 0..=8 {
        if i != 6 {
            funcao[8 * lado + i] = true;
            funcao[i * lado + 8] = true;
        }
    }
    for i in (lado - 8)..lado {
        funcao[8 * lado + i] = true;
    }
    for i in (lado - 7)..lado {
        funcao[i * lado + 8] = true;
    }
}

fn codificar_bytes(bytes: &[u8], total_dados: usize) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::with_capacity(total_dados * 8);
    // Modo Byte: 0100 (4 bits) + tamanho em 8 bits (versões 1..9).
    empurrar_bits(&mut bits, 0b0100, 4);
    empurrar_bits(&mut bits, bytes.len() as u32, 8);
    for &b in bytes {
        empurrar_bits(&mut bits, b as u32, 8);
    }
    let limite = total_dados * 8;
    let terminador = (limite - bits.len()).min(4);
    for _ in 0..terminador {
        bits.push(false);
    }
    while bits.len() % 8 != 0 {
        bits.push(false);
    }

    let mut saida = Vec::with_capacity(total_dados);
    for chunk in bits.chunks_exact(8) {
        let mut val = 0u8;
        for &b in chunk {
            val = (val << 1) | (b as u8);
        }
        saida.push(val);
    }
    let mut pad = 0xECu8;
    while saida.len() < total_dados {
        saida.push(pad);
        pad = if pad == 0xEC { 0x11 } else { 0xEC };
    }
    saida
}

fn empurrar_bits(destino: &mut Vec<bool>, valor: u32, n: u8) {
    for i in (0..n).rev() {
        destino.push(((valor >> i) & 1) != 0);
    }
}

fn intercalar_com_ec(dados: &[u8], spec: &SpecVersao) -> Vec<u8> {
    let gen = rs_polinomio(spec.ec_por_bloco);
    let mut blocos_dados: Vec<&[u8]> = Vec::with_capacity(spec.blocos);
    let mut blocos_ec: Vec<Vec<u8>> = Vec::with_capacity(spec.blocos);
    for b in 0..spec.blocos {
        let ini = b * spec.dados_por_bloco;
        let bloco = &dados[ini..ini + spec.dados_por_bloco];
        blocos_dados.push(bloco);
        blocos_ec.push(rs_calcular_resto(bloco, &gen));
    }

    let mut out = Vec::with_capacity((spec.dados_por_bloco + spec.ec_por_bloco) * spec.blocos);
    for i in 0..spec.dados_por_bloco {
        for b in 0..spec.blocos {
            out.push(blocos_dados[b][i]);
        }
    }
    for i in 0..spec.ec_por_bloco {
        for b in 0..spec.blocos {
            out.push(blocos_ec[b][i]);
        }
    }
    out
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut r = 0u8;
    for _ in 0..8 {
        if (b & 1) != 0 {
            r ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1D; // polinômio 0x11D sem o bit x^8
        }
        b >>= 1;
    }
    r
}

fn rs_polinomio(grau: usize) -> Vec<u8> {
    let mut p = vec![0u8; grau];
    p[grau - 1] = 1;
    let mut raiz = 1u8;
    for _ in 0..grau {
        for j in 0..grau {
            p[j] = gf_mul(p[j], raiz);
            if j + 1 < grau {
                p[j] ^= p[j + 1];
            }
        }
        raiz = gf_mul(raiz, 0x02);
    }
    p
}

fn rs_calcular_resto(dados: &[u8], div: &[u8]) -> Vec<u8> {
    let mut resto = vec![0u8; div.len()];
    for &b in dados {
        let fator = b ^ resto[0];
        resto.copy_within(1.., 0);
        *resto.last_mut().unwrap() = 0;
        for (r, &coef) in resto.iter_mut().zip(div.iter()) {
            *r ^= gf_mul(coef, fator);
        }
    }
    resto
}

fn preencher_dados(modulos: &mut [bool], funcao: &[bool], lado: usize, codewords: &[u8]) {
    let mut bit_idx = 0usize;
    let total_bits = codewords.len() * 8;
    let mut subindo = true;
    let mut right = lado as isize - 1;

    while right >= 1 {
        if right == 6 {
            right -= 1;
        }
        for step in 0..lado {
            let y = if subindo { lado - 1 - step } else { step };
            for dx in 0..=1 {
                let x = (right - dx) as usize;
                let idx = y * lado + x;
                if funcao[idx] {
                    continue;
                }
                let bit = if bit_idx < total_bits {
                    let cw = codewords[bit_idx >> 3];
                    ((cw >> (7 - (bit_idx & 7))) & 1) != 0
                } else {
                    false
                };
                modulos[idx] = bit;
                bit_idx += 1;
            }
        }
        subindo = !subindo;
        right -= 2;
    }
}

fn aplicar_mascara(base: &[bool], funcao: &[bool], lado: usize, mascara: u8) -> Vec<bool> {
    let mut out = base.to_vec();
    for y in 0..lado {
        for x in 0..lado {
            let idx = y * lado + x;
            if funcao[idx] {
                continue;
            }
            let inverter = match mascara {
                0 => (x + y) % 2 == 0,
                1 => y % 2 == 0,
                2 => x % 3 == 0,
                3 => (x + y) % 3 == 0,
                4 => (x / 3 + y / 2) % 2 == 0,
                5 => (x * y) % 2 + (x * y) % 3 == 0,
                6 => ((x * y) % 2 + (x * y) % 3) % 2 == 0,
                7 => ((x + y) % 2 + (x * y) % 3) % 2 == 0,
                _ => false,
            };
            if inverter {
                out[idx] = !out[idx];
            }
        }
    }
    out
}

fn bits_formato_l(mascara: u8) -> u16 {
    // Nível L = 0b01; 5 bits de formato = (01 << 3) | mascara.
    let dados = ((0b01u16) << 3) | (mascara as u16 & 0b111);
    let mut rem = dados;
    for _ in 0..10 {
        rem = (rem << 1) ^ ((rem >> 9) * 0x537);
    }
    ((dados << 10) | rem) ^ 0x5412
}

fn desenhar_formato(modulos: &mut [bool], lado: usize, mascara: u8) {
    let bits = bits_formato_l(mascara);
    // Cópia 1 (ao redor do localizador superior esquerdo): bits 0..14 (LSB->MSB).
    for i in 0..=5 {
        modulos[i * lado + 8] = ((bits >> i) & 1) != 0;
    }
    modulos[7 * lado + 8] = ((bits >> 6) & 1) != 0;
    modulos[8 * lado + 8] = ((bits >> 7) & 1) != 0;
    modulos[8 * lado + 7] = ((bits >> 8) & 1) != 0;
    for i in 9..15 {
        modulos[8 * lado + (14 - i)] = ((bits >> i) & 1) != 0;
    }
    // Cópia 2 (superior direito horizontal e inferior esquerdo vertical).
    for i in 0..8 {
        modulos[8 * lado + (lado - 1 - i)] = ((bits >> i) & 1) != 0;
    }
    for i in 8..15 {
        modulos[(lado - 15 + i) * lado + 8] = ((bits >> i) & 1) != 0;
    }
}

fn escolher_melhor_mascara(base: &[bool], funcao: &[bool], lado: usize) -> u8 {
    let mut melhor = 0u8;
    let mut menor_pen = u32::MAX;
    for m in 0..8u8 {
        let mut candidato = aplicar_mascara(base, funcao, lado, m);
        desenhar_formato(&mut candidato, lado, m);
        let pen = penalidade(&candidato, lado);
        if pen < menor_pen {
            menor_pen = pen;
            melhor = m;
        }
    }
    melhor
}

fn penalidade(m: &[bool], lado: usize) -> u32 {
    let mut pontos = 0u32;
    let at = |x: usize, y: usize| m[y * lado + x];

    // N1: 5+ módulos iguais em sequência por linha e por coluna.
    for y in 0..lado {
        pontos += penalidade_linha((0..lado).map(|x| at(x, y)));
    }
    for x in 0..lado {
        pontos += penalidade_linha((0..lado).map(|y| at(x, y)));
    }
    // N2: blocos 2x2 da mesma cor.
    for y in 0..(lado - 1) {
        for x in 0..(lado - 1) {
            let c = at(x, y);
            if at(x + 1, y) == c && at(x, y + 1) == c && at(x + 1, y + 1) == c {
                pontos += 3;
            }
        }
    }
    // N3: padrão parecido com localizador (1011101 + 4 claros antes ou depois).
    for y in 0..lado {
        pontos += penalidade_finder((0..lado).map(|x| at(x, y)));
    }
    for x in 0..lado {
        pontos += penalidade_finder((0..lado).map(|y| at(x, y)));
    }
    // N4: proporção de módulos escuros afastada de 50%.
    let escuros = m.iter().filter(|&&v| v).count() as i32;
    let total = (lado * lado) as i32;
    let k = ((escuros * 20 - total * 10).abs() + total - 1) / total - 1;
    pontos += (k.max(0) as u32) * 10;
    pontos
}

fn penalidade_linha(iter: impl Iterator<Item = bool>) -> u32 {
    let mut pen = 0u32;
    let mut ultimo = None;
    let mut run = 0u32;
    for b in iter {
        if Some(b) == ultimo {
            run += 1;
            if run == 5 {
                pen += 3;
            } else if run > 5 {
                pen += 1;
            }
        } else {
            ultimo = Some(b);
            run = 1;
        }
    }
    pen
}

fn penalidade_finder(iter: impl Iterator<Item = bool>) -> u32 {
    let linha: Vec<bool> = iter.collect();
    if linha.len() < 11 {
        return 0;
    }
    const A: [bool; 11] = [false, false, false, false, true, false, true, true, true, false, true];
    const B: [bool; 11] = [true, false, true, true, true, false, true, false, false, false, false];
    let mut pen = 0u32;
    for w in linha.windows(11) {
        if w == A || w == B {
            pen += 40;
        }
    }
    pen
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Reconstrói o fluxo de codewords de uma matriz gerada e confere a
    /// integridade dos dados codificados + síndromes Reed-Solomon == 0.
    fn extrair_e_verificar(matriz: &MatrizQr, spec: &SpecVersao) -> Vec<u8> {
        let lado = matriz.lado;
        let mut funcao = vec![false; lado * lado];
        let mut dummy = vec![false; lado * lado];
        desenhar_localizadores(&mut dummy, &mut funcao, lado, spec.versao);
        desenhar_temporizacao(&mut dummy, &mut funcao, lado);
        definir_funcao(&mut dummy, &mut funcao, lado, 8, lado - 8, true);
        reservar_formato(&mut funcao, lado);

        // Lê bits de formato da cópia 1 e confere com cópia 2.
        let mut fmt1 = 0u16;
        for i in 0..=5 {
            fmt1 |= (matriz.get(8, i) as u16) << i;
        }
        fmt1 |= (matriz.get(8, 7) as u16) << 6;
        fmt1 |= (matriz.get(8, 8) as u16) << 7;
        fmt1 |= (matriz.get(7, 8) as u16) << 8;
        for i in 9..15 {
            fmt1 |= (matriz.get(14 - i, 8) as u16) << i;
        }
        let mut fmt2 = 0u16;
        for i in 0..8 {
            fmt2 |= (matriz.get(lado - 1 - i, 8) as u16) << i;
        }
        for i in 8..15 {
            fmt2 |= (matriz.get(8, lado - 15 + i) as u16) << i;
        }
        assert_eq!(fmt1, fmt2, "cópias do formato diferem");
        let mascara = (((fmt1 ^ 0x5412) >> 10) & 0b111) as u8;
        let desmascarado = aplicar_mascara(&matriz.modulos, &funcao, lado, mascara);

        // Extrai bits em ordem zig-zag e agrupa em bytes.
        let total_codewords = (spec.dados_por_bloco + spec.ec_por_bloco) * spec.blocos;
        let mut bytes = vec![0u8; total_codewords];
        let mut bit_idx = 0usize;
        let mut subindo = true;
        let mut right = lado as isize - 1;
        while right >= 1 {
            if right == 6 {
                right -= 1;
            }
            for step in 0..lado {
                let y = if subindo { lado - 1 - step } else { step };
                for dx in 0..=1 {
                    let x = (right - dx) as usize;
                    let idx = y * lado + x;
                    if funcao[idx] {
                        continue;
                    }
                    if bit_idx < total_codewords * 8 && desmascarado[idx] {
                        bytes[bit_idx >> 3] |= 1 << (7 - (bit_idx & 7));
                    }
                    bit_idx += 1;
                }
            }
            subindo = !subindo;
            right -= 2;
        }

        // Desintercala blocos e valida resto Reed-Solomon == 0.
        let gen = rs_polinomio(spec.ec_por_bloco);
        let mut dados_juntos = Vec::new();
        for b in 0..spec.blocos {
            let mut bloco_dados = Vec::with_capacity(spec.dados_por_bloco);
            for i in 0..spec.dados_por_bloco {
                bloco_dados.push(bytes[i * spec.blocos + b]);
            }
            let base_ec = spec.dados_por_bloco * spec.blocos;
            let mut bloco_ec = Vec::with_capacity(spec.ec_por_bloco);
            for i in 0..spec.ec_por_bloco {
                bloco_ec.push(bytes[base_ec + i * spec.blocos + b]);
            }
            assert_eq!(rs_calcular_resto(&bloco_dados, &gen), bloco_ec);
            dados_juntos.extend_from_slice(&bloco_dados);
        }
        dados_juntos
    }

    #[test]
    fn qr_url_addons_valida_localizadores_e_reed_solomon() {
        let casos = [
            ("http://192.168.1.42:8999/addons", 25usize, 1usize),
            ("http://192.168.100.142:8999/addons", 29usize, 2usize),
        ];
        for (url, lado_esperado, spec_idx) in casos {
            let qr = gerar(url).expect("QR da URL local deve gerar sem erro");
            assert_eq!(qr.lado, lado_esperado);
            // Confere canto superior esquerdo do localizador 7x7 + módulo escuro fixo.
            assert!(qr.get(0, 0));
            assert!(!qr.get(1, 1));
            assert!(qr.get(3, 3));
            assert!(!qr.get(7, 7)); // separador
            assert!(qr.get(8, qr.lado - 8)); // dark module

            let dados = extrair_e_verificar(&qr, &VERSOES_L[spec_idx]);
            // Modo Byte (0100) + tamanho em bytes no início do bitstream.
            let modo = dados[0] >> 4;
            let len = ((dados[0] & 0x0F) << 4) | (dados[1] >> 4);
            assert_eq!(modo, 0b0100);
            assert_eq!(len as usize, url.len());
            let mut texto_decodificado = Vec::with_capacity(len as usize);
            for i in 0..len as usize {
                let byte = ((dados[1 + i] & 0x0F) << 4) | (dados[2 + i] >> 4);
                texto_decodificado.push(byte);
            }
            assert_eq!(String::from_utf8(texto_decodificado).unwrap(), url);

            let img = qr.para_imagem(4);
            assert_eq!(img.size, [(lado_esperado + 8) * 4, (lado_esperado + 8) * 4]);
        }
    }

    #[test]
    fn qr_dois_blocos_v6_tambem_fecha_reed_solomon() {
        let url = "http://192.168.100.200:8999/addons?token=exemplo-muito-longo-para-forcar-versao-6-com-dois-blocos-de-correcao-e-intercalacao-completa";
        assert!(url.len() > 106 && url.len() <= 134);
        let qr = gerar(url).unwrap();
        assert_eq!(qr.lado, 41);
        let _ = extrair_e_verificar(&qr, &VERSOES_L[5]);
    }
}
