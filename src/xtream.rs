//! Lista com usuário e senha (o formato "Xtream Codes" que a maioria dos
//! provedores de IPTV usa).
//!
//! Uma conta Xtream é guardada como um link M3U comum
//! (`http://servidor:porta/get.php?username=…&password=…&type=m3u_plus`): assim
//! ela viaja pelo Saimo Connect e entra em "Minhas listas" como qualquer outra.
//! Quem cola esse link direto, ou digita servidor + usuário + senha, cai no
//! mesmo caminho: o app lê os canais pela API do servidor (`player_api.php`),
//! que é mais leve e traz as categorias. Se a API não responde, vale o M3U do
//! próprio `get.php`.
//!
//! A senha nunca é mostrada de volta nem entra em log ou em mensagem de erro.

use crate::minhas_listas::{CanalM3u, Erro, FonteM3u};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conta {
    pub base: String,
    pub usuario: String,
    pub senha: String,
}

/// Percent-encoding dos caracteres que não são "livres" numa URL.
fn cod(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decodificar(s: &str) -> String {
    let s = s.replace('+', " ");
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn link_m3u(base: &str, usuario: &str, senha: &str) -> String {
    format!("{base}/get.php?username={}&password={}&type=m3u_plus&output=ts", cod(usuario), cod(senha))
}

impl Conta {
    pub fn link_m3u(&self) -> String {
        link_m3u(&self.base, &self.usuario, &self.senha)
    }
    pub fn api(&self, acao: Option<&str>) -> String {
        let mut u = format!("{}/player_api.php?username={}&password={}", self.base, cod(&self.usuario), cod(&self.senha));
        if let Some(a) = acao {
            u.push_str(&format!("&action={a}"));
        }
        u
    }
    pub fn canal(&self, id: &str, ext: &str) -> String {
        format!("{}/live/{}/{}/{id}.{ext}", self.base, cod(&self.usuario), cod(&self.senha))
    }
    pub fn filme(&self, id: &str, ext: &str) -> String {
        format!("{}/movie/{}/{}/{id}.{ext}", self.base, cod(&self.usuario), cod(&self.senha))
    }
    pub fn episodio(&self, id: &str, ext: &str) -> String {
        format!("{}/series/{}/{}/{id}.{ext}", self.base, cod(&self.usuario), cod(&self.senha))
    }
}

fn host_valido(host: &str) -> bool {
    let (nome, porta) = match host.rfind(':') {
        Some(i) if !host.ends_with(']') => (&host[..i], Some(&host[i + 1..])),
        _ => (host, None),
    };
    if let Some(p) = porta {
        if p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    if nome.starts_with('[') {
        return nome.ends_with(']') && nome.len() > 2 && nome[1..nome.len() - 1].bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.');
    }
    let b = nome.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_alphanumeric()
        && b[b.len() - 1].is_ascii_alphanumeric()
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'.' || *c == b'-')
}

/// Servidor digitado → base sem barra no fim (`http://host:porta`, com o caminho
/// do painel, se houver). Sem esquema ganha `http://` (a quase totalidade dos
/// painéis não tem https). `None` se não parece um endereço.
pub fn normalizar_servidor(texto: &str) -> Option<String> {
    let t = texto.trim();
    if t.is_empty() || t.chars().any(char::is_whitespace) {
        return None;
    }
    let com_esquema = if t.contains("://") { t.to_string() } else { format!("http://{t}") };
    let (esquema, resto) = com_esquema.split_once("://")?;
    let esquema = esquema.to_ascii_lowercase();
    if esquema != "http" && esquema != "https" {
        return None;
    }
    let sem_extras = resto.split(['?', '#']).next().unwrap_or("");
    let (host, caminho) = match sem_extras.find('/') {
        Some(i) => (&sem_extras[..i], &sem_extras[i..]),
        None => (sem_extras, ""),
    };
    if !host_valido(host) {
        return None;
    }
    let mut caminho = caminho.to_string();
    if ["/get.php", "/player_api.php", "/panel_api.php", "/c", "/c/"].contains(&caminho.to_ascii_lowercase().as_str()) {
        caminho.clear();
    }
    let caminho = caminho.trim_end_matches('/');
    Some(format!("{esquema}://{}{caminho}", host.to_ascii_lowercase()))
}

/// Reconhece o link de uma conta (`get.php` ou `player_api.php` com usuário e senha).
pub fn de_link(link: &str) -> Option<Conta> {
    let t = link.trim();
    let baixo = t.to_ascii_lowercase();
    let (i, tam) = ["/get.php?", "/player_api.php?"].iter().find_map(|p| baixo.find(p).map(|i| (i, p.len())))?;
    if !baixo.starts_with("http://") && !baixo.starts_with("https://") {
        return None;
    }
    let consulta = t[i + tam..].split('#').next().unwrap_or("");
    let (mut usuario, mut senha) = (None, None);
    for par in consulta.split('&') {
        if let Some((k, v)) = par.split_once('=') {
            match k.to_ascii_lowercase().as_str() {
                "username" => usuario = Some(decodificar(v)),
                "password" => senha = Some(decodificar(v)),
                _ => {}
            }
        }
    }
    let (usuario, senha) = (usuario.filter(|u| !u.is_empty())?, senha.filter(|s| !s.is_empty())?);
    Some(Conta { base: normalizar_servidor(&t[..i])?, usuario, senha })
}

/// O link a guardar a partir do que a pessoa digitou. Com usuário e senha, o
/// primeiro campo é o servidor; sem eles, é o link da lista.
pub fn link_do_formulario(digitado: &str, usuario: &str, senha: &str) -> Result<String, &'static str> {
    let u = usuario.trim();
    if u.is_empty() != senha.is_empty() {
        return Err("Usuário e senha andam juntos: preencha os dois, ou deixe os dois em branco se a lista é só um link");
    }
    if u.is_empty() {
        return Ok(digitado.trim().to_string());
    }
    let base = normalizar_servidor(digitado).ok_or("Esse servidor não parece um endereço válido")?;
    Ok(link_m3u(&base, u, senha))
}

// MARK: - Resposta da API (sem rede: testável)

#[derive(Debug, PartialEq, Eq)]
pub enum Entrada {
    Ok,
    SemApi,
    Recusada(&'static str),
}

fn texto(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(o) => o.to_string(),
    }
}

fn agora() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn verificar_entrada(json: &str) -> Entrada {
    let Ok(raiz) = serde_json::from_str::<Value>(json) else { return Entrada::SemApi };
    let Some(info) = raiz.get("user_info").filter(|i| i.is_object()) else { return Entrada::SemApi };
    let auth = texto(info.get("auth")).to_lowercase();
    if auth != "1" && auth != "true" {
        return Entrada::Recusada("Usuário ou senha incorretos");
    }
    let status = texto(info.get("status"));
    if !status.is_empty() && !status.eq_ignore_ascii_case("active") {
        return Entrada::Recusada("Essa conta não está ativa");
    }
    if let Ok(exp) = texto(info.get("exp_date")).parse::<f64>() {
        if exp > 0.0 && exp <= agora() {
            return Entrada::Recusada("Essa conta venceu");
        }
    }
    Entrada::Ok
}

fn chave(nome: &str) -> String {
    crate::minhas_listas::chave_do_nome(nome)
}

/// `get_live_streams` + categorias → canais, com `.m3u8` e `.ts` como duas fontes.
pub fn canais_da_api(conta: &Conta, streams: &str, categorias: &str, limite: usize) -> Vec<CanalM3u> {
    let Ok(Value::Array(lista)) = serde_json::from_str::<Value>(streams) else { return Vec::new() };
    let mut cats = std::collections::HashMap::new();
    if let Ok(Value::Array(c)) = serde_json::from_str::<Value>(categorias) {
        for o in c {
            cats.insert(texto(o.get("category_id")), texto(o.get("category_name")));
        }
    }
    let fonte = |url: String| FonteM3u { url, referer: None, ua: None, key_id: None, key: None, qualidade: None };
    let mut ordem: Vec<String> = Vec::new();
    let mut por_nome: std::collections::HashMap<String, CanalM3u> = std::collections::HashMap::new();
    for o in lista {
        if ordem.len() >= limite {
            break;
        }
        let id = texto(o.get("stream_id"));
        if id.is_empty() {
            continue;
        }
        let mut nome = texto(o.get("name"));
        if nome.is_empty() {
            nome = format!("Canal {}", ordem.len() + 1);
        }
        let grupo = cats.get(&texto(o.get("category_id"))).filter(|g| !g.is_empty()).cloned();
        let icone = texto(o.get("stream_icon"));
        let logo = icone.to_ascii_lowercase().starts_with("http").then_some(icone);
        let fontes = vec![fonte(conta.canal(&id, "m3u8")), fonte(conta.canal(&id, "ts"))];
        let k = chave(&nome);
        match por_nome.get_mut(&k) {
            Some(antigo) => {
                antigo.grupo = antigo.grupo.take().or(grupo);
                antigo.logo = antigo.logo.take().or(logo);
                antigo.fontes.extend(fontes);
            }
            None => {
                ordem.push(k.clone());
                por_nome.insert(k, CanalM3u { nome, grupo, logo, fontes });
            }
        }
    }
    ordem.into_iter().filter_map(|k| por_nome.remove(&k)).collect()
}

// MARK: - Rede

/// Entra na conta e devolve os canais. `buscar` baixa um texto (injetado para o teste).
pub fn canais(
    conta: &Conta,
    buscar: &dyn Fn(&str) -> Result<String, Erro>,
    ler_m3u: &dyn Fn(&str) -> Vec<CanalM3u>,
) -> Result<Vec<CanalM3u>, Erro> {
    let do_m3u = || -> Result<Vec<CanalM3u>, Erro> {
        let texto = buscar(&conta.link_m3u()).map_err(|_| Erro::Conta("Não consegui falar com o servidor"))?;
        let canais = ler_m3u(&texto);
        if canais.is_empty() {
            return Err(Erro::Conta("Essa conta não tem canais ao vivo"));
        }
        Ok(canais)
    };
    let entrada = match buscar(&conta.api(None)) {
        Ok(t) => verificar_entrada(&t),
        Err(_) => return do_m3u(),
    };
    match entrada {
        Entrada::SemApi => return do_m3u(),
        Entrada::Recusada(m) => return Err(Erro::Conta(m)),
        Entrada::Ok => {}
    }
    let cats = buscar(&conta.api(Some("get_live_categories"))).unwrap_or_else(|_| "[]".into());
    match buscar(&conta.api(Some("get_live_streams"))) {
        Ok(streams) => {
            let canais = canais_da_api(conta, &streams, &cats, 20_000);
            if canais.is_empty() { do_m3u() } else { Ok(canais) }
        }
        Err(_) => do_m3u(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn servidor() {
        assert_eq!(normalizar_servidor("exemplo.dev:8080").as_deref(), Some("http://exemplo.dev:8080"));
        assert_eq!(normalizar_servidor("  http://exemplo.dev:8080/  ").as_deref(), Some("http://exemplo.dev:8080"));
        assert_eq!(normalizar_servidor("http://exemplo.dev:8080/get.php").as_deref(), Some("http://exemplo.dev:8080"));
        assert_eq!(normalizar_servidor("HTTPS://EXEMPLO.dev/player_api.php").as_deref(), Some("https://exemplo.dev"));
        assert_eq!(normalizar_servidor("http://exemplo.dev/painel/").as_deref(), Some("http://exemplo.dev/painel"));
        assert_eq!(normalizar_servidor("10.0.2.2:8765").as_deref(), Some("http://10.0.2.2:8765"));
        for ruim in ["", "   ", "ftp://x.dev", "http://", "a b.dev", "http://x.dev:abc", "http://u@x.dev", "http://:80"] {
            assert_eq!(normalizar_servidor(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn link_da_conta() {
        let c = de_link("http://exemplo.dev:8080/get.php?username=ana&password=s%40nha&type=m3u_plus&output=ts").unwrap();
        assert_eq!(c, Conta { base: "http://exemplo.dev:8080".into(), usuario: "ana".into(), senha: "s@nha".into() });
        assert_eq!(c.canal("12", "m3u8"), "http://exemplo.dev:8080/live/ana/s%40nha/12.m3u8");
        assert_eq!(c.filme("7", "mp4"), "http://exemplo.dev:8080/movie/ana/s%40nha/7.mp4");
        assert_eq!(c.episodio("9", "mkv"), "http://exemplo.dev:8080/series/ana/s%40nha/9.mkv");
        assert_eq!(de_link(&c.link_m3u()), Some(c));
        assert_eq!(de_link("https://exemplo.dev/lista.m3u"), None);
        assert_eq!(de_link("https://exemplo.dev/get.php?username=ana"), None);
        assert_eq!(de_link("https://exemplo.dev/get.php?username=&password=x"), None);
        assert_eq!(de_link("lixo"), None);
    }

    #[test]
    fn formulario() {
        assert_eq!(
            link_do_formulario("10.0.2.2:8765", "teste", "senha123").unwrap(),
            "http://10.0.2.2:8765/get.php?username=teste&password=senha123&type=m3u_plus&output=ts"
        );
        assert_eq!(link_do_formulario(" https://x.dev/lista.m3u ", "", "").unwrap(), "https://x.dev/lista.m3u");
        assert!(link_do_formulario("x.dev", "ana", "").is_err());
        assert!(link_do_formulario("a b", "ana", "x").is_err());
    }

    #[test]
    fn entrada() {
        assert_eq!(verificar_entrada(r#"{"user_info":{"auth":1,"status":"Active","exp_date":"4102444800"}}"#), Entrada::Ok);
        assert_eq!(verificar_entrada(r#"{"user_info":{"auth":"1","status":"Active","exp_date":null}}"#), Entrada::Ok);
        assert_eq!(verificar_entrada(r#"{"user_info":{"auth":0}}"#), Entrada::Recusada("Usuário ou senha incorretos"));
        assert_eq!(verificar_entrada(r#"{"user_info":{"auth":1,"status":"Expired"}}"#), Entrada::Recusada("Essa conta não está ativa"));
        assert_eq!(verificar_entrada(r#"{"user_info":{"auth":1,"status":"Active","exp_date":"1000"}}"#), Entrada::Recusada("Essa conta venceu"));
        assert_eq!(verificar_entrada("<html>painel</html>"), Entrada::SemApi);
        assert_eq!(verificar_entrada(r#"{"outra":"coisa"}"#), Entrada::SemApi);
    }

    #[test]
    fn canais_da_api_com_categorias() {
        let conta = Conta { base: "http://s.dev:80".into(), usuario: "u".into(), senha: "p".into() };
        let cats = r#"[{"category_id":"1","category_name":"Abertos"},{"category_id":"3","category_name":"Adultos +18"}]"#;
        let streams = r#"[
            {"stream_id":10,"name":"Globo","stream_icon":"http://img/globo.png","category_id":"1"},
            {"stream_id":11,"name":"Globo","stream_icon":"","category_id":"1"},
            {"stream_id":12,"name":"Canal XXX","stream_icon":"","category_id":"3"},
            {"stream_id":13,"name":"Sem categoria","category_id":"99"},
            {"name":"Sem id"}]"#;
        let c = canais_da_api(&conta, streams, cats, 20_000);
        assert_eq!(c.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>(), ["Globo", "Canal XXX", "Sem categoria"]);
        assert_eq!(c[0].grupo.as_deref(), Some("Abertos"));
        assert_eq!(c[0].logo.as_deref(), Some("http://img/globo.png"));
        let urls: Vec<&str> = c[0].fontes.iter().map(|f| f.url.as_str()).collect();
        assert_eq!(urls, ["http://s.dev:80/live/u/p/10.m3u8", "http://s.dev:80/live/u/p/10.ts", "http://s.dev:80/live/u/p/11.m3u8", "http://s.dev:80/live/u/p/11.ts"]);
        assert_eq!(c[2].grupo, None);
        assert!(canais_da_api(&conta, "lixo", "[]", 10).is_empty());
    }

    #[test]
    fn fluxo_completo() {
        let conta = Conta { base: "http://s.dev".into(), usuario: "u".into(), senha: "p".into() };
        let ler = |t: &str| -> Vec<CanalM3u> {
            t.lines().filter(|l| l.starts_with("#EXTINF")).map(|l| CanalM3u { nome: l.rsplit(',').next().unwrap().into(), grupo: None, logo: None, fontes: vec![] }).collect()
        };
        let boa = |u: &str| -> Result<String, Erro> {
            if u.contains("get_live_categories") { Ok(r#"[{"category_id":"1","category_name":"A"}]"#.into()) }
            else if u.contains("get_live_streams") { Ok(r#"[{"stream_id":1,"name":"Canal 1","category_id":"1"}]"#.into()) }
            else { Ok(r#"{"user_info":{"auth":1,"status":"Active"}}"#.into()) }
        };
        assert_eq!(canais(&conta, &boa, &ler).unwrap()[0].nome, "Canal 1");
        let recusa = |_: &str| -> Result<String, Erro> { Ok(r#"{"user_info":{"auth":0}}"#.into()) };
        assert_eq!(canais(&conta, &recusa, &ler), Err(Erro::Conta("Usuário ou senha incorretos")));
        let sem_api = |u: &str| -> Result<String, Erro> {
            if u.contains("get.php") { Ok("#EXTM3U\n#EXTINF:-1,Canal M\nhttp://s.dev/live/u/p/5\n".into()) } else { Ok("<html>".into()) }
        };
        assert_eq!(canais(&conta, &sem_api, &ler).unwrap()[0].nome, "Canal M");
        let rede = |_: &str| -> Result<String, Erro> { Err(Erro::Download) };
        assert_eq!(canais(&conta, &rede, &ler), Err(Erro::Conta("Não consegui falar com o servidor")));
    }
}
