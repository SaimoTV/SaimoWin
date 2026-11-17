//! Categoria de cada título do acervo: anime, dorama, novela e kids. Vem de
//! `categorias.txt` (gerar_categorias.py, pelo país de origem e o idioma no
//! TMDB). Título fora do arquivo é filme ou série comum.

use std::collections::HashMap;

#[derive(Default)]
pub struct CategoriasVod {
    mapa: HashMap<String, Vec<String>>,
}

impl CategoriasVod {
    pub fn vazio(&self) -> bool {
        self.mapa.is_empty()
    }

    pub fn tem(&self, titulo: &str, serie: bool, categoria: &str) -> bool {
        let p = if serie { "s|" } else { "f|" };
        let achou = self.mapa.get(&format!("{p}{titulo}")).or_else(|| {
            let t = titulo.trim_end();
            let i = t.rfind(" (")?;
            (t.ends_with(')') && t.len() - i == 7).then(|| self.mapa.get(&format!("{p}{}", &t[..i])))?
        });
        achou.is_some_and(|l| l.iter().any(|c| c == categoria))
    }
}

pub fn baixar() -> CategoriasVod {
    let Some(texto) = crate::vod::arquivo_publico("categorias.txt") else {
        return CategoriasVod::default();
    };
    let mut mapa = HashMap::with_capacity(8_000);
    for linha in texto.lines() {
        if linha.starts_with('#') {
            continue;
        }
        let c: Vec<&str> = linha.split('\t').collect();
        if c.len() < 3 {
            continue;
        }
        let cats: Vec<String> = c[2].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if !cats.is_empty() {
            mapa.insert(format!("{}|{}", c[0], c[1]), cats);
        }
    }
    CategoriasVod { mapa }
}
