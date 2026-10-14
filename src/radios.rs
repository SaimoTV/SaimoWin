//! As rádios: `vod/radios.txt` do SaimoPlayer, a mesma lista da TV Box.
//!
//! Uma por linha, `nome|endereço|logo` (o logo é opcional). A tela mostra na
//! hora a última lista guardada em disco e busca a do repositório por trás,
//! como a TV Box: quem já tinha uma lista também recebe as rádios novas.
//!
//! O estilo (rock, notícias, gospel...) sai do nome da rádio, com as mesmas
//! palavras da TV Box: a lista publicada só tem nome, endereço e logo.

const ENDERECO: &str = "https://raw.githubusercontent.com/gabrielsaimo/SaimoPlayer/main/vod/radios.txt";
const ARQUIVO: &str = "radios.txt";
const RECENTES: &str = "radios-recentes.txt";

#[derive(Clone, Debug, PartialEq)]
pub struct Radio {
    pub nome: String,
    pub url: String,
    pub logo: Option<String>,
}

/// Os estilos na ordem da TV Box: rádio que cabe em dois fica no primeiro.
pub const ESTILOS: [(&str, &[&str]); 6] = [
    ("Rock", &["rock", "metal", "kiss", "punk"]),
    ("Notícias e esporte", &["news", "noticia", "cbn", "gaucha", "itatiaia", "tupi", "esporte", "jornal",
        "bandnews", "radio globo", "eldorado", "bandeirantes"]),
    ("Gospel e católicas", &["gospel", "evangel", "catolic", "crista", "louvor", "aleluia", "cancao nova",
        "novo tempo", "aparecida", "fe ", "deus", "jesus", "igreja", "biblia"]),
    ("Pop e hits", &["pop", "hits", "dance", "mix", "jovem pan", "transamerica", "energia", "eletro", "club",
        "pulse", "top"]),
    ("Sertanejo, MPB e samba", &["sertanej", "mpb", "samba", "pagode", "forro", "nativa", "caipira", "brega",
        "axe", "piseiro", "moda de viola"]),
    ("Clássica e jazz", &["classic", "jazz", "cultura", "educativa", "universit", "instrumental", "blues", "lounge"]),
];

/// Nome sem acento e em minúsculas, para achar o estilo e a letra.
pub fn chave(nome: &str) -> String {
    crate::catalogo::chave_de_ordem(nome)
}

/// O estilo da rádio, pelo nome dela.
pub fn estilo_de(nome: &str) -> Option<&'static str> {
    let nome = format!("{} ", chave(nome));
    ESTILOS.iter().find(|(_, palavras)| palavras.iter().any(|p| nome.contains(p))).map(|(e, _)| *e)
}

pub fn ler(texto: &str) -> Vec<Radio> {
    let mut vistas = std::collections::HashSet::new();
    texto
        .lines()
        .filter_map(|linha| {
            let mut partes = linha.split('|');
            let nome = partes.next()?.trim().to_string();
            let url = partes.next()?.trim().to_string();
            if nome.is_empty() || !url.starts_with("http") {
                return None;
            }
            let logo = partes.next().map(str::trim).filter(|l| l.starts_with("http")).map(str::to_string);
            Some(Radio { nome, url, logo })
        })
        .filter(|r| vistas.insert(r.nome.clone()))
        .collect()
}

/// A última lista baixada, sem rede.
pub fn locais() -> Vec<Radio> {
    std::fs::read_to_string(crate::catalogo::pasta().join(ARQUIVO)).map(|t| ler(&t)).unwrap_or_default()
}

/// A lista do repositório, guardada para a próxima vez. Nada quando a rede
/// falhou ou veio outra coisa: aí vale a local.
pub fn baixar() -> Option<Vec<Radio>> {
    let texto = crate::rede::texto(ENDERECO)?;
    let lista = ler(&texto);
    if lista.len() < 10 {
        return None;
    }
    let _ = std::fs::write(crate::catalogo::pasta().join(ARQUIVO), &texto);
    Some(lista)
}

/// As ouvidas por último, da mais recente para a mais antiga.
pub fn recentes() -> Vec<String> {
    std::fs::read_to_string(crate::catalogo::pasta().join(RECENTES))
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect())
        .unwrap_or_default()
}

pub fn ouviu(nome: &str) {
    let mut lista = recentes();
    lista.retain(|n| n != nome);
    lista.insert(0, nome.to_string());
    lista.truncate(12);
    let _ = std::fs::write(crate::catalogo::pasta().join(RECENTES), lista.join("\n"));
}

/// A letra da fileira de A a Z: números e símbolos juntos, antes do A.
pub fn letra_de(nome: &str) -> char {
    chave(nome)
        .chars()
        .next()
        .filter(|c| c.is_ascii_lowercase())
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('#')
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_a_lista_publicada() {
        let lista = ler("012 Rock News|https://a/stream|https://l.png\nSem url|ftp://x\n87 FM|https://b/;\n87 FM|https://c\n");
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].logo.as_deref(), Some("https://l.png"));
        assert_eq!(lista[1].logo, None);
    }

    #[test]
    fn estilo_pelo_nome() {
        assert_eq!(estilo_de("Kiss FM"), Some("Rock"));
        assert_eq!(estilo_de("Rádio Gaúcha"), Some("Notícias e esporte"));
        assert_eq!(estilo_de("Canção Nova"), Some("Gospel e católicas"));
        assert_eq!(estilo_de("Rádio Qualquer"), None);
        assert_eq!(letra_de("87 FM"), '#');
        assert_eq!(letra_de("Água FM"), 'A');
    }
}
