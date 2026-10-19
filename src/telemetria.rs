//! O que o Saimo Monitor fica sabendo deste PC.
//!
//! O mesmo de sempre: que o app abriu (versão, modelo e sistema), o que está
//! tocando de tempos em tempos, e quando uma fonte falha ou um canal cai. O
//! aparelho é um UUID sorteado na primeira abertura, guardado ao lado do cache.
//! Sem rede o monitor fica sem o dado e o app continua igual.
//!
//! Telemetria por amostra: o monitor é cobrado por pedido, então só uma fatia
//! estável dos aparelhos fala com ele. A conta é a mesma do servidor
//! (`amostrado` no worker.js): FNV-1a de 32 bits sobre o id do aparelho em
//! minúsculas, e entra quem tem `hash % 10000 < amostraPct * 100`. Fora da
//! amostra nada sai — nem olá, nem batida, nem evento —, menos o relatório de
//! quebra. O percentual vem de `telemetria.json`, arquivo estático do monitor
//! (servido de graça, sem rodar o Worker), lido no máximo uma vez por dia.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const BASE: &str = "https://saimo-monitor.gabrielsaimo68.workers.dev/v1";
const AMOSTRA_URL: &str = "https://saimo-monitor.gabrielsaimo68.workers.dev/telemetria.json";
/// Sem leitura nenhuma do percentual, vale o mesmo do servidor hoje.
const AMOSTRA_PADRAO: f64 = 5.0;
/// O percentual é relido no máximo uma vez por dia.
const AMOSTRA_VALIDADE_S: u64 = 24 * 3600;
/// Batida extra (pausa, troca de título, parada) no máximo uma a cada 30 s:
/// quem zapeia ou pausa e volta não vira uma rajada de pedidos.
const INTERVALO_EXTRA: Duration = Duration::from_secs(30);
const PLATAFORMA: &str = "windows";
/// Zapeando, cada canal que passa não vira batida: só quem ficou.
const MINIMO_PARA_CONTAR_S: u64 = 20;

struct Tocando {
    kind: String,
    titulo: String,
    host: Option<String>,
}

struct Estado {
    id: String,
    tocando: Option<Tocando>,
    /// Só conta tempo com o vídeo andando: pausado ou carregando não é
    /// assistir. `acumulado` guarda o que já andou desde a última batida.
    acumulado: Duration,
    rodando_desde: Option<Instant>,
    pausado: bool,
    qualidade: Option<String>,
    travou_desde: Option<Instant>,
    buscas: [Busca; 2],
    ultima_batida: Instant,
    batida_s: u64,
    erros: u32,
    /// Percentual da amostra (0 a 100).
    pct: f64,
    /// O olá do monitor pode dizer de que lado este aparelho está; vale mais
    /// que a conta daqui enquanto o app estiver aberto.
    amostra_servidor: Option<bool>,
    cumprimentou: bool,
    /// Quando saiu a última batida de verdade, para segurar as extras.
    batida_enviada: Option<Instant>,
}

#[derive(Default)]
struct Busca {
    texto: String,
    desde: Option<Instant>,
    avisada: bool,
}

impl Estado {
    fn na_amostra(&self) -> bool {
        self.amostra_servidor.unwrap_or_else(|| amostrado(&self.id, self.pct))
    }

    fn segundos_rodando(&self) -> u64 {
        (self.acumulado + self.rodando_desde.map(|d| d.elapsed()).unwrap_or_default()).as_secs()
    }
}

static ESTADO: Mutex<Option<Estado>> = Mutex::new(None);

/// FNV-1a de 32 bits, sem sinal, sobre os bytes UTF-8.
fn fnv1a(texto: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in texto.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

/// O mesmo lado da amostra que o servidor calcula para este id.
fn amostrado(id: &str, pct: f64) -> bool {
    ((fnv1a(&id.to_lowercase()) % 10000) as f64) < pct * 100.0
}

fn pct_valido(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(|v| v.as_f64()).filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
}

fn agora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn arquivo_amostra() -> std::path::PathBuf {
    crate::catalogo::pasta().join("amostra.json")
}

/// O percentual guardado e quando foi lido (segundos Unix; 0 se nunca).
fn amostra_guardada() -> (f64, u64) {
    let lido: Option<serde_json::Value> = std::fs::read_to_string(arquivo_amostra())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let pct = pct_valido(lido.as_ref().and_then(|v| v.get("amostraPct"))).unwrap_or(AMOSTRA_PADRAO);
    let em = lido.as_ref().and_then(|v| v.get("lidoEm")).and_then(|v| v.as_u64()).unwrap_or(0);
    (pct, em)
}

fn guardar_amostra(pct: f64, lido_em: u64) {
    let corpo = serde_json::json!({ "amostraPct": pct, "lidoEm": lido_em });
    let _ = std::fs::write(arquivo_amostra(), corpo.to_string());
}

/// UUID v4 sem depender de mais uma biblioteca: tempo, endereço de pilha e
/// contador bastam para um número que só identifica esta instalação.
fn sorteia_id() -> String {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pilha = &agora as *const _ as usize as u128;
    let mut semente = agora ^ (pilha << 64) ^ (std::process::id() as u128) << 32;
    let mut digitos = String::new();
    for i in 0..32 {
        semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let nibble = ((semente >> 64) & 0xF) as u8;
        let nibble = match i {
            12 => 4,                    // versão 4
            16 => (nibble & 0x3) | 0x8, // variante
            _ => nibble,
        };
        digitos.push(char::from_digit(nibble as u32, 16).unwrap_or('0'));
        if matches!(i, 7 | 11 | 15 | 19) {
            digitos.push('-');
        }
    }
    digitos
}

fn id_do_aparelho() -> String {
    let arquivo = crate::catalogo::pasta().join("aparelho.txt");
    if let Ok(guardado) = std::fs::read_to_string(&arquivo) {
        let guardado = guardado.trim().to_string();
        if guardado.len() >= 32 {
            return guardado;
        }
    }
    let novo = sorteia_id();
    let _ = std::fs::write(&arquivo, &novo);
    novo
}

fn modelo() -> String {
    // Sem consultar hardware: o que interessa ao painel é separar PC de TV Box.
    std::env::var("PROCESSOR_ARCHITECTURE")
        .map(|arq| format!("PC Windows ({arq})"))
        .unwrap_or_else(|_| "PC Windows".into())
}

fn sistema() -> String {
    std::env::var("OS").unwrap_or_else(|_| "Windows".into())
}

fn host(url: &str) -> Option<String> {
    let sem_esquema = url.split("://").nth(1)?;
    let host = sem_esquema.split('/').next()?.split('@').next_back()?;
    Some(host.split(':').next()?.trim_start_matches("www.").to_string())
}

fn enviar(rota: &str, mut corpo: serde_json::Value) {
    let estado = ESTADO.lock().unwrap();
    let Some(estado) = estado.as_ref() else { return };
    // Fora da amostra só o relatório de quebra sai.
    let quebra = rota == "event" && corpo.get("type").and_then(|t| t.as_str()) == Some("crash");
    if !estado.na_amostra() && !quebra {
        return;
    }
    corpo["deviceId"] = estado.id.clone().into();
    corpo["platform"] = PLATAFORMA.into();
    corpo["version"] = crate::VERSAO.into();
    crate::rede::postar(format!("{BASE}/{rota}"), corpo.to_string());
}

pub fn iniciar() {
    // Teste da interface fora do Windows não deve virar aparelho no painel.
    if std::env::var("SAIMO_SEM_TELEMETRIA").is_ok() {
        return;
    }
    let (pct, lido_em) = amostra_guardada();
    {
        let mut estado = ESTADO.lock().unwrap();
        if estado.is_some() {
            return;
        }
        *estado = Some(Estado {
            id: id_do_aparelho(),
            tocando: None,
            acumulado: Duration::ZERO,
            rodando_desde: None,
            pausado: false,
            qualidade: None,
            travou_desde: None,
            buscas: Default::default(),
            ultima_batida: Instant::now(),
            batida_s: 300,
            erros: 0,
            pct,
            amostra_servidor: None,
            cumprimentou: false,
            batida_enviada: None,
        });
    }
    cumprimentar();
    // O percentual da amostra, uma vez por dia; se mudou e este aparelho
    // passou a entrar, o olá sai agora.
    if agora_s().saturating_sub(lido_em) >= AMOSTRA_VALIDADE_S {
        std::thread::spawn(|| {
            let Some(novo) = crate::rede::json_curto(AMOSTRA_URL)
                .and_then(|v| pct_valido(v.get("amostraPct")))
            else { return };
            guardar_amostra(novo, agora_s());
            if let Some(estado) = ESTADO.lock().unwrap().as_mut() {
                estado.pct = novo;
            }
            cumprimentar();
        });
    }
}

/// O olá, uma vez por abertura e só da amostra.
fn cumprimentar() {
    // O monitor responde de quanto em quanto tempo quer a batida; sem ler a
    // resposta, um ajuste no painel não chegaria neste app.
    let corpo = {
        let mut estado = ESTADO.lock().unwrap();
        let Some(estado) = estado.as_mut() else { return };
        if estado.cumprimentou || !estado.na_amostra() {
            return;
        }
        estado.cumprimentou = true;
        serde_json::json!({
            "deviceId": estado.id,
            "platform": PLATAFORMA,
            "version": crate::VERSAO,
            "model": modelo(),
            "os": sistema(),
            "deviceType": "computador",
            "net": "cabo",
            "lang": std::env::var("LANG").ok(),
        })
    };
    std::thread::spawn(move || {
        let Some(resposta) = crate::rede::postar_e_ler(format!("{BASE}/hello"), corpo.to_string()) else { return };
        let novo_pct = pct_valido(resposta.get("amostraPct"));
        if let Some(pct) = novo_pct {
            guardar_amostra(pct, amostra_guardada().1);
        }
        let mut estado = ESTADO.lock().unwrap();
        let Some(estado) = estado.as_mut() else { return };
        if let Some(segundos) = resposta.get("heartbeatSeconds").and_then(|v| v.as_u64()) {
            if (60..=3600).contains(&segundos) {
                estado.batida_s = segundos;
            }
        }
        if let Some(pct) = novo_pct {
            estado.pct = pct;
        }
        if let Some(dentro) = resposta.get("amostra").and_then(|v| v.as_bool()) {
            estado.amostra_servidor = Some(dentro);
        }
    });
}

/// Chamada a cada quadro: manda a batida quando der a hora, e nada além disso.
pub fn no_relogio() {
    let passou = {
        let estado = ESTADO.lock().unwrap();
        match estado.as_ref() {
            Some(e) => e.ultima_batida.elapsed().as_secs() >= e.batida_s,
            None => return,
        }
    };
    if passou {
        bater(false);
    }
}

/// `extra` é a batida fora do relógio (pausa, troca, parada): sai no máximo
/// uma a cada 30 s. A do relógio e a do fechamento saem sempre.
fn bater(extra: bool) {
    let corpo = {
        let mut estado = ESTADO.lock().unwrap();
        let Some(estado) = estado.as_mut() else { return };
        // Fora da amostra o relógio continua, mas nada sai nem é zerado.
        if !estado.na_amostra() {
            estado.ultima_batida = Instant::now();
            return;
        }
        if extra && estado.batida_enviada.map(|b| b.elapsed() < INTERVALO_EXTRA).unwrap_or(false) {
            return;
        }
        estado.batida_enviada = Some(Instant::now());
        let segundos = if estado.tocando.is_some() { estado.segundos_rodando() } else { 0 };
        estado.acumulado = Duration::ZERO;
        if estado.rodando_desde.is_some() {
            estado.rodando_desde = Some(Instant::now());
        }
        estado.ultima_batida = Instant::now();
        let tocando = match &estado.tocando {
            Some(t) => serde_json::json!({
                "kind": t.kind, "title": t.titulo, "host": t.host,
                "paused": estado.pausado, "quality": estado.qualidade,
            }),
            None => serde_json::Value::Null,
        };
        serde_json::json!({ "seconds": segundos, "playing": tocando })
    };
    enviar("beat", corpo);
}

fn evento(tipo: &str, kind: Option<&str>, titulo: Option<&str>, url: Option<&str>,
          fonte: Option<usize>, detalhe: Option<String>) {
    enviar("event", serde_json::json!({
        "type": tipo,
        "kind": kind,
        "title": titulo,
        "host": url.and_then(host),
        "source": fonte.map(|f| f as i64),
        "detail": detalhe,
    }));
}

/// `nova` é falso quando é só a próxima fonte do mesmo canal depois de uma
/// falha: aí não conta como mais uma abertura.
pub fn comecou(kind: &str, titulo: &str, url: &str, fonte: usize, nova: bool) {
    // O que fecha a conta do canal anterior é decidido com o cadeado na mão,
    // mas a batida só sai depois de soltá-lo: `bater` tranca o mesmo cadeado.
    let fechar_conta = {
        let estado = ESTADO.lock().unwrap();
        match estado.as_ref() {
            Some(e) => {
                e.tocando.as_ref().map(|t| t.titulo != titulo).unwrap_or(false)
                    && e.segundos_rodando() >= MINIMO_PARA_CONTAR_S
            }
            None => return,
        }
    };
    if fechar_conta {
        bater(true);
    }
    {
        let mut estado = ESTADO.lock().unwrap();
        let Some(estado) = estado.as_mut() else { return };
        if estado.tocando.as_ref().map(|t| t.titulo != titulo).unwrap_or(true) {
            estado.acumulado = Duration::ZERO;
            estado.rodando_desde = None;
            estado.pausado = false;
            estado.qualidade = None;
        }
        estado.travou_desde = None;
        estado.tocando = Some(Tocando {
            kind: kind.to_string(),
            titulo: titulo.to_string(),
            host: host(url),
        });
    }
    if nova {
        evento("play_start", Some(kind), Some(titulo), Some(url), Some(fonte), None);
    }
}

pub fn tocou(kind: &str, titulo: &str, url: &str, fonte: usize, ms: u128) {
    enviar("event", serde_json::json!({
        "type": "play_ok",
        "kind": kind,
        "title": titulo,
        "host": host(url),
        "source": fonte as i64,
        "detail": format!("{ms} ms"),
        "ms": ms as u64,
    }));
}

/// Chamada a cada quadro com o que o player está fazendo agora.
///
/// `rodando` é o vídeo confirmado na tela; `carregando` é o mpv parado
/// esperando dados. Pausar ou voltar manda batida na hora, para o painel não
/// mostrar como assistindo quem pausou. Carregar depois de já ter começado é
/// travamento, e vai para o painel com a duração.
pub fn video(rodando: bool, pausado: bool, carregando: bool, pulou: bool, qualidade: Option<String>) {
    let mut bater_agora = false;
    let mut travou: Option<(String, String, Option<String>, u128)> = None;
    {
        let mut estado = ESTADO.lock().unwrap();
        let Some(e) = estado.as_mut() else { return };
        if e.tocando.is_none() {
            return;
        }
        let andando = rodando && !pausado && !carregando;
        match (andando, e.rodando_desde) {
            (true, None) => e.rodando_desde = Some(Instant::now()),
            (false, Some(desde)) => {
                e.acumulado += desde.elapsed();
                e.rodando_desde = None;
            }
            _ => {}
        }
        if rodando && pausado != e.pausado {
            e.pausado = pausado;
            bater_agora = true;
        }
        if rodando && qualidade.is_some() {
            e.qualidade = qualidade;
        }
        if pulou {
            e.travou_desde = None;
        } else if rodando && carregando && !pausado {
            e.travou_desde.get_or_insert_with(Instant::now);
        } else if let Some(desde) = e.travou_desde.take() {
            let ms = desde.elapsed().as_millis();
            if ms >= 500 && !pausado {
                if let Some(t) = e.tocando.as_ref() {
                    travou = Some((t.kind.clone(), t.titulo.clone(), t.host.clone(), ms));
                }
            }
        }
    }
    if bater_agora {
        bater(true);
    }
    if let Some((kind, titulo, host, ms)) = travou {
        enviar("event", serde_json::json!({
            "type": "stall", "kind": kind, "title": titulo, "host": host,
            "ms": ms as u64, "detail": format!("{ms} ms"),
        }));
    }
}

/// Busca que ficou parada 2 s sem resultado nenhum: o painel mostra o que
/// procuram e não acham. `qual` separa a busca de canais (0) da do acervo (1).
pub fn busca(qual: usize, texto: &str, achou: bool) {
    let texto = texto.trim();
    let mut avisar = None;
    {
        let mut estado = ESTADO.lock().unwrap();
        let Some(e) = estado.as_mut() else { return };
        let Some(b) = e.buscas.get_mut(qual) else { return };
        if b.texto != texto {
            b.texto = texto.to_string();
            b.desde = Some(Instant::now());
            b.avisada = false;
            return;
        }
        let parada = b.desde.map(|d| d.elapsed() >= Duration::from_secs(2)).unwrap_or(false);
        if parada && !b.avisada && !achou && texto.chars().count() >= 3 {
            b.avisada = true;
            avisar = Some(texto.to_string());
        }
    }
    if let Some(texto) = avisar {
        enviar("event", serde_json::json!({
            "type": "search_miss",
            "kind": if qual == 0 { "live" } else { "vod" },
            "query": texto,
        }));
    }
}

pub fn falhou(kind: &str, titulo: &str, url: &str, fonte: usize, detalhe: &str) {
    evento("source_fail", Some(kind), Some(titulo), Some(url), Some(fonte), Some(detalhe.into()));
}

pub fn caiu(kind: &str, titulo: &str, fontes: usize) {
    evento("channel_down", Some(kind), Some(titulo), None, None,
           Some(format!("nenhuma das {fontes} fonte(s) abriu")));
}

pub fn parou() {
    let fechar_conta = {
        let estado = ESTADO.lock().unwrap();
        match estado.as_ref() {
            Some(e) if e.tocando.is_some() => e.segundos_rodando() >= MINIMO_PARA_CONTAR_S,
            _ => return,
        }
    };
    if fechar_conta {
        bater(true);
    }
    {
        let mut estado = ESTADO.lock().unwrap();
        if let Some(estado) = estado.as_mut() {
            estado.tocando = None;
            estado.acumulado = Duration::ZERO;
            estado.rodando_desde = None;
            estado.pausado = false;
            estado.qualidade = None;
            estado.travou_desde = None;
        }
    }
    evento("play_stop", None, None, None, None, None);
}

/// Falha engolida: conta, mas no máximo algumas por abertura.
pub fn erro(detalhe: String) {
    {
        let mut estado = ESTADO.lock().unwrap();
        let Some(estado) = estado.as_mut() else { return };
        if estado.erros >= 5 {
            return;
        }
        estado.erros += 1;
    }
    evento("error", None, None, None, None, Some(detalhe));
}

/// Queda da abertura anterior: vai mesmo fora da amostra (o monitor aceita `crash` de qualquer aparelho).
pub fn quebra(detalhe: String) {
    evento("crash", None, None, None, None, Some(detalhe));
}

pub fn fechando() {
    let tocando = ESTADO.lock().unwrap().as_ref().map(|e| e.tocando.is_some()).unwrap_or(false);
    bater(false);
    if tocando {
        evento("play_stop", None, None, None, None, None);
    }
}

#[cfg(test)]
mod testes {
    use super::{amostrado, fnv1a};

    // Valores calculados com a conta do servidor (worker.js) no node.
    #[test]
    fn hash_igual_ao_do_servidor() {
        assert_eq!(fnv1a("79a275ad-b783-488c-8e18-2439ff3aed1c"), 3670190157);
        assert_eq!(fnv1a("db09f237-fffa-40c7-94c5-35eddcad6970"), 2926602047);
    }

    #[test]
    fn amostra_de_cinco_por_cento() {
        assert!(amostrado("79a275ad-b783-488c-8e18-2439ff3aed1c", 5.0));
        assert!(amostrado("79A275AD-B783-488C-8E18-2439FF3AED1C", 5.0));
        assert!(!amostrado("db09f237-fffa-40c7-94c5-35eddcad6970", 5.0));
        assert!(amostrado("db09f237-fffa-40c7-94c5-35eddcad6970", 100.0));
        assert!(!amostrado("79a275ad-b783-488c-8e18-2439ff3aed1c", 0.0));
    }
}
