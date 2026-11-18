//! Canal, filme ou episódio: a classificação dos itens de uma lista M3U.
//! Contrato e regras em SaimoPlayer/docs/listas-m3u.md; a referência é
//! SaimoPlayer/scripts/classificar_m3u.py e o gabarito comum fica em
//! tests/dados/classificar.m3u (+ .esperado.json).

const VIDEO: [&str; 14] = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "3gp", "divx", "ogv", "vob"];
const TIPO_CANAL: [&str; 4] = ["live", "channel", "tv", "radio"];
const TIPO_FILME: [&str; 5] = ["movie", "movies", "vod", "film", "filme"];
const TIPO_SERIE: [&str; 5] = ["series", "serie", "tvshow", "show", "episode"];
const SEG_FILME: [&str; 4] = ["movie", "movies", "vod", "filmes"];
const SEG_SERIE: [&str; 2] = ["series", "serie"];
const PALAVRAS_ACERVO: [&str; 15] = [
    "filme", "filmes", "movie", "movies", "vod", "serie", "series", "novela", "novelas", "anime", "animes", "dorama",
    "doramas", "temporada", "temporadas",
];
const SEPARADORES: &[char] = &[' ', '-', ':', '|', '.', '_', '(', '[', '\u{2013}', '\u{2014}', '\t'];
const TAGS_LEG: [&str; 4] = ["l", "leg", "legendado", "legendada"];
const TAGS_DUB: [&str; 6] = ["d", "dub", "dublado", "dublada", "nac", "nacional"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marca {
    pub temporada: u32,
    pub episodio: u32,
    /// Em caracteres (não bytes) do nome.
    pub inicio: usize,
    pub forte: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo {
    Canal,
    Filme,
    Episodio(Marca),
}

fn sem_acento(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        outro => outro,
    }
}

/// Minúsculas sem acento, um caractere para cada caractere do original.
pub fn dobrar(t: &str) -> Vec<char> {
    t.chars().map(|c| sem_acento(c.to_lowercase().next().unwrap_or(c))).collect()
}

fn dobrar_texto(t: &str) -> String {
    dobrar(t).into_iter().collect()
}

fn alnum(c: char) -> bool {
    c.is_alphanumeric()
}

fn inicio_de_palavra(n: &[char], i: usize) -> bool {
    i == 0 || !alnum(n[i - 1])
}

/// 1..=maximo dígitos em n[j..], sem outro dígito logo depois.
fn ler_digitos(n: &[char], j: usize, maximo: usize) -> Option<(u32, usize)> {
    let mut k = j;
    while k < n.len() && n[k].is_ascii_digit() && k - j < maximo {
        k += 1;
    }
    if k == j || (k < n.len() && n[k].is_ascii_digit()) {
        return None;
    }
    let s: String = n[j..k].iter().collect();
    Some((s.parse().ok()?, k))
}

fn pular(n: &[char], mut j: usize, chars: &[char]) -> usize {
    while j < n.len() && chars.contains(&n[j]) {
        j += 1;
    }
    j
}

fn comeca(n: &[char], j: usize, p: &str) -> bool {
    let p: Vec<char> = p.chars().collect();
    j + p.len() <= n.len() && n[j..j + p.len()] == p[..]
}

const PALAVRAS_EP: [&str; 5] = ["episodio", "episode", "capitulo", "cap", "ep"];

fn palavra_de_episodio(n: &[char], j: usize) -> Option<u32> {
    for p in PALAVRAS_EP {
        if comeca(n, j, p) {
            let mut k = j + p.chars().count();
            if k < n.len() && n[k].is_alphabetic() {
                continue;
            }
            k = pular(n, k, &['.']);
            k = pular(n, k, &[' ']);
            if let Some((e, _)) = ler_digitos(n, k, 4) {
                return Some(e);
            }
        }
    }
    None
}

/// Dígitos (1..=maximo) seguidos exatamente de `seguinte`.
fn ler_digitos_ate(n: &[char], j: usize, maximo: usize, seguinte: char) -> Option<(u32, usize)> {
    let mut k = j;
    while k < n.len() && n[k].is_ascii_digit() {
        k += 1;
    }
    if k == j || k - j > maximo || k >= n.len() || n[k] != seguinte {
        return None;
    }
    let s: String = n[j..k].iter().collect();
    Some((s.parse().ok()?, k))
}

/// A marca de episódio do nome, em três passadas: S01E02/T01E02 no nome
/// inteiro; "Temporada N ... Episódio M"; 1x02 (nunca no começo do nome:
/// "4x4 Sob Medida" e "4x100" são títulos). A fraca (Ep 12) só sem forte.
pub fn marca(nome: &str) -> Option<Marca> {
    let n = dobrar(nome);
    for i in 0..n.len() {
        let c = n[i];
        if (c != 's' && c != 't') || !inicio_de_palavra(&n, i) {
            continue;
        }
        let Some((t, j)) = ler_digitos(&n, i + 1, 3) else { continue };
        let mut j = pular(&n, j, &[' ', '.', '_', '-']);
        if j < n.len() && (n[j] == 'e' || (c == 's' && n[j] == 'x')) {
            j += 1;
            if j < n.len() && n[j] == 'p' {
                j += 1;
            }
            j = pular(&n, j, &[' ', '.']);
            if let Some((e, _)) = ler_digitos(&n, j, 4) {
                return Some(Marca { temporada: t, episodio: e, inicio: i, forte: true });
            }
        }
    }
    for i in 0..n.len() {
        if !inicio_de_palavra(&n, i) {
            continue;
        }
        for p in ["temporada", "season"] {
            if !comeca(&n, i, p) {
                continue;
            }
            let j = pular(&n, i + p.chars().count(), &[' ']);
            let Some((t, j)) = ler_digitos(&n, j, 3) else { continue };
            for k in j..(j + 13).min(n.len()) {
                if inicio_de_palavra(&n, k) {
                    if let Some(e) = palavra_de_episodio(&n, k) {
                        return Some(Marca { temporada: t, episodio: e, inicio: i, forte: true });
                    }
                }
            }
        }
    }
    for i in 1..n.len() {
        if !n[i].is_ascii_digit() || !inicio_de_palavra(&n, i) {
            continue;
        }
        let Some((t, j)) = ler_digitos_ate(&n, i, 2, 'x') else { continue };
        if let Some((e, k)) = ler_digitos(&n, j + 1, 3) {
            if k == n.len() || !alnum(n[k]) {
                return Some(Marca { temporada: t, episodio: e, inicio: i, forte: true });
            }
        }
    }
    for i in 0..n.len() {
        if inicio_de_palavra(&n, i) {
            if let Some(e) = palavra_de_episodio(&n, i) {
                return Some(Marca { temporada: 1, episodio: e, inicio: i, forte: false });
            }
        }
    }
    None
}

/// Segmentos do caminho do endereço, em minúsculas.
fn caminho(url: &str) -> Vec<String> {
    let u = url.split('#').next().unwrap_or("").split('?').next().unwrap_or("");
    let u = match u.split_once("://") {
        Some((_, resto)) => resto.split_once('/').map(|(_, c)| c).unwrap_or(""),
        None => u,
    };
    u.to_lowercase().split('/').filter(|s| !s.is_empty()).map(str::to_string).collect()
}

fn palavras(t: &str) -> Vec<String> {
    let mut todas = Vec::new();
    let mut atual = String::new();
    for c in t.chars() {
        if c.is_alphanumeric() {
            atual.push(c);
        } else if !atual.is_empty() {
            todas.push(std::mem::take(&mut atual));
        }
    }
    if !atual.is_empty() {
        todas.push(atual);
    }
    todas
}

fn grupo_de_acervo(grupo: &str) -> bool {
    let g = dobrar_texto(grupo);
    let ws = palavras(&g);
    if !ws.iter().any(|w| PALAVRAS_ACERVO.contains(&w.as_str())) {
        return false;
    }
    if g.contains("24h") || g.contains("24 horas") || g.contains("24/7") || g.contains("ao vivo") {
        return false;
    }
    !ws.iter().any(|w| matches!(w.as_str(), "canal" | "canais" | "live"))
}

/// "(19xx)"/"(20xx)" no fim.
pub fn ano_no_fim(nome: &str) -> bool {
    let c: Vec<char> = nome.trim_end().chars().collect();
    let n = c.len();
    n >= 6
        && c[n - 1] == ')'
        && c[n - 6] == '('
        && c[n - 5..n - 1].iter().all(|d| d.is_ascii_digit())
        && matches!((c[n - 5], c[n - 4]), ('1', '9') | ('2', '0'))
}

fn ano_do_fim(nome: &str) -> String {
    let c: Vec<char> = nome.trim_end().chars().collect();
    c[c.len() - 5..c.len() - 1].iter().collect()
}

/// O tipo de um item da lista.
pub fn tipo(url: &str, nome: &str, grupo: &str, declarado: &str) -> Tipo {
    let m = marca(nome);
    let d = declarado.trim().to_lowercase();
    if TIPO_CANAL.contains(&d.as_str()) {
        return Tipo::Canal;
    }
    let mut acervo = TIPO_FILME.contains(&d.as_str()) || TIPO_SERIE.contains(&d.as_str());
    let segs = caminho(url);
    if !acervo {
        for s in segs.iter().take(segs.len().saturating_sub(1)) {
            if SEG_FILME.contains(&s.as_str()) || SEG_SERIE.contains(&s.as_str()) {
                acervo = true;
                break;
            }
            if s == "live" {
                return Tipo::Canal;
            }
        }
        // Canal Xtream sem prefixo: /usuario/senha/123(.ts|.m3u8).
        if !acervo && segs.len() == 3 {
            let (num, ext) = segs[2].split_once('.').unwrap_or((segs[2].as_str(), ""));
            if !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()) && matches!(ext, "" | "ts" | "m3u8") {
                return Tipo::Canal;
            }
        }
    }
    if !acervo {
        if let Some(ultimo) = segs.last() {
            if let Some((_, ext)) = ultimo.rsplit_once('.') {
                acervo = VIDEO.contains(&ext);
            }
        }
    }
    if acervo {
        return match m {
            Some(m) => Tipo::Episodio(m),
            None => Tipo::Filme,
        };
    }
    if grupo_de_acervo(grupo) {
        if let Some(m) = m.filter(|m| m.forte) {
            return Tipo::Episodio(m);
        }
        if ano_no_fim(&limpar(nome).0) {
            return Tipo::Filme;
        }
    }
    Tipo::Canal
}

// ---------- títulos ----------

fn resolucao(t: &str) -> bool {
    let c: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_ascii_digit() {
            let ini = i;
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            if i - ini >= 3 && i < c.len() && matches!(c[i], 'p' | 'P' | 'i' | 'I') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

fn tem_palavra(t: &str, p: &str) -> bool {
    let c: Vec<char> = t.chars().collect();
    let pc: Vec<char> = p.chars().collect();
    let palavra = |x: char| x.is_alphanumeric() || x == '_';
    (0..c.len()).any(|i| {
        i + pc.len() <= c.len()
            && c[i..i + pc.len()] == pc[..]
            && (i == 0 || !palavra(c[i - 1]))
            && (i + pc.len() >= c.len() || !palavra(c[i + pc.len()]))
    })
}

fn tag_ruim(conteudo: &str) -> bool {
    let t = conteudo.to_lowercase();
    resolucao(&t)
        || t.contains("not 24/7")
        || t.contains("geoblocked")
        || t.contains("geo-blocked")
        || t.contains("backup")
        || t.contains("vertical")
        || tem_palavra(&t, "sd")
        || tem_palavra(&t, "hd")
        || t.contains("fhd")
        || tem_palavra(&t, "4k")
}

fn juntar_espacos(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Tira tags de qualidade e de versão; devolve (nome, versão).
pub fn limpar(nome: &str) -> (String, Option<&'static str>) {
    let mut versao: Option<&'static str> = None;
    let mut nome = nome.trim().to_string();
    loop {
        let t = nome.trim_end().to_string();
        if let Some(fecha) = t.chars().last().filter(|c| *c == ')' || *c == ']') {
            let abre = if fecha == ')' { '(' } else { '[' };
            if let Some(pos) = t.rfind(abre) {
                let conteudo = t[pos + 1..t.len() - 1].trim();
                let c = dobrar_texto(conteudo);
                if TAGS_LEG.contains(&c.as_str()) {
                    versao = versao.or(Some("leg"));
                    nome = t[..pos].to_string();
                    continue;
                }
                if TAGS_DUB.contains(&c.as_str()) {
                    versao = versao.or(Some("dub"));
                    nome = t[..pos].to_string();
                    continue;
                }
                if tag_ruim(conteudo) {
                    nome = t[..pos].to_string();
                    continue;
                }
            }
        }
        if let Some((antes, ultima)) = t.rsplit_once(' ') {
            if matches!(dobrar_texto(ultima).as_str(), "leg" | "legendado" | "legendada") {
                versao = versao.or(Some("leg"));
                nome = antes.to_string();
                continue;
            }
        }
        nome = t;
        break;
    }
    // Tags de versão no meio ("Filme [L] (2020)").
    for (abre, fecha) in [('[', ']'), ('(', ')')] {
        let mut de = 0;
        while let Some(i) = nome[de..].find(abre).map(|i| i + de) {
            let Some(f) = nome[i + 1..].find(fecha).map(|f| f + i + 1) else { break };
            let c = dobrar_texto(nome[i + 1..f].trim());
            if TAGS_LEG.contains(&c.as_str()) || TAGS_DUB.contains(&c.as_str()) {
                versao = versao.or(Some(if TAGS_LEG.contains(&c.as_str()) { "leg" } else { "dub" }));
                nome = format!("{} {}", nome[..i].trim_end(), nome[f + 1..].trim_start()).trim().to_string();
                de = i.min(nome.len());
            } else {
                de = f + 1;
            }
        }
    }
    (juntar_espacos(&nome), versao)
}

/// "Filme (2025) (2025)" vira "Filme (2025)".
fn ano_repetido(nome: &str) -> String {
    let mut t = nome.trim_end().to_string();
    loop {
        if ano_no_fim(&t) {
            let antes = t[..t.len() - 6].trim_end().to_string();
            if ano_no_fim(&antes) && ano_do_fim(&antes) == ano_do_fim(&t) {
                t = antes;
                continue;
            }
        }
        return t;
    }
}

fn sem_extensao(nome: &str) -> String {
    let t = nome.trim();
    if let Some((antes, ext)) = t.rsplit_once('.') {
        if VIDEO.contains(&ext.to_lowercase().as_str()) {
            return antes.trim().to_string();
        }
    }
    t.to_string()
}

fn versao_do_grupo(grupo: &str) -> &'static str {
    if dobrar_texto(grupo).contains("legendad") { "leg" } else { "dub" }
}

/// (título, ano, versão) de um filme.
pub fn titulo_do_filme(nome: &str, grupo: &str) -> (String, String, &'static str) {
    let (t, versao) = limpar(&sem_extensao(nome));
    let t = ano_repetido(&t);
    let ano = if ano_no_fim(&t) { ano_do_fim(&t) } else { String::new() };
    (t, ano, versao.unwrap_or_else(|| versao_do_grupo(grupo)))
}

/// (título, ano, versão) da série de um episódio.
pub fn titulo_da_serie(nome: &str, inicio: usize, grupo: &str) -> (String, String, &'static str) {
    let antes: String = nome.chars().take(inicio).collect();
    let depois: String = nome.chars().skip(inicio).collect();
    let (t, versao) = limpar(antes.trim_end_matches(SEPARADORES));
    let t = t.trim_end_matches(SEPARADORES).trim().to_string();
    let versao = versao.or(limpar(&depois).1);
    let mut t = ano_repetido(&t);
    let mut ano = String::new();
    if ano_no_fim(&t) {
        ano = ano_do_fim(&t);
        let c: Vec<char> = t.chars().collect();
        t = c[..c.len() - 6].iter().collect::<String>().trim_end_matches(SEPARADORES).trim().to_string();
    }
    if t.is_empty() {
        t = if grupo.trim().is_empty() { "Série".to_string() } else { grupo.trim().to_string() };
    }
    (t, ano, versao.unwrap_or_else(|| versao_do_grupo(grupo)))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn atributo(linha: &str, nome: &str) -> String {
        let procurado = format!("{nome}=\"");
        linha
            .find(&procurado)
            .and_then(|i| {
                let resto = &linha[i + procurado.len()..];
                resto.find('"').map(|f| resto[..f].trim().to_string())
            })
            .unwrap_or_default()
    }

    #[test]
    fn gabarito() {
        let texto = std::fs::read_to_string("tests/dados/classificar.m3u").unwrap();
        let esperado: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string("tests/dados/classificar.esperado.json").unwrap()).unwrap();
        let mut obtido = Vec::new();
        let mut pend = String::new();
        for linha in texto.lines().map(str::trim) {
            if linha.starts_with("#EXTINF") {
                pend = linha.to_string();
                continue;
            }
            if linha.is_empty() || linha.starts_with('#') {
                continue;
            }
            let nome = pend.rsplit_once("\",").map(|(_, n)| n).or_else(|| pend.split_once(',').map(|(_, n)| n)).unwrap_or("").trim().to_string();
            let grupo = atributo(&pend, "group-title");
            let declarado = [atributo(&pend, "tvg-type"), atributo(&pend, "type")].into_iter().find(|s| !s.is_empty()).unwrap_or_default();
            let mut item = serde_json::json!({ "nome": nome });
            match tipo(linha, &nome, &grupo, &declarado) {
                Tipo::Canal => item["tipo"] = "canal".into(),
                Tipo::Filme => {
                    let (t, a, v) = titulo_do_filme(&nome, &grupo);
                    item["tipo"] = "filme".into();
                    item["titulo"] = t.into();
                    item["ano"] = a.into();
                    item["versao"] = v.into();
                }
                Tipo::Episodio(m) => {
                    let (t, a, v) = titulo_da_serie(&nome, m.inicio, &grupo);
                    item["tipo"] = "episodio".into();
                    item["titulo"] = t.into();
                    item["ano"] = a.into();
                    item["versao"] = v.into();
                    item["temporada"] = m.temporada.into();
                    item["episodio"] = m.episodio.into();
                }
            }
            obtido.push(item);
        }
        assert_eq!(serde_json::Value::Array(obtido), esperado);
    }

    #[test]
    fn marcas() {
        assert_eq!(marca("Breaking Bad S05 E14").map(|m| (m.temporada, m.episodio)), Some((5, 14)));
        assert_eq!(marca("Premiere 1920x1080"), None);
        assert_eq!(marca("Lost 4x07").map(|m| (m.temporada, m.episodio)), Some((4, 7)));
        assert!(marca("Esportes").is_none());
    }
}
