//! A fonte que a pessoa vinha assistindo numa série, para o episódio seguinte
//! tocar pela mesma. Cada episódio tem a própria lista de fontes, e a ordem
//! muda de um para o outro: o que se guarda é o provedor (o host) e a posição.

use crate::fontes_desativadas::servidor;

/// O que se lembra da série no ar, só nesta sessão.
#[derive(Clone, Debug, PartialEq)]
pub struct Memoria {
    pub serie: String,
    pub host: String,
    pub posicao: usize,
}

/// O servidor sem "www.", para comparar provedores.
pub fn host(url: &str) -> String {
    let s = servidor(url).unwrap_or_default();
    s.strip_prefix("www.").map(str::to_string).unwrap_or(s)
}

/// Qual fonte da lista tocar primeiro: a do mesmo provedor (a primeira que
/// bater), senão a da mesma posição, senão a primeira, que é a ordem de hoje.
pub fn indice(urls: &[String], host_lembrado: &str, posicao: usize) -> usize {
    if !host_lembrado.is_empty() {
        if let Some(i) = urls.iter().position(|u| host(u) == host_lembrado) {
            return i;
        }
    }
    if posicao < urls.len() { posicao } else { 0 }
}

/// A lista com a preferida na frente e as outras atrás, na ordem original,
/// para o player descer sozinho se ela não abrir.
pub fn ordenar(urls: &[String], host_lembrado: &str, posicao: usize) -> Vec<String> {
    let mut saida = urls.to_vec();
    let i = indice(urls, host_lembrado, posicao);
    if i > 0 && i < saida.len() {
        let escolhida = saida.remove(i);
        saida.insert(0, escolhida);
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    fn lista() -> Vec<String> {
        ["https://x.com/1", "https://www.b.net:8080/2", "http://c.org/3"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn mesmo_provedor_vence_a_posicao() {
        assert_eq!(indice(&lista(), "b.net", 0), 1);
        assert_eq!(indice(&lista(), "c.org", 1), 2);
    }

    #[test]
    fn sem_provedor_vale_a_posicao_e_depois_a_ordem_de_hoje() {
        assert_eq!(indice(&lista(), "outro.tv", 2), 2);
        assert_eq!(indice(&lista(), "outro.tv", 9), 0);
        assert_eq!(indice(&[], "x", 3), 0);
    }

    #[test]
    fn preferida_vai_na_frente_e_o_resto_segue_em_ordem() {
        let l = lista();
        assert_eq!(ordenar(&l, "c.org", 0), vec![l[2].clone(), l[0].clone(), l[1].clone()]);
        assert_eq!(ordenar(&l, "nada", 0), l);
    }

    #[test]
    fn host_ignora_www_porta_e_caixa() {
        assert_eq!(host("https://WWW.Exemplo.com:81/a"), "exemplo.com");
    }
}

// MARK: - Canais ao vivo

/// A última fonte que tocou em cada canal (pela URL, que sobrevive a
/// mudanças de ordem no catálogo). Sintonizar começa por ela, como no Mac e
/// na TV Box: a fonte 1 fora do ar custava 14 s em todo canal.
static LEMBRADAS: std::sync::Mutex<Option<std::collections::HashMap<String, String>>> = std::sync::Mutex::new(None);

fn arquivo_lembradas() -> std::path::PathBuf {
    crate::catalogo::pasta().join("fonte-canal.json")
}

fn lembradas() -> std::sync::MutexGuard<'static, Option<std::collections::HashMap<String, String>>> {
    let mut g = LEMBRADAS.lock().unwrap();
    if g.is_none() {
        *g = Some(
            std::fs::read_to_string(arquivo_lembradas())
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default(),
        );
    }
    g
}

/// Índice da fonte lembrada do canal, se ela ainda existe na lista.
pub fn do_canal(nome: &str, urls: &[&str]) -> Option<usize> {
    let g = lembradas();
    let url = g.as_ref()?.get(nome)?;
    urls.iter().position(|u| u == url)
}

pub fn lembrar_do_canal(nome: &str, url: &str) {
    let mut g = lembradas();
    let mapa = g.as_mut().unwrap();
    if mapa.get(nome).map(String::as_str) == Some(url) {
        return;
    }
    mapa.insert(nome.to_string(), url.to_string());
    if let Ok(t) = serde_json::to_string(mapa) {
        let _ = std::fs::write(arquivo_lembradas(), t);
    }
}
