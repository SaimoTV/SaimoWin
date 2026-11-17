//! O acervo de filmes e séries de uma lista M3U/Xtream, no formato Saimo
//! (SaimoPlayer/docs/formato-saimo.md, item 5; regras em docs/listas-m3u.md).
//! Gerado ao adicionar ou atualizar a lista, numa pasta só dela; o `vod` lê
//! essa pasta como lê o acervo publicado, sem `base:` (endereços completos).

use crate::classificar::{self, Tipo};
use crate::minhas_listas::ItemVod;
use std::collections::HashMap;
use std::path::Path;

/// Pedaços de episódios de até uns 100 KB, como no acervo publicado.
const PEDACO_BYTES: usize = 100 * 1024;

/// A letra do título (item 5.1): primeira letra sem acento, em maiúscula; o resto vai em `#`.
pub fn letra(titulo: &str) -> String {
    match classificar::dobrar(titulo).into_iter().find(|c| !c.is_whitespace()) {
        Some(c) if c.is_ascii_lowercase() => c.to_ascii_uppercase().to_string(),
        _ => "#".to_string(),
    }
}

/// Nome do arquivo da letra no disco (`#` vira `hash`, como o cache do acervo).
pub fn nome_no_disco(nome: &str) -> String {
    nome.replace("%23", "hash").replace('#', "hash")
}

fn chave(t: &str) -> String {
    classificar::dobrar(t).into_iter().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn campo(t: &str) -> String {
    t.replace(['\t', '\n', '\r'], " ").trim().to_string()
}

/// Vírgula separa fontes no formato: dentro do endereço vira %2C.
fn fonte(url: &str) -> String {
    url.replace(',', "%2C").replace(['\t', '\n', '\r'], "")
}

struct FilmeLista {
    titulo: String,
    ano: String,
    versoes: Vec<(&'static str, Vec<String>)>,
    capa: Option<String>,
    generos: Vec<String>,
}

struct SerieLista {
    titulo: String,
    ano: String,
    capa: Option<String>,
    generos: Vec<String>,
    /// (temporada, episódio, versão) -> fontes, na ordem da lista.
    episodios: Vec<((u32, u32, &'static str), Vec<String>)>,
    indice: HashMap<(u32, u32, &'static str), usize>,
}

/// "# Drama" vira "Drama"; "▶ Direct Filmes" vira "Direct Filmes".
fn genero(grupo: &str) -> String {
    campo(grupo.trim_start_matches(|c: char| !c.is_alphanumeric()).trim()).replace(',', " ")
}

fn completar(capa: &mut Option<String>, generos: &mut Vec<String>, item: &ItemVod) {
    if capa.is_none() {
        capa.clone_from(&item.logo);
    }
    let g = genero(&item.grupo);
    if !g.is_empty() && !generos.contains(&g) {
        generos.push(g);
    }
}

/// Quantos filmes e séries o acervo tem.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resumo {
    pub filmes: usize,
    pub series: usize,
}

/// Monta os arquivos do acervo em `pasta` (apaga o que havia). Sem itens, a pasta some.
pub fn gravar(pasta: &Path, itens: &[ItemVod]) -> Resumo {
    let _ = std::fs::remove_dir_all(pasta);
    if itens.is_empty() {
        return Resumo::default();
    }
    let mut filmes: Vec<FilmeLista> = Vec::new();
    let mut idx_filme: HashMap<String, usize> = HashMap::new();
    let mut series: Vec<SerieLista> = Vec::new();
    let mut idx_serie: HashMap<String, usize> = HashMap::new();
    for item in itens {
        match item.tipo {
            Tipo::Canal => {}
            Tipo::Filme => {
                let (titulo, ano, versao) = classificar::titulo_do_filme(&item.nome, &item.grupo);
                let titulo = campo(&titulo);
                if titulo.is_empty() {
                    continue;
                }
                let k = chave(&titulo);
                let i = *idx_filme.entry(k).or_insert_with(|| {
                    filmes.push(FilmeLista { titulo: titulo.clone(), ano: ano.clone(), versoes: Vec::new(), capa: None, generos: Vec::new() });
                    filmes.len() - 1
                });
                let f = &mut filmes[i];
                completar(&mut f.capa, &mut f.generos, item);
                let url = fonte(&item.url);
                match f.versoes.iter_mut().find(|(v, _)| *v == versao) {
                    Some((_, urls)) => {
                        if !urls.contains(&url) {
                            urls.push(url);
                        }
                    }
                    None => f.versoes.push((versao, vec![url])),
                }
            }
            Tipo::Episodio(m) => {
                let (titulo, ano, versao) = classificar::titulo_da_serie(&item.nome, m.inicio, &item.grupo);
                let titulo = campo(&titulo);
                let k = format!("{}|{}", chave(&titulo), ano);
                let i = *idx_serie.entry(k).or_insert_with(|| {
                    series.push(SerieLista { titulo: titulo.clone(), ano: ano.clone(), capa: None, generos: Vec::new(), episodios: Vec::new(), indice: HashMap::new() });
                    series.len() - 1
                });
                let s = &mut series[i];
                completar(&mut s.capa, &mut s.generos, item);
                let ek = (m.temporada, m.episodio, versao);
                let url = fonte(&item.url);
                match s.indice.get(&ek) {
                    Some(&j) => {
                        if !s.episodios[j].1.contains(&url) {
                            s.episodios[j].1.push(url);
                        }
                    }
                    None => {
                        s.indice.insert(ek, s.episodios.len());
                        s.episodios.push((ek, vec![url]));
                    }
                }
            }
        }
    }

    // Por letra, em ordem alfabética (como o acervo publicado).
    let ordem = |t: &str| classificar::dobrar(t).into_iter().collect::<String>();
    filmes.sort_by_key(|f| ordem(&f.titulo));
    series.sort_by_key(|s| ordem(&s.titulo));
    let mut por_letra: std::collections::BTreeMap<String, (Vec<&FilmeLista>, Vec<&SerieLista>)> = Default::default();
    for f in &filmes {
        por_letra.entry(letra(&f.titulo)).or_default().0.push(f);
    }
    for s in &series {
        por_letra.entry(letra(&s.titulo)).or_default().1.push(s);
    }

    let tmp = pasta.with_extension("novo");
    let _ = std::fs::remove_dir_all(&tmp);
    if std::fs::create_dir_all(&tmp).is_err() {
        return Resumo::default();
    }
    let escrever = |nome: &str, texto: &str| {
        let _ = std::fs::write(tmp.join(nome_no_disco(nome)), texto);
    };
    let mut indice = String::new();
    let mut busca = String::new();
    for (l, (fs, ss)) in &por_letra {
        indice.push_str(&format!("{l}\t{}\t{}\t0\n", fs.len(), ss.len()));
        if !fs.is_empty() {
            let mut texto = String::new();
            for f in fs {
                texto.push_str(&f.titulo);
                for (v, urls) in &f.versoes {
                    texto.push_str(&format!("\t{v}={}", urls.join(",")));
                }
                texto.push('\n');
                busca.push_str(&format!("{}\tf\t{l}\t{}\n", f.titulo, f.ano));
            }
            escrever(&format!("filmes-{l}.txt"), &texto);
        }
        if !ss.is_empty() {
            let mut lista = String::new();
            let mut pedaco = 0usize;
            let mut atual = String::new();
            for s in ss {
                let mut bloco = format!("@{}\t{}\n", s.titulo, s.ano);
                let mut eps: Vec<&((u32, u32, &str), Vec<String>)> = s.episodios.iter().collect();
                eps.sort_by_key(|((t, e, v), _)| (*t, *e, *v != "dub"));
                for ((t, e, v), urls) in eps {
                    bloco.push_str(&format!("{t}\t{e}\t{v}\t{}\n", urls.join(",")));
                }
                if !atual.is_empty() && atual.len() + bloco.len() > PEDACO_BYTES {
                    escrever(&format!("series-{l}-{pedaco}.txt"), &atual);
                    atual.clear();
                    pedaco += 1;
                }
                atual.push_str(&bloco);
                lista.push_str(&format!("{}\t{}\t{pedaco}\t{}\n", s.titulo, s.ano, s.episodios.len()));
                busca.push_str(&format!("{}\ts\t{l}\t{}\n", s.titulo, s.ano));
            }
            if !atual.is_empty() {
                escrever(&format!("series-{l}-{pedaco}.txt"), &atual);
            }
            escrever(&format!("series-{l}.txt"), &lista);
        }
    }
    escrever("indice.txt", &indice);
    escrever("busca.txt", &busca);
    // Capas e gêneros (o grupo da lista), no formato do fichas.txt publicado.
    // Imagem usada por mais de 3 títulos diferentes é a genérica do provedor
    // (uma mesma "capa" em milhares de séries): melhor sem capa que a errada.
    let mut usos: HashMap<&str, usize> = HashMap::new();
    for c in filmes.iter().filter_map(|f| f.capa.as_deref()).chain(series.iter().filter_map(|s| s.capa.as_deref())) {
        *usos.entry(c).or_default() += 1;
    }
    let generica = |c: &Option<String>| c.as_deref().is_some_and(|c| usos.get(c).copied().unwrap_or(0) > 3);
    let filmes_capa: Vec<Option<String>> = filmes.iter().map(|f| if generica(&f.capa) { None } else { f.capa.clone() }).collect();
    let series_capa: Vec<Option<String>> = series.iter().map(|s| if generica(&s.capa) { None } else { s.capa.clone() }).collect();
    let mut fichas = String::from("capa: \n");
    let linha = |tipo: &str, titulo: &str, capa: &Option<String>, generos: &[String]| {
        format!("{tipo}\t{titulo}\t0\t{}\t{}\n", capa.as_deref().map(fonte).unwrap_or_default(), generos.join(","))
    };
    for (f, capa) in filmes.iter().zip(&filmes_capa) {
        if capa.is_some() || !f.generos.is_empty() {
            fichas.push_str(&linha("f", &f.titulo, capa, &f.generos));
        }
    }
    for (s, capa) in series.iter().zip(&series_capa) {
        if capa.is_some() || !s.generos.is_empty() {
            fichas.push_str(&linha("s", &s.titulo, capa, &s.generos));
        }
    }
    escrever("fichas.txt", &fichas);
    let _ = std::fs::rename(&tmp, pasta);
    Resumo { filmes: filmes.len(), series: series.len() }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn monta_o_acervo_do_gabarito() {
        let texto = std::fs::read_to_string("tests/dados/classificar.m3u").unwrap();
        let (canais, vod) = crate::minhas_listas::ler_tudo(&texto);
        let pasta = std::env::temp_dir().join(format!("saimo-acervo-teste-{}", std::process::id()));
        let r = gravar(&pasta, &vod);
        assert_eq!(canais.len(), 8);
        assert_eq!(r, Resumo { filmes: 5, series: 10 });
        let indice = std::fs::read_to_string(pasta.join("indice.txt")).unwrap();
        assert!(indice.contains("V\t1\t0\t0"), "{indice}");
        let filmes_v = std::fs::read_to_string(pasta.join("filmes-V.txt")).unwrap();
        assert!(filmes_v.starts_with("Vítimas do Dia\tleg=") && filmes_v.contains("\tdub="), "{filmes_v}");
        let hash = std::fs::read_to_string(pasta.join("series-hash-0.txt")).unwrap();
        assert!(hash.starts_with("@3 Palavrinhas\t\n1\t1\tdub\t"), "{hash}");
        let busca = std::fs::read_to_string(pasta.join("busca.txt")).unwrap();
        assert!(busca.contains("Avenida Brasil\ts\tA\t2012"), "{busca}");
        let _ = std::fs::remove_dir_all(&pasta);
    }
}

#[cfg(test)]
mod lista_real {
    /// SAIMO_LISTA_REAL=/caminho/lista.m3u cargo test --release lista_real -- --ignored --nocapture
    #[test]
    #[ignore = "precisa de uma lista grande no disco"]
    fn lista_real() {
        let Ok(caminho) = std::env::var("SAIMO_LISTA_REAL") else { return };
        let t0 = std::time::Instant::now();
        let texto = crate::minhas_listas::ler_arquivo(std::path::Path::new(&caminho)).unwrap();
        let (canais, vod) = crate::minhas_listas::ler_tudo(&texto);
        let t1 = t0.elapsed();
        let pasta = std::env::temp_dir().join("saimo-acervo-real");
        let r = super::gravar(&pasta, &vod);
        println!("canais {} | itens vod {} | {:?} | leitura {:?} total {:?}", canais.len(), vod.len(), r, t1, t0.elapsed());
        println!("pasta: {}", pasta.display());
    }
}
