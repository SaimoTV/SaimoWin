//! Ano, nota, classificação indicativa e popularidade de cada título, para os
//! filtros da grade do acervo. Vem de `detalhes.txt` (gerar_detalhes.py, do
//! TMDB); sem ele, os filtros de ano, idade e nota não aparecem.

use std::collections::HashMap;

#[derive(Clone, Copy, Default)]
pub struct Info {
    pub ano: u32,
    pub nota: f32,
    /// 0 = Livre, 10 a 18 = idade; -1 = não informada.
    pub idade: i8,
    pub popularidade: u32,
}

#[derive(Default)]
pub struct InfoDosTitulos {
    mapa: HashMap<String, Info>,
}

impl InfoDosTitulos {
    pub fn vazio(&self) -> bool {
        self.mapa.is_empty()
    }

    pub fn de(&self, titulo: &str, serie: bool) -> Option<Info> {
        let p = if serie { "s|" } else { "f|" };
        if let Some(i) = self.mapa.get(&format!("{p}{titulo}")) {
            return Some(*i);
        }
        let sem_ano = titulo.trim_end();
        let sem_ano = match sem_ano.rfind(" (") {
            Some(i) if sem_ano.ends_with(')') && sem_ano.len() - i == 7 => &sem_ano[..i],
            _ => return None,
        };
        self.mapa.get(&format!("{p}{sem_ano}")).copied()
    }
}

pub fn baixar() -> InfoDosTitulos {
    let Some(texto) = crate::vod::arquivo_publico("detalhes.txt") else {
        return InfoDosTitulos::default();
    };
    let mut mapa = HashMap::with_capacity(40_000);
    for linha in texto.lines() {
        if linha.starts_with('#') {
            continue;
        }
        let c: Vec<&str> = linha.split('\t').collect();
        if c.len() < 6 {
            continue;
        }
        let idade = match c[4] {
            "L" => 0,
            "" => -1,
            n => n.parse().unwrap_or(-1),
        };
        mapa.insert(format!("{}|{}", c[0], c[1]), Info {
            ano: c[2].parse().unwrap_or(0),
            nota: c[3].parse().unwrap_or(0.0),
            idade,
            popularidade: c[5].parse().unwrap_or(0),
        });
    }
    InfoDosTitulos { mapa }
}
