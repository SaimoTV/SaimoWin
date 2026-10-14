//! Os jogos de hoje e dos próximos dias que passam em algum canal da lista,
//! com a mesma lógica da tela de Eventos da TV Box e do celular.
//!
//! Três fontes: a agenda da embedtv diz em que canal cada jogo passa; a do
//! monitor traz os jogos dos canais de streaming (CazéTV, Disney+, Prime,
//! Paramount+, GOAT, ge tv, N Sports, SportyNet) que a embedtv não tem, no
//! mesmo formato e com placar; os placares da ESPN dizem se já começou, o
//! placar e o tempo de jogo, e trazem escudos melhores. Só entra jogo que tem canal (sem canal não há o que
//! assistir), e jogo encerrado sai.

use chrono::{Datelike, Local, TimeZone};
use serde_json::Value;

const EMBED: &str = "https://embedtv.cc/api/events";
const STREAMING: &str = "https://saimo-monitor.gabrielsaimo68.workers.dev/v1/eventos";
const ESPN: [&str; 23] = [
    "soccer/bra.1", "soccer/bra.2", "soccer/bra.copa_do_brazil", "soccer/conmebol.libertadores",
    "soccer/conmebol.sudamericana", "soccer/uefa.champions", "soccer/uefa.europa", "soccer/eng.1",
    "soccer/esp.1", "soccer/ita.1", "soccer/ger.1", "soccer/fra.1", "soccer/por.1", "soccer/ned.1",
    "soccer/arg.1", "soccer/ksa.1", "soccer/fifa.world", "soccer/fifa.worldq.conmebol",
    "basketball/nba", "football/nfl", "baseball/mlb", "hockey/nhl", "mma/ufc",
];
/// Palavras que não dizem qual é o time: "SE Palmeiras" e "Palmeiras" casam.
const SIGLAS: [&str; 25] = [
    "fc", "ec", "sc", "ac", "afc", "cf", "cd", "club", "clube", "esporte", "sport", "futebol", "de", "do",
    "da", "sp", "rj", "mg", "rs", "pr", "ba", "pe", "ce", "go", "sad",
];
const HORA: i64 = 3_600_000;

/// Os grupos da tela, nesta ordem.
pub const GRUPOS: [&str; 5] = ["Ao vivo agora", "Começa em breve", "Hoje mais tarde", "Amanhã", "Próximos dias"];

#[derive(Clone)]
pub struct Jogo {
    pub casa: String,
    pub fora: String,
    pub casa_logo: String,
    pub fora_logo: String,
    pub liga: String,
    pub liga_logo: String,
    /// Milissegundos desde 1970, como nos outros aplicativos.
    pub inicio: i64,
    pub fim: i64,
    /// O nome do canal na lista: é por ele que o cartão abre o canal.
    pub canal: String,
    pub canal_logo: Option<String>,
    /// "pre", "in" ou "post", quando a ESPN conhece o jogo.
    pub estado: String,
    pub placar: String,
    pub tempo: String,
}

impl Jogo {
    pub fn ao_vivo(&self, agora: i64) -> bool {
        self.estado == "in" || (self.estado != "post" && agora >= self.inicio && agora <= self.fim)
    }

    pub fn acabou(&self, agora: i64) -> bool {
        self.estado == "post" || agora > self.fim
    }
}

/// O placar de um jogo da ESPN.
struct Placar {
    casa: String,
    fora: String,
    casa_logo: String,
    fora_logo: String,
    liga_logo: String,
    inicio: i64,
    estado: String,
    /// Os dois números, da casa e de fora, para virar quando a agenda inverte.
    gols: Option<(String, String)>,
    tempo: String,
}

pub fn agora() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// A agenda com os placares, só com os jogos dos `canais` (nome e logo).
/// `None` quando a agenda não veio.
pub fn buscar(canais: &[(String, Option<String>)]) -> Option<Vec<Jogo>> {
    // As 25 chamadas saem juntas: em fila, a tela esperaria a soma de todas.
    let (agenda, streaming, placares) = std::thread::scope(|escopo| {
        let agenda = escopo.spawn(|| crate::rede::json(EMBED));
        let streaming = escopo.spawn(|| crate::rede::json(STREAMING));
        let placares: Vec<_> = ESPN
            .iter()
            .map(|liga| {
                escopo.spawn(move || {
                    crate::rede::json(&format!("https://site.api.espn.com/apis/site/v2/sports/{liga}/scoreboard"))
                })
            })
            .collect();
        let placares: Vec<Value> = placares.into_iter().filter_map(|p| p.join().ok().flatten()).collect();
        (agenda.join().ok().flatten(), streaming.join().ok().flatten(), placares)
    });
    let agenda = agenda?;
    let espn: Vec<Placar> = placares.iter().flat_map(ler_espn).collect();
    let mut jogos = ler_agenda(&agenda, canais);
    // Jogo do monitor que a embedtv já tem (mesmos times, começo a menos de
    // meia hora) não entra de novo.
    let extras: Vec<Jogo> = streaming
        .map(|s| ler_agenda(&s, canais))
        .unwrap_or_default()
        .into_iter()
        .filter(|x| {
            !jogos.iter().any(|e| {
                (e.inicio - x.inicio).abs() < HORA / 2
                    && ((mesmo_time(&e.casa, &x.casa) && mesmo_time(&e.fora, &x.fora))
                        || (mesmo_time(&e.casa, &x.fora) && mesmo_time(&e.fora, &x.casa)))
            })
        })
        .collect();
    jogos.extend(extras);
    for jogo in &mut jogos {
        // O mesmo jogo na ESPN: mesmos times (em qualquer ordem) e começo
        // a menos de quatro horas um do outro.
        let par = espn.iter().find_map(|e| {
            if (e.inicio - jogo.inicio).abs() >= 4 * HORA {
                return None;
            }
            if mesmo_time(&e.casa, &jogo.casa) && mesmo_time(&e.fora, &jogo.fora) {
                Some((e, false))
            } else if mesmo_time(&e.casa, &jogo.fora) && mesmo_time(&e.fora, &jogo.casa) {
                Some((e, true))
            } else {
                None
            }
        });
        let Some((par, invertido)) = par else { continue };
        // Na ordem da agenda: o escudo e o gol de cada lado ficam com o nome
        // que aparece daquele lado do cartão.
        let (casa_logo, fora_logo) = if invertido { (&par.fora_logo, &par.casa_logo) } else { (&par.casa_logo, &par.fora_logo) };
        if !casa_logo.is_empty() {
            jogo.casa_logo = casa_logo.clone();
        }
        if !fora_logo.is_empty() {
            jogo.fora_logo = fora_logo.clone();
        }
        if jogo.liga_logo.is_empty() {
            jogo.liga_logo = par.liga_logo.clone();
        }
        jogo.estado = par.estado.clone();
        jogo.placar = match &par.gols {
            Some((a, b)) if invertido => format!("{b} x {a}"),
            Some((a, b)) => format!("{a} x {b}"),
            None => String::new(),
        };
        jogo.tempo = par.tempo.clone();
    }
    Some(jogos)
}

/// Os jogos que ainda não acabaram, em ordem de início, separados pelos
/// `GRUPOS`: o número é a posição do grupo. A busca filtra por time, liga e
/// canal.
pub fn agrupar(jogos: &[Jogo], agora: i64, busca: &str) -> Vec<(usize, Vec<Jogo>)> {
    let busca = crate::catalogo::chave_de_ordem(busca.trim());
    let mut restantes: Vec<&Jogo> = jogos
        .iter()
        .filter(|j| !j.acabou(agora))
        .filter(|j| {
            busca.is_empty()
                || [&j.casa, &j.fora, &j.liga, &j.canal]
                    .iter()
                    .any(|t| crate::catalogo::chave_de_ordem(t).contains(&busca))
        })
        .collect();
    restantes.sort_by_key(|j| j.inicio);
    let mut grupos: Vec<Vec<Jogo>> = vec![Vec::new(); GRUPOS.len()];
    for j in restantes {
        grupos[grupo_de(j, agora)].push(j.clone());
    }
    grupos.into_iter().enumerate().filter(|(_, lista)| !lista.is_empty()).collect()
}

fn grupo_de(j: &Jogo, agora: i64) -> usize {
    if j.ao_vivo(agora) {
        return 0;
    }
    if j.inicio - agora <= 2 * HORA {
        return 1;
    }
    match dia_de(j.inicio) - dia_de(agora) {
        d if d <= 0 => 2,
        1 => 3,
        _ => 4,
    }
}

/// O dia no relógio de quem está vendo, contado desde o começo do calendário.
fn dia_de(ms: i64) -> i64 {
    Local.timestamp_millis_opt(ms).single().map(|d| d.num_days_from_ce() as i64).unwrap_or(0)
}

/// "21:30", "amanhã 16:00" ou "sáb 16:00".
pub fn quando(inicio: i64, agora: i64) -> String {
    let Some(data) = Local.timestamp_millis_opt(inicio).single() else { return String::new() };
    let hora = data.format("%H:%M").to_string();
    match dia_de(inicio) - dia_de(agora) {
        d if d <= 0 => hora,
        1 => format!("amanhã {hora}"),
        _ => {
            const SEMANA: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
            format!("{} {hora}", SEMANA[data.weekday().num_days_from_sunday() as usize])
        }
    }
}

/// "N jogos nos canais da lista · M ao vivo".
pub fn resumo(grupos: &[(usize, Vec<Jogo>)]) -> String {
    let total: usize = grupos.iter().map(|(_, l)| l.len()).sum();
    let vivos = grupos.iter().find(|(g, _)| *g == 0).map(|(_, l)| l.len()).unwrap_or(0);
    let jogos = if total == 1 { "jogo" } else { "jogos" };
    format!("{total} {jogos} nos canais da lista · {vivos} ao vivo")
}

/// Escudos e logos da agenda vêm do CDN do 365scores pedidos em 24 ou 34 px
/// (`w_24,h_24`, 48 px com `dpr_2`) e em qualidade econômica: borrados no
/// cartão. O CDN monta a imagem pelo endereço, então pedir 256 px e qualidade
/// máxima traz o original (350 a 415 px); `c_limit` nunca amplia além dele.
pub fn em_alta_resolucao(url: &str) -> String {
    const MARCA: &str = "imagecache.365scores.com/image/upload/";
    let Some(pos) = url.find(MARCA) else { return url.to_string() };
    let inicio = pos + MARCA.len();
    let fim = url[inicio..].find('/').map(|b| inicio + b).unwrap_or(url.len());
    let ajustes: Vec<&str> = url[inicio..fim]
        .split(',')
        .map(|p| {
            if p.starts_with("w_") {
                "w_256"
            } else if p.starts_with("h_") {
                "h_256"
            } else if p == "q_auto:eco" {
                "q_auto:best"
            } else {
                p
            }
        })
        .collect();
    format!("{}{}{}", &url[..inicio], ajustes.join(","), &url[fim..])
}

/// O canal pelo fim do endereço do player ("…/premiere-fc" é o Premiere FC).
fn canal_do(url: &str, canais: &[(String, Option<String>)]) -> Option<(String, Option<String>)> {
    let slug: String = url.rsplit('/').next().unwrap_or("").to_lowercase().chars().filter(|c| *c != '-' && *c != ' ').collect();
    if slug.is_empty() {
        return None;
    }
    let chave = |nome: &str| nome.to_lowercase().replace(' ', "");
    canais
        .iter()
        .find(|(nome, _)| chave(nome) == slug)
        .or_else(|| canais.iter().find(|(nome, _)| chave(nome).contains(&slug)))
        .cloned()
}

/// Milissegundos de uma data da agenda ("2026-10-13T21:30:00-03:00") ou da
/// ESPN, que manda sem os segundos ("2026-10-16T23:00Z").
fn ler_data(texto: &str) -> Option<i64> {
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(texto) {
        return Some(d.timestamp_millis());
    }
    let base = texto.strip_suffix('Z')?;
    chrono::DateTime::parse_from_rfc3339(&format!("{base}:00Z")).ok().map(|d| d.timestamp_millis())
}

fn texto(v: &Value, caminho: &[&str]) -> String {
    caminho.iter().try_fold(v, |v, chave| v.get(chave)).and_then(Value::as_str).unwrap_or("").to_string()
}

fn ler_agenda(agenda: &Value, canais: &[(String, Option<String>)]) -> Vec<Jogo> {
    let Some(lista) = agenda.as_array() else { return Vec::new() };
    let mut vistos = std::collections::HashSet::new();
    let mut jogos = Vec::new();
    for o in lista {
        // Jogo com dados faltando fica de fora.
        let Some(player) = o.get("players").and_then(|p| p.get(0)).and_then(Value::as_str) else { continue };
        let Some((canal, canal_logo)) = canal_do(player, canais) else { continue };
        let Some(inicio) = ler_data(&texto(o, &["time_start"])) else { continue };
        let casa = texto(o, &["teams", "home", "name"]);
        let fora = texto(o, &["teams", "away", "name"]);
        if casa.is_empty() || fora.is_empty() {
            continue;
        }
        if !vistos.insert(format!("{casa} x {fora}|{inicio}")) {
            continue;
        }
        jogos.push(Jogo {
            casa_logo: em_alta_resolucao(&texto(o, &["teams", "home", "image"])),
            fora_logo: em_alta_resolucao(&texto(o, &["teams", "away", "image"])),
            liga: texto(o, &["league", "name"]),
            liga_logo: em_alta_resolucao(&texto(o, &["league", "image"])),
            inicio,
            fim: ler_data(&texto(o, &["time_end"])).unwrap_or(inicio + 2 * HORA),
            casa,
            fora,
            canal,
            canal_logo,
            // Só a agenda do monitor manda o placar; a ESPN, quando acha o jogo, vale mais.
            estado: texto(o, &["estado"]),
            placar: texto(o, &["placar"]),
            tempo: texto(o, &["tempo"]),
        });
    }
    jogos
}

fn ler_espn(dados: &Value) -> Vec<Placar> {
    let liga_logo = dados
        .pointer("/leagues/0/logos/0/href")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let Some(eventos) = dados.get("events").and_then(Value::as_array) else { return Vec::new() };
    let mut saida = Vec::new();
    for ev in eventos {
        let Some(times) = ev.pointer("/competitions/0/competitors").and_then(Value::as_array) else { continue };
        let casa = times.iter().find(|c| c.get("homeAway").and_then(Value::as_str) == Some("home"));
        let fora = times.iter().find(|c| c.get("homeAway").and_then(Value::as_str) != Some("home"));
        let (Some(casa), Some(fora)) = (casa, fora) else { continue };
        let estado = texto(ev, &["status", "type", "state"]);
        let gols = |c: &Value| match c.get("score") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        saida.push(Placar {
            casa: texto(casa, &["team", "name"]),
            fora: texto(fora, &["team", "name"]),
            casa_logo: texto(casa, &["team", "logo"]),
            fora_logo: texto(fora, &["team", "logo"]),
            liga_logo: liga_logo.clone(),
            inicio: ler_data(&texto(ev, &["date"])).unwrap_or(0),
            gols: (estado != "pre").then(|| (gols(casa), gols(fora))),
            tempo: if estado == "in" { texto(ev, &["status", "type", "shortDetail"]) } else { String::new() },
            estado,
        });
    }
    saida
}

/// Sem acento, sem as siglas e sem o que não é letra ou número.
fn chave_time(nome: &str) -> String {
    crate::catalogo::chave_de_ordem(nome)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty() && !SIGLAS.contains(p))
        .collect()
}

fn mesmo_time(a: &str, b: &str) -> bool {
    let x = chave_time(a);
    let y = chave_time(b);
    if x.is_empty() || y.is_empty() {
        return false;
    }
    x == y || (x.len() >= 4 && y.contains(&x)) || (y.len() >= 4 && x.contains(&y))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escudo_em_alta_resolucao() {
        let url = "https://imagecache.365scores.com/image/upload/f_png,w_34,h_34,c_limit,q_auto:eco,dpr_2,d_Competitors:default1.png/v2/Competitors/46472";
        assert_eq!(
            em_alta_resolucao(url),
            "https://imagecache.365scores.com/image/upload/f_png,w_256,h_256,c_limit,q_auto:best,dpr_2,d_Competitors:default1.png/v2/Competitors/46472"
        );
        assert_eq!(em_alta_resolucao("https://a.espncdn.com/x.png"), "https://a.espncdn.com/x.png");
    }

    #[test]
    fn times_com_e_sem_sigla() {
        assert!(mesmo_time("SE Palmeiras", "Palmeiras"));
        assert!(mesmo_time("Grêmio", "Gremio"));
        assert!(mesmo_time("São Paulo FC", "Sao Paulo"));
        assert!(!mesmo_time("Flamengo", "Fluminense"));
    }

    #[test]
    fn canal_pelo_endereco() {
        let canais = vec![("Premiere FC".to_string(), None), ("ESPN 2".to_string(), None), ("ESPN".to_string(), None)];
        assert_eq!(canal_do("https://x.embedtv.cc/premiere-fc", &canais).map(|c| c.0).as_deref(), Some("Premiere FC"));
        assert_eq!(canal_do("https://x.embedtv.cc/espn2", &canais).map(|c| c.0).as_deref(), Some("ESPN 2"));
        assert!(canal_do("https://x.embedtv.cc/sportv", &canais).is_none());
    }

    #[test]
    fn data_da_espn_sem_segundos() {
        assert_eq!(ler_data("2026-10-16T23:00Z"), ler_data("2026-10-16T20:00:00-03:00"));
        assert!(ler_data("").is_none());
    }

    #[test]
    #[ignore = "usa a rede"]
    fn agenda_com_os_canais_publicados() {
        let canais: Vec<(String, Option<String>)> = crate::catalogo::baixar("catalogo.txt")
            .expect("catálogo")
            .into_iter()
            .filter(|c| c.secao() != "Adulto")
            .map(|c| (c.nome, c.logo))
            .collect();
        let jogos = buscar(&canais).expect("agenda");
        let grupos = agrupar(&jogos, agora(), "");
        println!("{}", resumo(&grupos));
        for (g, lista) in &grupos {
            for j in lista {
                println!("{} | {} x {} | {} | {} {} {} | {}", GRUPOS[*g], j.casa, j.fora, j.canal, j.estado, j.placar, j.tempo, j.casa_logo);
            }
        }
    }

    #[test]
    fn resumo_no_singular() {
        let jogo = Jogo {
            casa: "A".into(), fora: "B".into(), casa_logo: String::new(), fora_logo: String::new(),
            liga: String::new(), liga_logo: String::new(), inicio: 0, fim: i64::MAX,
            canal: "C".into(), canal_logo: None, estado: "in".into(), placar: "1 x 0".into(), tempo: String::new(),
        };
        let grupos = agrupar(&[jogo], 1, "");
        assert_eq!(resumo(&grupos), "1 jogo nos canais da lista · 1 ao vivo");
    }
}
