//! A lista do canal já na régua do app — o `ConnectHls.kt` da TV Box.
//!
//! `GET /hls?u=<lista>&r=<referer>&ua=<user-agent>`: o app baixa a lista com
//! os cabeçalhos certos e devolve com os endereços absolutos (os pedaços vêm
//! direto da origem) e com a régua escrita ([`super::historico`]). Lista
//! mestra tem as variantes reescritas para passar por aqui também.
//!
//! Quem mais usa esta rota é o próprio mpv do app (`&tv=<id do mpv>`, só de
//! 127.0.0.1): é assim que a régua da TV e a do celular ficam a mesma. Para
//! ele, qualquer falha vira um redirecionamento para a lista original, e o
//! canal toca como tocava antes, só sem a régua.

use super::historico::{self, Pedido, Sonda};
use std::io::Read;
use std::sync::mpsc::{channel, Sender};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

/// A resposta, para o servidor escrever.
pub struct Resposta {
    pub codigo: u16,
    pub tipo: &'static str,
    pub corpo: Vec<u8>,
    /// Para onde mandar (302), quando o mpv deve ir direto à origem.
    pub destino: Option<String>,
}

impl Resposta {
    fn texto(codigo: u16, corpo: &str) -> Self {
        Resposta { codigo, tipo: "text/plain", corpo: corpo.as_bytes().to_vec(), destino: None }
    }

    fn para(url: &str) -> Self {
        Resposta { codigo: 302, tipo: "text/plain", corpo: Vec::new(), destino: Some(url.to_string()) }
    }
}

// MARK: - Endereços

/// Codifica para ir numa consulta (tudo que não é letra, número ou `-_.~`).
pub fn codificar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len() * 3 / 2);
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            saida.push(b as char);
        } else {
            saida.push_str(&format!("%{b:02X}"));
        }
    }
    saida
}

/// O contrário, aceitando também o `+` como espaço (o `URLEncoder` do Java).
pub fn decodificar(texto: &str) -> String {
    let bytes = texto.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => saida.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        saida.push(v);
                        i += 2;
                    }
                    Err(_) => saida.push(b'%'),
                }
            }
            outro => saida.push(outro),
        }
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}

/// Os parâmetros da consulta de `/hls?...`.
pub fn parametros(alvo: &str) -> Vec<(String, String)> {
    let consulta = alvo.split_once('?').map(|(_, c)| c).unwrap_or("");
    consulta
        .split('&')
        .filter_map(|par| par.split_once('='))
        .map(|(k, v)| (decodificar(k), decodificar(v)))
        .collect()
}

/// Resolve `relativo` contra `base` (RFC 3986), o `URI.resolve` do Java.
pub fn resolver(base: &str, relativo: &str) -> String {
    let relativo = relativo.trim();
    if relativo.contains("://") && relativo.split("://").next().is_some_and(|e| e.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))) {
        return relativo.to_string();
    }
    let Some((esquema, resto)) = base.split_once("://") else { return relativo.to_string() };
    if let Some(r) = relativo.strip_prefix("//") {
        return format!("{esquema}://{r}");
    }
    let (autoridade, caminho_base) = match resto.find(['/', '?', '#']) {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, ""),
    };
    let caminho_base = caminho_base.split(['?', '#']).next().unwrap_or("");
    if relativo.is_empty() {
        return base.to_string();
    }
    if relativo.starts_with('?') {
        return format!("{esquema}://{autoridade}{caminho_base}{relativo}");
    }
    let (caminho_rel, sufixo) = match relativo.find(['?', '#']) {
        Some(i) => (&relativo[..i], &relativo[i..]),
        None => (relativo, ""),
    };
    let juntado = if caminho_rel.starts_with('/') {
        caminho_rel.to_string()
    } else {
        let pasta = match caminho_base.rfind('/') {
            Some(i) => &caminho_base[..=i],
            None => "/",
        };
        format!("{pasta}{caminho_rel}")
    };
    format!("{esquema}://{autoridade}{}{sufixo}", sem_pontos(&juntado))
}

/// Tira os `.` e `..` do caminho.
fn sem_pontos(caminho: &str) -> String {
    let mut partes: Vec<&str> = Vec::new();
    let termina_em_pasta = caminho.ends_with("/.") || caminho.ends_with("/..") || caminho.ends_with('/');
    for p in caminho.split('/') {
        match p {
            "." | "" => {}
            ".." => {
                partes.pop();
            }
            outro => partes.push(outro),
        }
    }
    let mut saida = format!("/{}", partes.join("/"));
    if termina_em_pasta && !saida.ends_with('/') {
        saida.push('/');
    }
    saida
}

/// Se o endereço é de uma lista HLS que vale passar pela régua.
pub fn parece_hls(url: &str) -> bool {
    let minusculo = url.to_ascii_lowercase();
    if !(minusculo.starts_with("http://") || minusculo.starts_with("https://")) {
        return false;
    }
    let caminho = minusculo.split(['?', '#']).next().unwrap_or("");
    minusculo.contains(".m3u8") || caminho.ends_with(".txt")
}

/// O endereço da régua para uma lista: o que o mpv do app abre, e o que o
/// celular monta do lado dele.
pub fn endereco(host: &str, url: &str, referer: Option<&str>, agente: Option<&str>, tela: Option<u64>) -> String {
    let mut s = format!("http://{host}/hls?u={}", codificar(url));
    if let Some(r) = referer.filter(|r| !r.is_empty()) {
        s.push_str(&format!("&r={}", codificar(r)));
    }
    if let Some(a) = agente.filter(|a| !a.is_empty()) {
        s.push_str(&format!("&ua={}", codificar(a)));
    }
    if let Some(t) = tela {
        s.push_str(&format!("&tv={t}"));
    }
    s
}

// MARK: - A rota

fn agente_http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(8)))
        .timeout_global(Some(Duration::from_secs(15)))
        // O mpv toca com tls-verify=no; aqui é igual, senão o canal que toca
        // hoje deixaria de ter régua.
        .tls_config(ureq::tls::TlsConfig::builder().disable_verification(true).build())
        .build()
        .into()
}

static AGENTE: LazyLock<ureq::Agent> = LazyLock::new(agente_http);

/// Baixa a lista: (endereço final depois dos redirecionamentos, texto).
fn baixar(url: &str, referer: Option<&str>, agente: Option<&str>) -> Option<(String, String)> {
    use ureq::ResponseExt;
    let mut pedido = AGENTE.get(url).header("User-Agent", agente.unwrap_or(crate::AGENTE)).header("Accept", "*/*");
    if let Some(r) = referer.filter(|r| !r.is_empty()) {
        pedido = pedido.header("Referer", r);
    }
    let resposta = pedido.call().ok()?;
    let final_ = resposta.get_uri().to_string();
    let texto = resposta.into_body().with_config().limit(8 * 1024 * 1024).read_to_string().ok()?;
    Some((final_, texto))
}

/// Responde `GET /hls?...`. `host` é o cabeçalho Host do pedido; `local` diz
/// se veio do próprio computador (só aí o `tv=` vale).
pub fn responder(alvo: &str, host: &str, local: bool, agora: i64) -> Resposta {
    let params = parametros(alvo);
    let valor = |k: &str| params.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let Some(url) = valor("u").filter(|u| u.starts_with("http://") || u.starts_with("https://")) else {
        return Resposta::texto(400, "lista?");
    };
    let referer = valor("r");
    let agente = valor("ua");
    let tela = if local { valor("tv").and_then(|t| t.parse::<u64>().ok()) } else { None };

    let Some((final_, corpo)) = baixar(&url, referer.as_deref(), agente.as_deref()) else {
        return if tela.is_some() { Resposta::para(&url) } else { Resposta::texto(502, "origem") };
    };
    if !corpo.trim_start().starts_with("#EXTM3U") {
        return if tela.is_some() { Resposta::para(&url) } else { Resposta::texto(502, "origem") };
    }
    let (texto, sondas) = reescrever(&url, &final_, &corpo, host, referer.as_deref(), agente.as_deref(), tela, agora);
    for s in sondas {
        sondar(s);
    }
    Resposta { codigo: 200, tipo: "application/vnd.apple.mpegurl", corpo: texto.into_bytes(), destino: None }
}

/// A lista reescrita: mestra com as variantes pela régua, a de pedaços
/// estendida (ou, quando não dá, com os endereços absolutos e a régua antiga).
#[allow(clippy::too_many_arguments)]
pub fn reescrever(
    url: &str,
    final_: &str,
    corpo: &str,
    host: &str,
    referer: Option<&str>,
    agente: Option<&str>,
    tela: Option<u64>,
    agora: i64,
) -> (String, Vec<Sonda>) {
    let absoluto = |u: &str| resolver(final_, u);
    let pelo_proxy = |u: &str| endereco(host, &absoluto(u), referer, agente, tela);
    let troca_uri = |linha: &str, f: &dyn Fn(&str) -> String| -> String {
        let Some(i) = linha.find("URI=\"") else { return linha.to_string() };
        let comeco = i + 5;
        let Some(fim) = linha[comeco..].find('"') else { return linha.to_string() };
        format!("{}{}{}", &linha[..comeco], f(&linha[comeco..comeco + fim]), &linha[comeco + fim..])
    };
    let linhas: Vec<&str> = corpo.lines().map(|l| l.trim_end()).collect();
    let mestra = linhas.iter().any(|l| l.starts_with("#EXT-X-STREAM-INF"));
    let mut saida = String::with_capacity(corpo.len() * 2);
    let mut linha = |l: &str| {
        saida.push_str(l);
        saida.push('\n');
    };

    if mestra {
        let mut proxima_e_variante = false;
        for l in &linhas {
            if l.starts_with("#EXT-X-STREAM-INF") {
                linha(l);
                proxima_e_variante = true;
            } else if l.starts_with("#EXT-X-MEDIA") || l.starts_with("#EXT-X-I-FRAME-STREAM-INF") {
                linha(&troca_uri(l, &pelo_proxy));
            } else if !l.trim().is_empty() && !l.starts_with('#') && proxima_e_variante {
                linha(&pelo_proxy(l));
                proxima_e_variante = false;
            } else {
                linha(l);
            }
        }
        return (saida, Vec::new());
    }

    let pedido = Pedido { url, agora, tela, referer, agente };
    if let Some((estendida, sondas)) = historico::estender(&pedido, &linhas, &absoluto) {
        return (estendida, sondas);
    }
    let tem_marca = linhas.iter().any(|l| l.starts_with("#EXT-X-PROGRAM-DATE-TIME"));
    let fim = linhas.iter().any(|l| l.starts_with("#EXT-X-ENDLIST"));
    let sequencia = linhas
        .iter()
        .find(|l| l.starts_with("#EXT-X-MEDIA-SEQUENCE:"))
        .and_then(|l| l.split_once(':')?.1.trim().parse::<i64>().ok())
        .unwrap_or(0);
    let passo = linhas
        .iter()
        .find(|l| l.starts_with("#EXT-X-TARGETDURATION:"))
        .and_then(|l| l.split_once(':')?.1.trim().parse::<f64>().ok())
        .map(|v| ((v * 1000.0) as i64).max(1))
        .unwrap_or(6000);
    let escrever_marca = !tem_marca && !fim;
    let mut indice = 0i64;
    for l in &linhas {
        if l.starts_with("#EXT-X-KEY") || l.starts_with("#EXT-X-MAP") {
            linha(&troca_uri(l, &absoluto));
        } else if l.starts_with("#EXTINF") {
            // A marca vai antes do EXTINF de cada trecho.
            if escrever_marca {
                linha(&format!("#EXT-X-PROGRAM-DATE-TIME:{}", historico::marca_sintetica(sequencia + indice, passo)));
            }
            linha(l);
        } else if !l.trim().is_empty() && !l.starts_with('#') {
            linha(&absoluto(l));
            indice += 1;
        } else {
            linha(l);
        }
    }
    (saida, Vec::new())
}

// MARK: - A sonda do PTS

/// O primeiro carimbo de apresentação (PTS, 90 kHz) de um pedaço MPEG-TS: o
/// do vídeo, que é o que o mpv mostra; sem vídeo nos primeiros bytes, o
/// primeiro que aparecer.
pub fn primeiro_pts(dados: &[u8]) -> Option<i64> {
    const PACOTE: usize = 188;
    // Há origem que disfarça o pedaço com o começo de uma imagem (PNG, GIF)
    // na frente do TS: a sincronia é procurada além do primeiro pacote.
    let inicio = (0..dados.len().min(64 * 1024)).find(|&i| {
        dados.get(i) == Some(&0x47)
            && dados.get(i + PACOTE).is_none_or(|b| *b == 0x47)
            && dados.get(i + 2 * PACOTE).is_none_or(|b| *b == 0x47)
    })?;
    let mut outro: Option<i64> = None;
    let mut i = inicio;
    while i + PACOTE <= dados.len() {
        let p = &dados[i..i + PACOTE];
        i += PACOTE;
        if p[0] != 0x47 {
            continue;
        }
        let comeco_de_pes = p[1] & 0x40 != 0;
        let campo = (p[3] >> 4) & 0x3;
        if !comeco_de_pes || campo & 0x1 == 0 {
            continue;
        }
        let mut carga = 4;
        if campo & 0x2 != 0 {
            carga += 1 + p[4] as usize;
        }
        let Some(pes) = p.get(carga..) else { continue };
        if pes.len() < 14 || pes[0] != 0 || pes[1] != 0 || pes[2] != 1 {
            continue;
        }
        let fluxo = pes[3];
        let video = (0xE0..=0xEF).contains(&fluxo);
        let audio = (0xC0..=0xDF).contains(&fluxo) || fluxo == 0xBD;
        if !(video || audio) || (pes[7] >> 6) & 0x2 == 0 {
            continue;
        }
        let b = &pes[9..14];
        let pts = ((b[0] as i64 >> 1) & 0x7) << 30
            | (b[1] as i64) << 22
            | ((b[2] as i64) >> 1) << 15
            | (b[3] as i64) << 7
            | (b[4] as i64) >> 1;
        if video {
            return Some(pts);
        }
        outro.get_or_insert(pts);
    }
    outro
}

/// Os primeiros bytes de um pedaço (pedido com Range; quem não aceita manda
/// o arquivo inteiro e a leitura para no limite).
fn comeco_do_pedaco(s: &Sonda) -> Option<Vec<u8>> {
    let mut pedido = AGENTE
        .get(&s.url)
        .header("User-Agent", s.agente.as_deref().unwrap_or(crate::AGENTE))
        .header("Accept", "*/*")
        .header("Range", "bytes=0-131071");
    if let Some(r) = s.referer.as_deref() {
        pedido = pedido.header("Referer", r);
    }
    let resposta = pedido.call().ok()?;
    let mut dados = Vec::new();
    resposta.into_body().into_reader().take(131_072).read_to_end(&mut dados).ok()?;
    Some(dados)
}

/// A fila da sonda: uma thread, um pedaço por vez — são poucos bytes a cada
/// poucos segundos.
static FILA: LazyLock<Mutex<Sender<Sonda>>> = LazyLock::new(|| {
    let (envio, recebe) = channel::<Sonda>();
    std::thread::Builder::new()
        .name("saimo-connect-pts".into())
        .spawn(move || {
            for s in recebe {
                if let Some(pts) = comeco_do_pedaco(&s).as_deref().and_then(primeiro_pts) {
                    historico::marcar_pts(&s.chave, s.seq, pts);
                }
            }
        })
        .ok();
    Mutex::new(envio)
});

fn sondar(s: Sonda) {
    let _ = FILA.lock().unwrap().send(s);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resolve_como_o_java() {
        let base = "https://cdn.x.com/a/b/lista.m3u8?tok=1";
        assert_eq!(resolver(base, "seg1.ts"), "https://cdn.x.com/a/b/seg1.ts");
        assert_eq!(resolver(base, "../c/seg.ts?x=2"), "https://cdn.x.com/a/c/seg.ts?x=2");
        assert_eq!(resolver(base, "/raiz/seg.ts"), "https://cdn.x.com/raiz/seg.ts");
        assert_eq!(resolver(base, "//outro.com/s.ts"), "https://outro.com/s.ts");
        assert_eq!(resolver(base, "http://abs.com/s.ts"), "http://abs.com/s.ts");
        assert_eq!(resolver("http://h.com", "s.ts"), "http://h.com/s.ts");
        assert_eq!(resolver(base, "./v/../w/1.m3u8"), "https://cdn.x.com/a/b/w/1.m3u8");
    }

    #[test]
    fn codifica_e_decodifica() {
        let url = "https://a.com/x y.m3u8?a=1&b=ç";
        assert_eq!(decodificar(&codificar(url)), url);
        assert_eq!(decodificar("a+b%20c%2"), "a b c%2");
        let p = parametros("/hls?u=https%3A%2F%2Fa.com%2Fl.m3u8&r=https%3A%2F%2Fref%2F&tv=3");
        assert_eq!(p[0], ("u".into(), "https://a.com/l.m3u8".into()));
        assert_eq!(p[2], ("tv".into(), "3".into()));
    }

    #[test]
    fn so_hls_passa_pela_regua() {
        assert!(parece_hls("https://a.com/live/index.m3u8?t=1"));
        assert!(parece_hls("http://a.com/master.TXT"));
        assert!(!parece_hls("https://a.com/canal.ts"));
        assert!(!parece_hls("https://a.com/radio.mp3"));
        assert!(!parece_hls("rtmp://a.com/x.m3u8"));
    }

    #[test]
    fn mestra_passa_as_variantes_pelo_proxy() {
        let mestra = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",URI=\"audio/pt.m3u8\"\n\
            #EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo/720.m3u8\n";
        let (texto, _) = reescrever("https://c.com/m.m3u8", "https://c.com/x/m.m3u8", mestra, "127.0.0.1:8999", Some("https://r/"), None, Some(5), 0);
        assert!(texto.contains(&format!("URI=\"http://127.0.0.1:8999/hls?u={}&r=https%3A%2F%2Fr%2F&tv=5\"", codificar("https://c.com/x/audio/pt.m3u8"))));
        assert!(texto.contains(&format!("\nhttp://127.0.0.1:8999/hls?u={}&r=https%3A%2F%2Fr%2F&tv=5\n", codificar("https://c.com/x/video/720.m3u8"))));
    }

    #[test]
    fn lista_que_nao_estende_ganha_a_regua_antiga() {
        let lista = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA-SEQUENCE:10\n#EXT-X-BYTERANGE:100@0\n#EXTINF:4,\nseg.ts\n";
        let (texto, _) = reescrever("https://c.com/b.m3u8", "https://c.com/b.m3u8", lista, "h", None, None, None, 0);
        assert!(texto.contains(&format!("#EXT-X-PROGRAM-DATE-TIME:{}\n#EXTINF:4,\nhttps://c.com/seg.ts", historico::marca_sintetica(10, 4000))));
    }

    /// Um pacote TS com começo de PES e o PTS pedido.
    fn pacote(pid: u16, fluxo: u8, pts: i64) -> Vec<u8> {
        let mut p = vec![0x47, 0x40 | (pid >> 8) as u8, pid as u8, 0x10];
        p.extend_from_slice(&[0, 0, 1, fluxo, 0, 0, 0x80, 0x80, 5]);
        p.push(0x21 | (((pts >> 30) & 0x7) << 1) as u8);
        p.push((pts >> 22) as u8);
        p.push(0x01 | (((pts >> 15) & 0x7F) << 1) as u8);
        p.push((pts >> 7) as u8);
        p.push(0x01 | ((pts & 0x7F) << 1) as u8);
        p.resize(188, 0xFF);
        p
    }

    #[test]
    fn le_o_pts_do_video() {
        let mut dados = vec![0x47, 0x40, 0x00, 0x10];
        dados.resize(188, 0xFF);
        dados.extend(pacote(0x101, 0xC0, 8_589_934_000));
        dados.extend(pacote(0x100, 0xE0, 1_234_567_890));
        assert_eq!(primeiro_pts(&dados), Some(1_234_567_890));
        // Só áudio: vale o do áudio.
        assert_eq!(primeiro_pts(&pacote(0x101, 0xC0, 77)), Some(77));
        assert_eq!(primeiro_pts(b"#EXTM3U\n"), None);
        // Com uma "imagem" na frente.
        let mut disfarcado = b"\x89PNG\r\n\x1a\n".to_vec();
        disfarcado.resize(700, 0);
        disfarcado.extend(pacote(0x100, 0xE0, 4242));
        disfarcado.extend(pacote(0x100, 0xE0, 4243));
        assert_eq!(primeiro_pts(&disfarcado), Some(4242));
    }
}
