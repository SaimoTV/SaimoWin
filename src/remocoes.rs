//! Registro do que foi removido ("Continue assistindo", favoritos, canais
//! favoritos, Minhas listas), para a sincronia (Conta Saimo e Saimo Connect)
//! não trazer de volta. Contrato em SaimoPlayer/docs/perfis.md: cada remoção é
//! `{…, em}` e um item só fica se for mais novo que ela (sem `em` conta como 0).
//! As mesmas regras do servidor (SaimoConta/src/juntar.ts).
//!
//! Por perfil (`remocoes.json` na pasta do perfil): prog e fav ({tipo, titulo,
//! ano, em}; ano vazio vale para qualquer ano), canal ({nome, em}), favEm e
//! canalEm (quando cada favorito entrou, pelo título/nome). Minhas listas (do
//! aparelho, `remocoes-listas.json`): rem ({link, em}) e em por link.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

const MAX: usize = 300;
const VIDA: i64 = 180 * 24 * 3600_000;

#[derive(Default, Clone)]
pub struct Registro {
    pub prog: Vec<Value>,
    pub fav: Vec<Value>,
    pub canal: Vec<Value>,
    pub fav_em: HashMap<String, i64>,
    pub canal_em: HashMap<String, i64>,
}

fn agora_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn texto(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn num(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))).unwrap_or(0)
}

fn tipo(v: &Value) -> &'static str {
    if v.get("tipo").and_then(Value::as_str) == Some("s") { "s" } else { "f" }
}

fn chave_titulo(v: &Value) -> Option<String> {
    let t = texto(v, "titulo");
    (!t.is_empty()).then(|| format!("{}|{}|{}", tipo(v), t, texto(v, "ano")))
}

fn chave_nome(v: &Value) -> Option<String> {
    let n = texto(v, "nome");
    (!n.is_empty()).then_some(n)
}

fn chave_link(v: &Value) -> Option<String> {
    let l = texto(v, "link").trim().to_string();
    (!l.is_empty()).then_some(l)
}

fn lista(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array).map(|a| a.iter().filter(|x| x.is_object()).cloned().collect()).unwrap_or_default()
}

fn mapa(v: Option<&Value>) -> HashMap<String, i64> {
    v.and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, x)| (k.clone(), x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)).unwrap_or(0))).collect())
        .unwrap_or_default()
}

/// Junta pela chave (vale o `em` maior); as velhas demais saem.
fn juntar_por(a: &[Value], b: &[Value], chave: fn(&Value) -> Option<String>) -> Vec<Value> {
    let limite = agora_ms() - VIDA;
    let mut m: HashMap<String, Value> = HashMap::new();
    for r in a.iter().chain(b.iter()) {
        let Some(k) = chave(r) else { continue };
        let em = num(r, "em");
        if em <= 0 || em < limite {
            continue;
        }
        if m.get(&k).is_some_and(|x| num(x, "em") >= em) {
            continue;
        }
        m.insert(k, r.clone());
    }
    let mut v: Vec<Value> = m.into_values().collect();
    v.sort_by_key(|x| -num(x, "em"));
    v.truncate(MAX);
    v
}

pub fn titulo_removido(rem: &[Value], tipo_: &str, titulo: &str, ano: &str, em_item: i64) -> bool {
    rem.iter().any(|r| {
        tipo(r) == tipo_
            && texto(r, "titulo") == titulo
            && (texto(r, "ano").is_empty() || ano.is_empty() || texto(r, "ano") == ano)
            && num(r, "em") >= em_item
    })
}

pub fn canal_removido(r: &Registro, nome: &str) -> bool {
    let em = r.canal_em.get(nome).copied().unwrap_or(0);
    r.canal.iter().any(|c| texto(c, "nome") == nome && num(c, "em") >= em)
}

pub fn fav_removido(r: &Registro, tipo_: &str, titulo: &str, ano: &str) -> bool {
    let em = r.fav_em.get(titulo).copied().unwrap_or(0);
    titulo_removido(&r.fav, tipo_, titulo, ano, em)
}

fn arquivo(id: &str) -> std::path::PathBuf {
    crate::perfis::pasta_de(id).join("remocoes.json")
}

pub fn ler(id: &str) -> Registro {
    let v: Value = std::fs::read_to_string(arquivo(id)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    Registro {
        prog: lista(v.get("prog")),
        fav: lista(v.get("fav")),
        canal: lista(v.get("canal")),
        fav_em: mapa(v.get("favEm")),
        canal_em: mapa(v.get("canalEm")),
    }
}

pub fn gravar(id: &str, r: &Registro) {
    let v = json!({
        "prog": juntar_por(&r.prog, &[], chave_titulo),
        "fav": juntar_por(&r.fav, &[], chave_titulo),
        "canal": juntar_por(&r.canal, &[], chave_nome),
        "favEm": r.fav_em,
        "canalEm": r.canal_em,
    });
    let _ = std::fs::write(arquivo(id), v.to_string());
}

/// Junta ao registro do perfil as remoções que vieram de fora (perfil do pacote).
pub fn juntar(id: &str, chegou: &Value) -> Registro {
    let mut r = ler(id);
    for (n, em) in mapa(chegou.get("favCanaisEm")) {
        if em > r.canal_em.get(&n).copied().unwrap_or(0) {
            r.canal_em.insert(n, em);
        }
    }
    for f in lista(chegou.get("favTitulos")) {
        let (t, em) = (texto(&f, "titulo"), num(&f, "em"));
        if !t.is_empty() && em > r.fav_em.get(&t).copied().unwrap_or(0) {
            r.fav_em.insert(t, em);
        }
    }
    r.prog = juntar_por(&r.prog, &lista(chegou.get("progressoRemovido")), chave_titulo);
    r.fav = juntar_por(&r.fav, &lista(chegou.get("favTitulosRemovidos")), chave_titulo);
    r.canal = juntar_por(&r.canal, &lista(chegou.get("favCanaisRemovidos")), chave_nome);
    gravar(id, &r);
    r
}

/// As remoções no perfil do pacote.
pub fn anexar(perfil: &mut Value, r: &Registro) {
    if !r.prog.is_empty() {
        perfil["progressoRemovido"] = json!(r.prog);
    }
    if !r.fav.is_empty() {
        perfil["favTitulosRemovidos"] = json!(r.fav);
    }
    if !r.canal.is_empty() {
        perfil["favCanaisRemovidos"] = json!(r.canal);
    }
    if !r.canal_em.is_empty() {
        perfil["favCanaisEm"] = json!(r.canal_em);
    }
}

/// Tirou do "Continue assistindo" (perfil ativo).
pub fn anotar_continuar(tipo_: &str, titulo: &str, ano: &str) {
    if titulo.is_empty() {
        return;
    }
    let id = crate::perfis::id_ativo();
    let mut r = ler(&id);
    r.prog.push(json!({"tipo": tipo_, "titulo": titulo, "ano": ano, "em": agora_ms()}));
    gravar(&id, &r);
}

/// Desfavoritou (removido) ou favoritou um título (perfil ativo).
pub fn anotar_favorito(tipo_: &str, titulo: &str, ano: &str, removido: bool) {
    let id = crate::perfis::id_ativo();
    let mut r = ler(&id);
    if removido {
        r.fav.push(json!({"tipo": tipo_, "titulo": titulo, "ano": ano, "em": agora_ms()}));
        r.fav_em.remove(titulo);
    } else {
        r.fav.retain(|x| texto(x, "titulo") != titulo);
        r.fav_em.insert(titulo.to_string(), agora_ms());
    }
    gravar(&id, &r);
}

/// Desfavoritou (removido) ou favoritou um canal (perfil ativo).
pub fn anotar_canal(nome: &str, removido: bool) {
    let id = crate::perfis::id_ativo();
    let mut r = ler(&id);
    if removido {
        r.canal.push(json!({"nome": nome, "em": agora_ms()}));
        r.canal_em.remove(nome);
    } else {
        r.canal.retain(|x| texto(x, "nome") != nome);
        r.canal_em.insert(nome.to_string(), agora_ms());
    }
    gravar(&id, &r);
}

// ---------- Minhas listas (do aparelho) ----------

#[derive(Default, Clone)]
pub struct RegistroListas {
    pub rem: Vec<Value>,
    pub em: HashMap<String, i64>,
}

fn arquivo_listas() -> std::path::PathBuf {
    crate::catalogo::pasta().join("remocoes-listas.json")
}

pub fn ler_listas() -> RegistroListas {
    let v: Value = std::fs::read_to_string(arquivo_listas()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    RegistroListas { rem: lista(v.get("rem")), em: mapa(v.get("em")) }
}

fn gravar_listas(r: &RegistroListas) {
    let mut m = Map::new();
    m.insert("rem".into(), json!(juntar_por(&r.rem, &[], chave_link)));
    m.insert("em".into(), json!(r.em));
    let _ = std::fs::write(arquivo_listas(), Value::Object(m).to_string());
}

pub fn juntar_listas(chegou: Option<&Value>) -> RegistroListas {
    let mut r = ler_listas();
    r.rem = juntar_por(&r.rem, &lista(chegou), chave_link);
    gravar_listas(&r);
    r
}

pub fn lista_removida(r: &RegistroListas, link: &str, em_item: Option<i64>) -> bool {
    let em = em_item.unwrap_or_else(|| r.em.get(link).copied().unwrap_or(0));
    r.rem.iter().any(|x| chave_link(x).as_deref() == Some(link) && num(x, "em") >= em)
}

pub fn anotar_lista(link: Option<&str>, removida: bool) {
    let Some(link) = link.filter(|l| !l.is_empty()) else { return };
    let mut r = ler_listas();
    if removida {
        r.rem.push(json!({"link": link, "em": agora_ms()}));
        r.em.remove(link);
    } else {
        r.rem.retain(|x| chave_link(x).as_deref() != Some(link));
        r.em.insert(link.to_string(), agora_ms());
    }
    gravar_listas(&r);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn remocao_vale_so_para_o_mais_antigo() {
        let rem = vec![json!({"tipo": "s", "titulo": "Dark", "ano": "", "em": 100})];
        assert!(titulo_removido(&rem, "s", "Dark", "2017", 50));
        assert!(!titulo_removido(&rem, "s", "Dark", "2017", 150));
        assert!(!titulo_removido(&rem, "f", "Dark", "", 50));
    }
}
