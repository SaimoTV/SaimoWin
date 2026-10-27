//! Servidores desligados à mão, no painel do monitor.
//!
//! Quando um provedor cai, cai inteiro: não é a fonte 3 de um canal que
//! morreu, é o servidor que parou de responder para todo mundo. Editar o
//! catálogo publicado a cada queda é lento e some com o link, que depois
//! precisa voltar. Desligar o servidor no painel some com ele de todo canal e
//! de todo filme, em todos os aplicativos, e religar devolve tudo.
//!
//! A lista é baixada sem chave nenhuma: são nomes de servidor, que o catálogo
//! publicado já mostra, e exigir segredo significaria embutir um segredo num
//! programa que qualquer um baixa.
//!
//! Sem rede a lista fica vazia e nada é escondido — o erro certo a cometer: um
//! canal a mais na tela é melhor que a lista inteira sumindo porque o monitor
//! não respondeu.

use std::collections::HashSet;
use std::sync::RwLock;

const ENDERECO: &str = "https://saimo-tv.pages.dev/fontes-desativadas.json";
/// Uma leitura por hora. Cada aparelho perguntando de dois em dois minutos
/// custava caro no monitor (cobrado por pedido); desligar um servidor agora
/// leva até uma hora para chegar a todos. A última lista boa fica guardada
/// com a hora em que veio, e reabrir o app dentro da hora não pergunta de novo.
const VALIDADE_S: u64 = 3600;
const ARQUIVO: &str = "fontes_desativadas.json";

static HOSTS: RwLock<Option<HashSet<String>>> = RwLock::new(None);
/// Segundos Unix da última leitura boa (0 se nunca).
static LIDO_EM: RwLock<Option<u64>> = RwLock::new(None);

fn agora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Na primeira consulta, a lista guardada da última vez.
fn carregar_guardado() {
    if HOSTS.read().map(|h| h.is_some()).unwrap_or(true) {
        return;
    }
    let lido: Option<serde_json::Value> = std::fs::read_to_string(crate::catalogo::pasta().join(ARQUIVO))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let hosts: HashSet<String> = lido
        .as_ref()
        .and_then(|v| v.get("desativados"))
        .and_then(|d| d.as_array())
        .map(|l| l.iter().filter_map(|v| v.as_str()).map(str::to_string).collect())
        .unwrap_or_default();
    let em = lido.as_ref().and_then(|v| v.get("lidoEm")).and_then(|v| v.as_u64()).unwrap_or(0);
    if let Ok(mut guarda) = HOSTS.write() {
        if guarda.is_none() {
            *guarda = Some(hosts);
            if let Ok(mut marca) = LIDO_EM.write() {
                // Relógio que voltou para trás não prende a lista velha para sempre.
                *marca = Some(if em <= agora_s() { em } else { 0 });
            }
        }
    }
}

fn hosts() -> HashSet<String> {
    carregar_guardado();
    HOSTS.read().ok().and_then(|h| h.clone()).unwrap_or_default()
}

/// Busca a lista quando ela envelheceu. Devolve verdadeiro quando mudou, que é
/// quando quem chamou precisa remontar a lista de canais.
pub fn atualizar() -> bool {
    carregar_guardado();
    if let Ok(marca) = LIDO_EM.read() {
        if marca.map(|m| agora_s().saturating_sub(m) < VALIDADE_S).unwrap_or(false) {
            return false;
        }
    }
    let Some(corpo) = crate::rede::json(ENDERECO) else { return false };
    let Some(lista) = corpo.get("desativados").and_then(|d| d.as_array()) else { return false };

    let agora = agora_s();
    if let Ok(mut marca) = LIDO_EM.write() {
        *marca = Some(agora);
    }
    let novos: HashSet<String> = lista
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let mut ordenados: Vec<&String> = novos.iter().collect();
    ordenados.sort_unstable();
    let guardar = serde_json::json!({ "desativados": ordenados, "lidoEm": agora });
    let _ = std::fs::write(crate::catalogo::pasta().join(ARQUIVO), guardar.to_string());

    let mudou = hosts() != novos;
    if mudou {
        if novos.is_empty() {
            eprintln!("nenhum servidor desligado");
        } else {
            let mut nomes: Vec<&str> = novos.iter().map(|s| s.as_str()).collect();
            nomes.sort_unstable();
            eprintln!("servidores desligados: {}", nomes.join(", "));
        }
        if let Ok(mut guarda) = HOSTS.write() {
            *guarda = Some(novos);
        }
    }
    mudou
}

/// O servidor de um endereço, em minúsculas: o pedaço entre "//" e a primeira
/// barra, sem usuário nem porta.
pub fn servidor(url: &str) -> Option<String> {
    let resto = url.split("://").nth(1).unwrap_or(url);
    let autoridade = resto.split(['/', '?', '#']).next()?;
    let sem_usuario = autoridade.rsplit('@').next()?;
    let so_host = sem_usuario.split(':').next()?;
    if so_host.is_empty() { None } else { Some(so_host.to_lowercase()) }
}

pub fn desligado(url: &str) -> bool {
    if url.contains("/error.txt") || url.contains(".s25-cloudfront-net.lat") {
        return true;
    }
    let hosts = hosts();
    if hosts.is_empty() {
        return false;
    }
    servidor(url).map(|h| hosts.contains(&h)).unwrap_or(false)
}

/// Os endereços de um filme ou episódio sem os que estão desligados.
pub fn peneirar(urls: Vec<String>) -> Vec<String> {
    if hosts().is_empty() {
        return urls;
    }
    urls.into_iter().filter(|u| !desligado(u)).collect()
}

/// O catálogo sem o que está desligado.
///
/// Canal que fica sem nenhuma fonte sai da lista: ele não abriria mesmo, e
/// deixá-lo ali só rende clique frustrado. Volta sozinho quando o servidor for
/// religado.
pub fn peneirar_canais(canais: Vec<crate::catalogo::Canal>) -> Vec<crate::catalogo::Canal> {
    if hosts().is_empty() {
        return canais;
    }
    canais
        .into_iter()
        .filter_map(|mut canal| {
            canal.fontes.retain(|f| !desligado(&f.url));
            if canal.fontes.is_empty() { None } else { Some(canal) }
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::servidor;

    #[test]
    fn tira_o_servidor_do_endereco() {
        assert_eq!(servidor("https://proxy.nr3.com/a/b.m3u8").as_deref(), Some("proxy.nr3.com"));
        assert_eq!(servidor("http://Proxy.NR3.com:8080/x").as_deref(), Some("proxy.nr3.com"));
        assert_eq!(servidor("https://ana:senha@casa.tv/p").as_deref(), Some("casa.tv"));
        assert_eq!(servidor("nada"), Some("nada".into()));
    }
}
