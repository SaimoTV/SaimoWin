//! A lista ao vivo, mais longa e numa régua de tempo contínua — o
//! `Historico.kt` da TV Box, e mais a parte que só o Windows precisa: saber
//! onde o mpv está nessa régua.
//!
//! Dois problemas que só a lista crua tinha:
//!
//! - Muitos canais guardam só os últimos 10 a 15 s na lista, e a TV costuma
//!   tocar mais atrás que isso. O trecho que a TV mostra já saiu da lista
//!   quando o celular pede: ele nunca chegava lá, pulava a cada poucos
//!   segundos e o som sumia. Aqui o app lembra os trechos dos últimos
//!   [`GUARDA_MS`] e devolve todos; os pedaços continuam no servidor do canal
//!   por mais tempo que isso.
//! - Sem marca de horário no canal, cada trecho começa onde o anterior
//!   terminou, pela duração real (EXTINF), a partir de uma âncora no relógio
//!   do computador na primeira vez que a lista é vista.
//!
//! O celular lê a régua das marcas de horário escritas aqui. O mpv não sabe de
//! marca de horário nenhuma: ele toca os pedaços pelo carimbo de tempo (PTS)
//! de dentro deles. Então o app lê o primeiro PTS de cada pedaço que o mpv vai
//! tocar ([`marcar_pts`]) e, com a posição do mpv no mesmo carimbo, acha o
//! trecho e quanto já tocou dele ([`instante_da_tela`]). Os dois aparelhos
//! medem a mesma coisa.

use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex};

const GUARDA_MS: i64 = 120_000;
const ESQUECE_MS: i64 = 10 * 60_000;
/// Origem da hora sintética da régua antiga (lista que não dá para estender).
/// Qualquer valor serve, desde que seja o mesmo no proxy e na conta da TV.
pub const T0: i64 = 1_600_000_000_000;
/// Quantos dos trechos mais novos o app sonda para achar o PTS.
const SONDAR_ULTIMOS: usize = 8;
/// O PTS do MPEG-TS tem 33 bits e dá a volta a cada 26,5 horas.
const VOLTA_DO_PTS: i64 = 1 << 33;

#[derive(Clone, Debug)]
struct Trecho {
    seq: i64,
    duracao_ms: i64,
    /// EXTINF, outras marcas do trecho e o endereço absoluto.
    corpo: Vec<String>,
    chave: Option<String>,
    mapa: Option<String>,
    descontinuidade: bool,
    /// Número de descontinuidade do trecho, como o ExoPlayer conta.
    sequencia_desc: i64,
    marca_propria: Option<String>,
    /// A marca própria lida, em ms desde 1970.
    marca_ms: Option<i64>,
    inicio_ms: i64,
    /// O endereço absoluto do pedaço: é dele que sai o PTS.
    url: String,
    /// O primeiro PTS do pedaço (90 kHz), quando já foi lido.
    pts: Option<i64>,
    /// Já foi pedido para a sonda (que pode ter falhado): não pede de novo.
    sondado: bool,
}

#[derive(Default)]
struct Regua {
    trechos: BTreeMap<i64, Trecho>,
    vista_em: i64,
    cabecalho: Vec<String>,
    alvo_s: i64,
    marca_real: bool,
    /// Os pedaços são MPEG-TS sem criptografia: dá para ler o PTS.
    sondavel: bool,
    referer: Option<String>,
    agente: Option<String>,
    /// Os mpv do app que pediram esta lista: quando e quantas vezes.
    telas: HashMap<u64, (i64, u32)>,
}

static REGUAS: LazyLock<Mutex<HashMap<String, Regua>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Marcas que descrevem a lista inteira, não um trecho.
const MARCAS_DA_LISTA: [&str; 10] = [
    "#EXTM3U", "#EXT-X-VERSION", "#EXT-X-TARGETDURATION", "#EXT-X-MEDIA-SEQUENCE",
    "#EXT-X-DISCONTINUITY-SEQUENCE", "#EXT-X-PLAYLIST-TYPE", "#EXT-X-INDEPENDENT-SEGMENTS",
    "#EXT-X-START", "#EXT-X-ALLOW-CACHE", "#EXT-X-SERVER-CONTROL",
];

/// Um pedaço cujo PTS o app quer ler.
#[derive(Debug, Clone, PartialEq)]
pub struct Sonda {
    pub chave: String,
    pub seq: i64,
    pub url: String,
    pub referer: Option<String>,
    pub agente: Option<String>,
}

/// Quem pediu a lista.
pub struct Pedido<'a> {
    pub url: &'a str,
    pub agora: i64,
    /// O mpv do próprio app que pediu (o celular não tem).
    pub tela: Option<u64>,
    pub referer: Option<&'a str>,
    pub agente: Option<&'a str>,
}

/// A chave da régua: o endereço sem a consulta, que costuma trocar de token.
pub fn chave_de(url: &str) -> String {
    let Some((esquema, resto)) = url.split_once("://") else { return url.to_string() };
    let resto = resto.split(['?', '#']).next().unwrap_or("");
    let (autoridade, caminho) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, ""),
    };
    // Sem usuário e sem porta, como o `URI.host` do Kotlin.
    let host = autoridade.rsplit('@').next().unwrap_or("");
    let host = if host.starts_with('[') {
        host.split(']').next().map(|h| format!("{h}]")).unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or("").to_string()
    };
    format!("{}://{}{}", esquema.to_ascii_lowercase(), host, caminho)
}

fn uri_atributo(linha: &str, absoluto: &dyn Fn(&str) -> String) -> String {
    let Some(i) = linha.find("URI=\"") else { return linha.to_string() };
    let comeco = i + 5;
    let Some(fim) = linha[comeco..].find('"') else { return linha.to_string() };
    format!("{}{}{}", &linha[..comeco], absoluto(&linha[comeco..comeco + fim]), &linha[comeco + fim..])
}

fn depois_dos_dois_pontos(linha: &str) -> &str {
    linha.split_once(':').map(|(_, v)| v.trim()).unwrap_or("")
}

/// A hora de uma marca `#EXT-X-PROGRAM-DATE-TIME`, em ms desde 1970.
pub fn ler_marca(linha: &str) -> Option<i64> {
    let valor = linha.split_once(':').map(|(_, v)| v.trim())?;
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(valor) {
        return Some(d.timestamp_millis());
    }
    // "+0000" sem os dois-pontos, que o RFC 3339 não aceita.
    chrono::DateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S%.f%z").ok().map(|d| d.timestamp_millis())
}

/// A marca de horário escrita na lista, em UTC com milissegundos.
pub fn formatar_marca(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_default()
}

/// A marca de horário sintética da régua antiga, para o trecho `sequencia`.
pub fn marca_sintetica(sequencia: i64, passo_ms: i64) -> String {
    formatar_marca(T0 + sequencia * passo_ms)
}

struct Lido {
    dur: i64,
    corpo: Vec<String>,
    chave: Option<String>,
    mapa: Option<String>,
    desc: bool,
    marca: Option<String>,
    url: String,
}

/// A lista estendida e com a régua escrita, mais os pedaços que o app deve
/// sondar; nada quando ela não é uma lista ao vivo simples (filme, LL-HLS com
/// partes, BYTERANGE) e vale a regra antiga.
pub fn estender(pedido: &Pedido, linhas: &[&str], absoluto: &dyn Fn(&str) -> String) -> Option<(String, Vec<Sonda>)> {
    let tem = |p: &str| linhas.iter().any(|l| l.starts_with(p));
    if tem("#EXT-X-ENDLIST") || tem("#EXT-X-PART") || tem("#EXT-X-PRELOAD-HINT") || tem("#EXT-X-BYTERANGE") {
        return None;
    }
    if tem("#EXT-X-PLAYLIST-TYPE:VOD") {
        return None;
    }

    let mut cabecalho: Vec<String> = Vec::new();
    let mut sequencia = 0i64;
    let mut sequencia_desc = 0i64;
    let mut alvo_s = 6i64;
    let mut lidos: Vec<Lido> = Vec::new();
    let mut chave: Option<String> = None;
    let mut mapa: Option<String> = None;
    let mut desc = false;
    let mut marca: Option<String> = None;
    let mut corpo: Vec<String> = Vec::new();
    let mut dur = 0i64;
    let mut algum_trecho = false;
    let mut cifrado = false;
    for l in linhas {
        let l = l.trim_end();
        if l.trim().is_empty() {
            continue;
        }
        if l.starts_with("#EXT-X-MEDIA-SEQUENCE:") {
            sequencia = depois_dos_dois_pontos(l).parse().unwrap_or(0);
        } else if l.starts_with("#EXT-X-DISCONTINUITY-SEQUENCE:") {
            sequencia_desc = depois_dos_dois_pontos(l).parse().unwrap_or(0);
        } else if l.starts_with("#EXT-X-TARGETDURATION:") {
            alvo_s = depois_dos_dois_pontos(l).parse::<f64>().map(|v| v.ceil() as i64).unwrap_or(6);
        } else if MARCAS_DA_LISTA.iter().any(|m| l.starts_with(m)) {
            if !l.starts_with("#EXTM3U") {
                cabecalho.push(l.to_string());
            }
        } else if l.starts_with("#EXT-X-KEY") {
            cifrado |= !l.contains("METHOD=NONE");
            chave = Some(uri_atributo(l, absoluto));
        } else if l.starts_with("#EXT-X-MAP") {
            mapa = Some(uri_atributo(l, absoluto));
        } else if l.starts_with("#EXT-X-DISCONTINUITY") {
            desc = true;
        } else if l.starts_with("#EXT-X-PROGRAM-DATE-TIME") {
            marca = Some(l.to_string());
        } else if l.starts_with("#EXTINF") {
            let segundos = depois_dos_dois_pontos(l).split(',').next().unwrap_or("").trim().parse::<f64>().unwrap_or(0.0);
            dur = (segundos * 1000.0) as i64;
            corpo.push(l.to_string());
        } else if l.starts_with('#') {
            if algum_trecho || !corpo.is_empty() {
                corpo.push(l.to_string());
            } else {
                cabecalho.push(l.to_string());
            }
        } else {
            let url = absoluto(l.trim());
            corpo.push(url.clone());
            lidos.push(Lido {
                dur: dur.max(1),
                corpo: std::mem::take(&mut corpo),
                chave: chave.clone(),
                mapa: mapa.clone(),
                desc,
                marca: marca.take(),
                url,
            });
            desc = false;
            dur = 0;
            algum_trecho = true;
        }
    }
    if lidos.is_empty() {
        return None;
    }

    let agora = pedido.agora;
    let chave_da_regua = chave_de(pedido.url);
    let mut reguas = REGUAS.lock().unwrap();
    let mut regua = reguas.remove(&chave_da_regua).unwrap_or_default();
    regua.vista_em = agora;
    regua.cabecalho = cabecalho;
    regua.alvo_s = alvo_s;
    regua.marca_real = lidos.iter().any(|l| l.marca.is_some());
    // fMP4 (EXT-X-MAP) e pedaço cifrado: o PTS não está à vista no começo.
    regua.sondavel = !cifrado && lidos.iter().all(|l| l.mapa.is_none());
    if pedido.referer.is_some() || pedido.agente.is_some() {
        regua.referer = pedido.referer.map(str::to_string);
        regua.agente = pedido.agente.map(str::to_string);
    }

    let quantos_lidos = lidos.len();
    let mut d = sequencia_desc;
    let total: i64 = lidos.iter().map(|l| l.dur).sum();
    let mut restante = total;
    for (i, lido) in lidos.into_iter().enumerate() {
        let seq = sequencia + i as i64;
        if lido.desc {
            d += 1;
        }
        let dur = lido.dur;
        if !regua.trechos.contains_key(&seq) {
            // Sem o anterior (primeira vez, ou um buraco depois de muito tempo
            // parado): a âncora de outra lista que já tem este trecho (outro
            // celular, outra qualidade do mesmo canal), para todas contarem
            // igual; senão o relógio, com o fim da lista agora.
            let inicio = regua
                .trechos
                .get(&(seq - 1))
                .map(|a| a.inicio_ms + a.duracao_ms)
                .or_else(|| {
                    reguas.values().find_map(|r| {
                        if agora - r.vista_em < 60_000 { r.trechos.get(&seq).map(|t| t.inicio_ms) } else { None }
                    })
                })
                .unwrap_or(agora - restante);
            let marca_ms = lido.marca.as_deref().and_then(ler_marca);
            regua.trechos.insert(
                seq,
                Trecho {
                    seq,
                    duracao_ms: dur,
                    corpo: lido.corpo,
                    chave: lido.chave,
                    mapa: lido.mapa,
                    descontinuidade: lido.desc,
                    sequencia_desc: d,
                    marca_propria: lido.marca,
                    marca_ms,
                    inicio_ms: inicio,
                    url: lido.url,
                    pts: None,
                    sondado: false,
                },
            );
        }
        restante -= dur;
    }
    // Um buraco na numeração (a lista ficou mais tempo sem ser pedida do que
    // a origem guarda): o que vem antes dele sai. Escrito numa lista só, o
    // player contaria os trechos depois do buraco com o número errado.
    let continuo_desde = regua
        .trechos
        .keys()
        .rev()
        .zip(regua.trechos.keys().rev().skip(1))
        .find(|(depois, antes)| **depois != **antes + 1)
        .map(|(depois, _)| *depois);
    if let Some(primeiro) = continuo_desde {
        regua.trechos = regua.trechos.split_off(&primeiro);
    }
    let corte = {
        let ultimo = regua.trechos.values().next_back().unwrap();
        ultimo.inicio_ms + ultimo.duracao_ms - GUARDA_MS
    };
    while regua.trechos.len() > quantos_lidos && regua.trechos.values().next().is_some_and(|t| t.inicio_ms < corte) {
        regua.trechos.pop_first();
    }

    let mut sondas = Vec::new();
    if let Some(tela) = pedido.tela {
        let entrada = regua.telas.entry(tela).or_insert((agora, 0));
        entrada.0 = agora;
        entrada.1 = entrada.1.saturating_add(1);
        // Só a lista que o mpv volta a pedir é a que ele toca: na abertura ele
        // lê todas as qualidades do canal uma vez, e sondar todas seria
        // martelar a origem à toa.
        if regua.sondavel && entrada.1 >= 2 {
            let (referer, agente) = (regua.referer.clone(), regua.agente.clone());
            for t in regua.trechos.values_mut().rev().take(SONDAR_ULTIMOS) {
                if t.pts.is_none() && !t.sondado {
                    t.sondado = true;
                    sondas.push(Sonda {
                        chave: chave_da_regua.clone(),
                        seq: t.seq,
                        url: t.url.clone(),
                        referer: referer.clone(),
                        agente: agente.clone(),
                    });
                }
            }
        }
    }
    let texto = escrever_lista(&regua);
    reguas.insert(chave_da_regua, regua);
    reguas.retain(|_, r| agora - r.vista_em <= ESQUECE_MS);
    Some((texto, sondas))
}

fn escrever_lista(regua: &Regua) -> String {
    let trechos: Vec<&Trecho> = regua.trechos.values().collect();
    let primeiro = trechos[0];
    let maior = trechos.iter().map(|t| t.duracao_ms).max().unwrap_or(0);
    let alvo = regua.alvo_s.max((maior as f64 / 1000.0).ceil() as i64);
    let mut sb = String::from("#EXTM3U\n");
    for l in &regua.cabecalho {
        if !l.starts_with("#EXT-X-TARGETDURATION") {
            sb.push_str(l);
            sb.push('\n');
        }
    }
    sb.push_str(&format!("#EXT-X-TARGETDURATION:{alvo}\n"));
    sb.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", primeiro.seq));
    sb.push_str(&format!(
        "#EXT-X-DISCONTINUITY-SEQUENCE:{}\n",
        primeiro.sequencia_desc - if primeiro.descontinuidade { 1 } else { 0 }
    ));
    let mut chave: Option<&String> = None;
    let mut mapa: Option<&String> = None;
    for t in trechos {
        if t.descontinuidade {
            sb.push_str("#EXT-X-DISCONTINUITY\n");
        }
        if let Some(c) = t.chave.as_ref().filter(|c| Some(*c) != chave) {
            sb.push_str(c);
            sb.push('\n');
            chave = Some(c);
        }
        if let Some(m) = t.mapa.as_ref().filter(|m| Some(*m) != mapa) {
            sb.push_str(m);
            sb.push('\n');
            mapa = Some(m);
        }
        if regua.marca_real {
            if let Some(m) = t.marca_propria.as_ref() {
                sb.push_str(m);
                sb.push('\n');
            }
        } else {
            sb.push_str(&format!("#EXT-X-PROGRAM-DATE-TIME:{}\n", formatar_marca(t.inicio_ms)));
        }
        for l in &t.corpo {
            sb.push_str(l);
            sb.push('\n');
        }
    }
    sb
}

/// O instante, na régua da lista que o celular recebe, do trecho `seq` mais
/// `dentro_ms`. Nada quando nenhuma lista com esse trecho foi pedida há pouco
/// (canal com marca de horário própria também não passa por aqui: a hora
/// real já é a régua).
///
/// É a conversão da TV Box (`Historico.instante`), que conhece o número do
/// trecho no ar pelo ExoPlayer. O mpv não diz o número do trecho, então aqui a
/// posição chega à régua por [`instante_da_tela`]; esta fica para quem souber
/// o trecho (e para os testes da régua).
#[allow(dead_code)]
pub fn instante(seq: i64, dentro_ms: i64, agora: i64) -> Option<i64> {
    let reguas = REGUAS.lock().unwrap();
    let regua = reguas
        .values()
        .filter(|r| {
            !r.marca_real
                && agora - r.vista_em < 60_000
                && !r.trechos.is_empty()
                && (r.trechos.keys().next().unwrap() - 30..=r.trechos.keys().next_back().unwrap() + 30).contains(&seq)
        })
        .max_by_key(|r| r.vista_em)?;
    if let Some(t) = regua.trechos.get(&seq) {
        return Some(t.inicio_ms + dentro_ms);
    }
    let media = (regua.trechos.values().map(|t| t.duracao_ms).sum::<i64>() / regua.trechos.len() as i64).max(1);
    let primeiro = regua.trechos.values().next().unwrap();
    let ultimo = regua.trechos.values().next_back().unwrap();
    Some(if seq < primeiro.seq {
        primeiro.inicio_ms - (primeiro.seq - seq) * media + dentro_ms
    } else {
        ultimo.inicio_ms + ultimo.duracao_ms + (seq - ultimo.seq - 1) * media + dentro_ms
    })
}

/// Guarda o primeiro PTS lido de um pedaço.
pub fn marcar_pts(chave: &str, seq: i64, pts: i64) {
    let mut reguas = REGUAS.lock().unwrap();
    if let Some(t) = reguas.get_mut(chave).and_then(|r| r.trechos.get_mut(&seq)) {
        t.pts = Some(pts);
    }
}

/// A diferença `a - b` de dois PTS, contando a volta dos 33 bits: o FFmpeg
/// às vezes já desdobra a volta, e o PTS lido do pedaço nunca.
pub fn diferenca_de_pts(a: i64, b: i64) -> i64 {
    let r = (a - b).rem_euclid(VOLTA_DO_PTS);
    if r >= VOLTA_DO_PTS / 2 { r - VOLTA_DO_PTS } else { r }
}

/// Onde uma tela do app está na régua comum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NaRegua {
    /// O instante do conteúdo no ar (o `st` do aviso de posição).
    pub instante: i64,
    /// O fim do último trecho conhecido na mesma régua: a ponta ao vivo.
    pub ponta: i64,
}

/// O começo do trecho `seq` na régua que o celular lê: a marca própria (ou a
/// do trecho anterior mais as durações, quando só alguns trazem marca) ou a
/// âncora sintética.
fn comeco(regua: &Regua, seq: i64) -> Option<i64> {
    if !regua.marca_real {
        return regua.trechos.get(&seq).map(|t| t.inicio_ms);
    }
    let mut soma = 0;
    for t in regua.trechos.range(..=seq).rev().map(|(_, t)| t) {
        if t.seq != seq {
            soma += t.duracao_ms;
        }
        if let Some(m) = t.marca_ms {
            return Some(m + soma);
        }
    }
    None
}

/// Anda `d_ms` a partir do começo do trecho `ancora`: o trecho onde cai, e
/// quanto dentro dele. Nada se no caminho há buraco ou descontinuidade (aí o
/// PTS recomeça e a conta não vale).
fn andar(regua: &Regua, ancora: i64, d_ms: i64) -> Option<(i64, i64)> {
    let mut acumulado = 0;
    let mut ultimo: Option<&Trecho> = None;
    for (esperado, t) in (ancora..).zip(regua.trechos.range(ancora..).map(|(_, t)| t)) {
        if t.seq != esperado || (t.seq != ancora && t.descontinuidade) {
            return None;
        }
        if d_ms < acumulado + t.duracao_ms {
            return Some((t.seq, d_ms - acumulado));
        }
        acumulado += t.duracao_ms;
        ultimo = Some(t);
    }
    // Um pouco além do último trecho conhecido: a lista nova ainda não chegou.
    let ultimo = ultimo?;
    let dentro = d_ms - (acumulado - ultimo.duracao_ms);
    (dentro < ultimo.duracao_ms + 5_000).then_some((ultimo.seq, dentro))
}

/// Onde o mpv `tela` está na régua comum, com a posição dele no carimbo de
/// tempo do arquivo (`time-pos` sem a mudança de origem do mpv).
///
/// O trecho de referência é o mais perto antes da posição entre os que têm o
/// PTS lido; dali a conta anda pelas durações reais até o trecho onde a
/// posição cai, e a régua é o começo desse trecho mais o que já tocou dele.
pub fn instante_da_tela(tela: u64, posicao_s: f64, agora: i64) -> Option<NaRegua> {
    if !posicao_s.is_finite() {
        return None;
    }
    let posicao = (posicao_s * 90_000.0).round() as i64;
    let reguas = REGUAS.lock().unwrap();
    let mut melhor: Option<(i64, NaRegua)> = None;
    for regua in reguas.values() {
        let Some((vista, _)) = regua.telas.get(&tela) else { continue };
        if agora - vista > 60_000 || regua.trechos.is_empty() {
            continue;
        }
        for t in regua.trechos.values() {
            let Some(pts) = t.pts else { continue };
            let d = diferenca_de_pts(posicao, pts);
            if d < 0 {
                continue;
            }
            let d_ms = d / 90;
            if d_ms > 600_000 || melhor.as_ref().is_some_and(|(m, _)| *m <= d_ms) {
                continue;
            }
            let Some((seq, dentro)) = andar(regua, t.seq, d_ms) else { continue };
            let Some(base) = comeco(regua, seq) else { continue };
            let ultimo = regua.trechos.values().next_back().unwrap();
            let ponta = comeco(regua, ultimo.seq).map(|c| c + ultimo.duracao_ms).unwrap_or(base + dentro);
            melhor = Some((d_ms, NaRegua { instante: base + dentro, ponta }));
        }
    }
    melhor.map(|(_, r)| r)
}

/// O que a régua sabe de cada trecho: chave, número, PTS lido e endereço
/// (só nos testes, para conferir contra o ffprobe).
#[cfg(test)]
pub fn depurar() -> Vec<(String, i64, Option<i64>, i64, String)> {
    let reguas = REGUAS.lock().unwrap();
    let mut lista: Vec<_> = reguas
        .iter()
        .flat_map(|(k, r)| r.trechos.values().map(move |t| (k.clone(), t.seq, t.pts, t.inicio_ms, t.url.clone())))
        .collect();
    lista.sort();
    lista
}

/// Esquece tudo (só nos testes).
#[cfg(test)]
pub fn limpar() {
    REGUAS.lock().unwrap().clear();
}

#[cfg(test)]
pub mod testes {
    use super::*;

    /// Os testes daqui mexem na mesma memória global: um de cada vez.
    pub static VEZ: Mutex<()> = Mutex::new(());

    fn lista(seq: i64, duracoes: &[f64], extra: &str) -> String {
        let mut s = format!("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:{seq}\n{extra}");
        for (i, d) in duracoes.iter().enumerate() {
            s.push_str(&format!("#EXTINF:{d:.3},\nseg{}.ts?tok=abc\n", seq + i as i64));
        }
        s
    }

    fn absoluto(u: &str) -> String {
        if u.starts_with("http") { u.to_string() } else { format!("https://cdn.exemplo.com/canal/{u}") }
    }

    fn pedir(url: &str, texto: &str, agora: i64, tela: Option<u64>) -> (String, Vec<Sonda>) {
        let linhas: Vec<&str> = texto.lines().collect();
        estender(&Pedido { url, agora, tela, referer: Some("https://ref/"), agente: None }, &linhas, &absoluto).unwrap()
    }

    fn marcas(texto: &str) -> Vec<i64> {
        texto.lines().filter(|l| l.starts_with("#EXT-X-PROGRAM-DATE-TIME")).filter_map(ler_marca).collect()
    }

    #[test]
    fn chave_sem_consulta_nem_porta() {
        assert_eq!(chave_de("https://a.com:8443/x/y.m3u8?token=1#f"), "https://a.com/x/y.m3u8");
        assert_eq!(chave_de("http://u:p@B.com/z.m3u8"), "http://B.com/z.m3u8");
    }

    #[test]
    fn regua_continua_pela_duracao_real_e_lembra_dois_minutos() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let url = "https://cdn.exemplo.com/canal/a.m3u8?tok=1";
        let agora = 1_700_000_000_000;
        let (texto, _) = pedir(url, &lista(100, &[2.936, 2.936, 2.936], ""), agora, None);
        let m = marcas(&texto);
        // Âncora: o fim da lista é agora; cada trecho começa onde o outro acaba.
        assert_eq!(m, vec![agora - 8808, agora - 5872, agora - 2936]);
        assert!(texto.contains("https://cdn.exemplo.com/canal/seg100.ts?tok=abc"));
        // Lista seguinte, já sem o 100: a régua devolve o 100 também, e o 103
        // continua do 102 (não do relógio).
        let (texto, _) = pedir(url, &lista(101, &[2.936, 2.936, 2.936], ""), agora + 3_500, None);
        let m = marcas(&texto);
        assert_eq!(m.len(), 4);
        assert_eq!(m[3], agora);
        assert!(texto.contains("#EXT-X-MEDIA-SEQUENCE:100"));
        // Muito tempo depois: só os últimos dois minutos ficam.
        let mut t = agora;
        for k in 0..60 {
            t += 2_936;
            pedir(url, &lista(104 + k, &[2.936, 2.936, 2.936], ""), t, None);
        }
        let (texto, _) = pedir(url, &lista(164, &[2.936, 2.936, 2.936], ""), t + 2_936, None);
        let n = marcas(&texto).len();
        assert!((40..=43).contains(&n), "{n} trechos");
    }

    #[test]
    fn outra_qualidade_herda_a_ancora() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let agora = 1_700_000_000_000;
        let (a, _) = pedir("https://cdn.exemplo.com/canal/720.m3u8", &lista(50, &[6.0, 6.0], ""), agora, None);
        let (b, _) = pedir("https://cdn.exemplo.com/canal/360.m3u8", &lista(50, &[6.0, 6.0], ""), agora + 1_234, None);
        assert_eq!(marcas(&a), marcas(&b));
    }

    #[test]
    fn lista_com_marca_propria_fica_com_ela() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let extra = "";
        let mut texto = lista(7, &[4.0], extra);
        texto = texto.replace("#EXTINF", "#EXT-X-PROGRAM-DATE-TIME:2026-10-18T12:00:00.000Z\n#EXTINF");
        let (saida, _) = pedir("https://cdn.exemplo.com/canal/pdt.m3u8", &texto, 1_700_000_000_000, None);
        assert_eq!(marcas(&saida), vec![ler_marca("#EXT-X-PROGRAM-DATE-TIME:2026-10-18T12:00:00.000Z").unwrap()]);
    }

    #[test]
    fn filme_e_ll_hls_nao_estendem() {
        let linhas_vod = ["#EXTM3U", "#EXTINF:4,", "a.ts", "#EXT-X-ENDLIST"];
        let p = Pedido { url: "https://x/v.m3u8", agora: 0, tela: None, referer: None, agente: None };
        assert!(estender(&p, &linhas_vod, &absoluto).is_none());
        let linhas_ll = ["#EXTM3U", "#EXT-X-PART:DURATION=1,URI=\"p.ts\"", "#EXTINF:4,", "a.ts"];
        assert!(estender(&p, &linhas_ll, &absoluto).is_none());
    }

    #[test]
    fn descontinuidade_e_chave_sao_mantidas() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let texto = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:10\n#EXT-X-DISCONTINUITY-SEQUENCE:3\n\
            #EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXTINF:6,\na.ts\n#EXT-X-DISCONTINUITY\n#EXTINF:6,\nb.ts\n";
        let (saida, _) = pedir("https://cdn.exemplo.com/canal/d.m3u8", texto, 1_700_000_000_000, Some(1));
        assert!(saida.contains("#EXT-X-DISCONTINUITY-SEQUENCE:3\n"));
        assert!(saida.contains("#EXT-X-KEY:METHOD=AES-128,URI=\"https://cdn.exemplo.com/canal/k.bin\""));
        assert_eq!(saida.matches("#EXT-X-DISCONTINUITY\n").count(), 1);
        // Pedaço cifrado: o PTS não dá para ler, nada a sondar.
        let (_, sondas) = pedir("https://cdn.exemplo.com/canal/d.m3u8", texto, 1_700_000_001_000, Some(1));
        assert!(sondas.is_empty());
    }

    #[test]
    fn buraco_na_numeracao_nao_vai_para_a_lista() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let url = "https://cdn.exemplo.com/canal/buraco.m3u8";
        let agora = 1_700_000_000_000;
        pedir(url, &lista(10, &[4.0, 4.0], ""), agora, None);
        // Um minuto sem ninguém pedir: a origem já está no 25.
        let (texto, _) = pedir(url, &lista(25, &[4.0, 4.0], ""), agora + 60_000, None);
        assert!(texto.contains("#EXT-X-MEDIA-SEQUENCE:25\n"));
        assert_eq!(marcas(&texto).len(), 2);
        assert!(!texto.contains("seg10.ts"));
    }

    #[test]
    fn pts_da_volta() {
        assert_eq!(diferenca_de_pts(100, 50), 50);
        assert_eq!(diferenca_de_pts(10, VOLTA_DO_PTS - 80), 90);
        assert_eq!(diferenca_de_pts(VOLTA_DO_PTS + 10, 10), 0);
        assert_eq!(diferenca_de_pts(50, 100), -50);
    }

    #[test]
    fn mpv_na_regua_pelo_pts() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let url = "https://cdn.exemplo.com/canal/pts.m3u8";
        let agora = 1_700_000_000_000;
        let tela = 42;
        // EXTINF arredondado (6 no lugar de 5,97): a régua anda por ele, e a
        // conta da TV também, então os dois continuam juntos.
        let (_, sondas) = pedir(url, &lista(200, &[6.0, 6.0, 6.0], ""), agora, Some(tela));
        assert!(sondas.is_empty(), "a primeira lista não é sondada");
        let (texto, sondas) = pedir(url, &lista(200, &[6.0, 6.0, 6.0], ""), agora + 2_000, Some(tela));
        assert_eq!(sondas.iter().map(|s| s.seq).collect::<Vec<_>>(), vec![202, 201, 200]);
        assert_eq!(sondas[0].referer.as_deref(), Some("https://ref/"));
        let regua = marcas(&texto);
        // Ninguém lido ainda: não há como saber.
        assert_eq!(instante_da_tela(tela, 1000.0, agora), None);
        // O 201 começa no PTS 1000 s (com a volta dos 33 bits a caminho).
        let base = VOLTA_DO_PTS - 90_000 * 3;
        marcar_pts(&chave_de(url), 201, base);
        // mpv 2,5 s dentro do 201, já com a volta desdobrada pelo FFmpeg.
        let r = instante_da_tela(tela, (base + 225_000) as f64 / 90_000.0, agora).unwrap();
        assert_eq!(r.instante, regua[1] + 2_500);
        assert_eq!(r.ponta, regua[2] + 6_000);
        // 8 s depois do começo do 201 cai 2 s dentro do 202.
        let r = instante_da_tela(tela, (base + 8 * 90_000) as f64 / 90_000.0, agora).unwrap();
        assert_eq!(r.instante, regua[2] + 2_000);
        // Antes do 201 e sem o 200 lido: nada.
        assert_eq!(instante_da_tela(tela, (base - 90_000) as f64 / 90_000.0, agora), None);
        // Outra tela (outro mpv) não usa esta régua.
        assert_eq!(instante_da_tela(7, (base + 225_000) as f64 / 90_000.0, agora), None);
    }

    #[test]
    fn marca_real_mais_o_que_tocou() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let url = "https://cdn.exemplo.com/canal/real.m3u8";
        let agora = 1_700_000_000_000;
        let texto = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:5\n\
            #EXT-X-PROGRAM-DATE-TIME:2026-10-18T12:00:00.000Z\n#EXTINF:4,\na.ts\n#EXTINF:4,\nb.ts\n";
        pedir(url, texto, agora, Some(3));
        marcar_pts(&chave_de(url), 5, 900_000);
        let r = instante_da_tela(3, (900_000 + 5 * 90_000) as f64 / 90_000.0, agora).unwrap();
        let doze = ler_marca("#X:2026-10-18T12:00:00.000Z").unwrap();
        // O 6 não traz marca: é a do 5 mais os 4 s dele, e 1 s dentro.
        assert_eq!(r.instante, doze + 5_000);
        assert_eq!(r.ponta, doze + 8_000);
    }

    #[test]
    fn descontinuidade_no_caminho_nao_conta() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let url = "https://cdn.exemplo.com/canal/desc.m3u8";
        let texto = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:1\n#EXTINF:4,\na.ts\n\
            #EXT-X-DISCONTINUITY\n#EXTINF:4,\nb.ts\n";
        let agora = 1_700_000_000_000;
        pedir(url, texto, agora, Some(9));
        marcar_pts(&chave_de(url), 1, 0);
        // 5 s depois do PTS do 1 já seria o 2, que vem depois de uma quebra.
        assert_eq!(instante_da_tela(9, 5.0, agora), None);
        assert!(instante_da_tela(9, 1.0, agora).is_some());
        // Lido o 2, ele vale (o PTS dele recomeça em outro lugar).
        marcar_pts(&chave_de(url), 2, 9_000_000);
        let r = instante_da_tela(9, 100.5, agora).unwrap();
        let (texto, _) = pedir(url, texto, agora, Some(9));
        assert_eq!(r.instante, marcas(&texto)[1] + 500);
    }

    #[test]
    fn instante_antigo_por_sequencia() {
        let _vez = VEZ.lock().unwrap();
        limpar();
        let agora = 1_700_000_000_000;
        let (texto, _) = pedir("https://cdn.exemplo.com/canal/s.m3u8", &lista(300, &[5.0, 5.0], ""), agora, None);
        let m = marcas(&texto);
        assert_eq!(instante(301, 1_000, agora), Some(m[1] + 1_000));
        assert_eq!(instante(302, 0, agora), Some(m[1] + 5_000));
        assert_eq!(instante(299, 0, agora), Some(m[0] - 5_000));
        assert_eq!(instante(9_999, 0, agora), None);
    }
}
