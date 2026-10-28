//! O gênero de cada título: Ação, Terror, Animação, Comédia.
//!
//! O catálogo não tem gênero — as listas de origem trazem nome e endereço,
//! nada mais. Perguntar ao TMDB por trinta e quatro mil títulos, no aparelho,
//! a cada abertura, é uma tela que nunca abre.
//!
//! A pergunta é feita uma vez no repositório (`gerar_generos.py`) e chega aqui
//! pronta, no mesmo arquivo que o Mac, a TV Box e o site leem.
//!
//! Sem o arquivo, a régua de gêneros não aparece: oferecer um filtro que
//! devolve vazio é pior que não oferecer.

use std::collections::{BTreeSet, HashMap};

/// O que o gerador publicou, já lido.
#[derive(Default)]
pub struct Generos {
    /// "f|Nome" ou "s|Nome" -> os gêneros dele.
    mapa: HashMap<String, Vec<String>>,
    /// "f|Nome" ou "s|Nome" -> o endereço do pôster, quando o TMDB conhece.
    capas: HashMap<String, String>,
    /// "f|Nome" ou "s|Nome" -> o id do TMDB. Com ele, a ficha completa de um
    /// título é um pedido só, sem busca por nome nem desempate.
    ids: HashMap<String, u32>,
    /// O caminho inverso: id do TMDB -> título do acervo. É assim que a
    /// filmografia de um ator vira uma lista clicável — só entra o que existe
    /// aqui dentro. Séries entram com o id negativo, para não colidir com o
    /// filme de mesmo número.
    por_id: HashMap<i64, String>,
    /// Todos os que aparecem no acervo, em ordem.
    pub todos: Vec<String>,
}

impl Generos {
    pub fn vazio(&self) -> bool {
        self.mapa.is_empty()
    }

    /// Os gêneros de um título.
    ///
    /// A chave é o nome como o acervo o escreve — e o acervo escreve o ano
    /// dentro do nome do filme, mas guarda o da série num campo à parte. Quem
    /// chama nem sempre sabe de qual dos dois veio, então procura-se o nome
    /// como ele chegou e, não achando, sem o ano.
    fn chaves_candidatas(&self, marca: &str, titulo: &str) -> Vec<String> {
        let s_ano = sem_ano(titulo);
        let mut chaves = Vec::new();
        let base_titulos = [titulo, &s_ano];
        for t in base_titulos {
            if t.is_empty() { continue; }
            chaves.push(format!("{marca}|{t}"));
            if t.contains(',') {
                let com_espaco = t.replace(',', ", ");
                let normalizado_com_espaco = com_espaco.split_whitespace().collect::<Vec<_>>().join(" ");
                chaves.push(format!("{marca}|{normalizado_com_espaco}"));
                let sem_espaco = t.replace(", ", ",");
                chaves.push(format!("{marca}|{sem_espaco}"));
            }
            if t.contains(':') {
                let com_espaco = t.replace(':', ": ");
                let normalizado = com_espaco.split_whitespace().collect::<Vec<_>>().join(" ");
                chaves.push(format!("{marca}|{normalizado}"));
                let sem_espaco = t.replace(": ", ":");
                chaves.push(format!("{marca}|{sem_espaco}"));
            }
        }
        chaves
    }

    pub fn de(&self, titulo: &str, serie: bool) -> &[String] {
        let marca = if serie { "s" } else { "f" };
        for c in self.chaves_candidatas(marca, titulo) {
            if let Some(lista) = self.mapa.get(&c) {
                return lista;
            }
        }
        &[]
    }

    pub fn tem(&self, titulo: &str, serie: bool, genero: &str) -> bool {
        genero.is_empty() || self.de(titulo, serie).iter().any(|g| g == genero)
    }

    /// O pôster de um título, pelo id que o gerador já resolveu.
    ///
    /// Antes a capa era procurada pelo nome no TMDB, a cada abertura: lento, e
    /// errado quando dois filmes se chamam igual. Quem não tem ficha fica sem
    /// capa, e a tela põe uma marca no lugar.
    /// O id do TMDB de um título, quando o gerador o resolveu.
    pub fn id(&self, titulo: &str, serie: bool) -> Option<u32> {
        let marca = if serie { "s" } else { "f" };
        for c in self.chaves_candidatas(marca, titulo) {
            if let Some(&id) = self.ids.get(&c) {
                return Some(id);
            }
        }
        None
    }

    /// O título do acervo que corresponde a um id do TMDB, se houver.
    pub fn titulo_de(&self, id: u32, serie: bool) -> Option<&str> {
        let marca = if serie { -(id as i64) } else { id as i64 };
        self.por_id.get(&marca).map(String::as_str)
    }

    pub fn capa(&self, titulo: &str, serie: bool) -> Option<&str> {
        let marca = if serie { "s" } else { "f" };
        for c in self.chaves_candidatas(marca, titulo) {
            if let Some(c_str) = self.capas.get(&c) {
                return Some(c_str.as_str());
            }
        }
        None
    }
}

/// Busca a lista. Chamada de uma thread: fala com a rede.
pub fn baixar() -> Generos {
    let Some(texto) = crate::vod::arquivo_publico("fichas.txt") else {
        return Generos::default();
    };
    ler(&texto)
}

fn ler(texto: &str) -> Generos {
    let mut mapa = HashMap::new();
    let mut capas = HashMap::new();
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut por_id: HashMap<i64, String> = HashMap::new();
    let mut vistos = BTreeSet::new();
    let mut base = String::new();
    // tipo \t título \t id do TMDB \t pôster \t gêneros
    for linha in texto.lines() {
        if let Some(resto) = linha.strip_prefix("capa:") {
            base = resto.trim().to_string();
            continue;
        }
        if linha.starts_with('#') {
            continue;
        }
        let campos: Vec<&str> = linha.split('\t').collect();
        if campos.len() < 5 {
            continue;
        }
        let chave = format!("{}|{}", campos[0], campos[1]);
        if !campos[3].trim().is_empty() {
            capas.insert(chave.clone(), format!("{base}{}", campos[3]));
        }
        if let Ok(id) = campos[2].trim().parse::<u32>() {
            if id > 0 {
                ids.insert(chave.clone(), id);
                let marca = if campos[0] == "s" { -(id as i64) } else { id as i64 };
                // Um mesmo id pode aparecer duas vezes no acervo (o mesmo
                // filme em duas grafias); o primeiro basta.
                por_id.entry(marca).or_insert_with(|| campos[1].to_string());
            }
        }
        let lista: Vec<String> = campos[4]
            .split(',')
            .map(str::trim)
            .filter(|g| !g.is_empty())
            .map(str::to_string)
            .collect();
        if lista.is_empty() {
            continue;
        }
        for g in &lista {
            vistos.insert(g.clone());
        }
        mapa.insert(chave, lista);
    }
    Generos { mapa, capas, ids, por_id, todos: vistos.into_iter().collect() }
}

/// O título sem o ano final, que é como a lista de gêneros o guarda.
pub fn sem_ano(titulo: &str) -> String {
    let cortado = titulo.trim_end();
    // O ano são 6 caracteres ASCII, "(2026)": conferido pelos bytes do final,
    // sem fatiar o texto (um título como "Fulano (Ação)" tem acento nessa ponta
    // e fatiar no byte errado derrubava o app inteiro ao abrir o On Demand).
    let b = cortado.as_bytes();
    if b.len() > 6 {
        let cauda = &b[b.len() - 6..];
        if cauda[0] == b'(' && cauda[5] == b')' && cauda[1..5].iter().all(u8::is_ascii_digit) {
            // Os 6 últimos bytes são ASCII, então este corte é sempre uma fronteira.
            return cortado[..cortado.len() - 6].trim_end().to_string();
        }
    }
    cortado.to_string()
}

#[cfg(test)]
mod testes_sem_ano {
    use super::sem_ano;

    #[test]
    fn tira_o_ano_do_fim() {
        assert_eq!(sem_ano("Duna: Parte Dois (2024)"), "Duna: Parte Dois");
        assert_eq!(sem_ano("Moana (2016)  "), "Moana");
    }

    #[test]
    fn titulo_com_acento_na_ponta_nao_derruba() {
        // Os 6 últimos bytes caem no meio do "ç"/"é": fatiar ali dava pânico.
        assert_eq!(sem_ano("Fulano (Ação)"), "Fulano (Ação)");
        assert_eq!(sem_ano("Lupin (Série)"), "Lupin (Série)");
        assert_eq!(sem_ano("Cidade de Deus (Nacional)"), "Cidade de Deus (Nacional)");
        assert_eq!(sem_ano("Ô (é)"), "Ô (é)");
        assert_eq!(sem_ano("(ãéíóú)"), "(ãéíóú)");
    }

    #[test]
    fn nao_confunde_parenteses_que_nao_sao_ano() {
        assert_eq!(sem_ano("Algo (20x6)"), "Algo (20x6)");
        assert_eq!(sem_ano("(2026)"), "(2026)");
        assert_eq!(sem_ano(""), "");
    }
}
