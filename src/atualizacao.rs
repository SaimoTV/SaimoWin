//! Procura versão nova no `versao.json` do endereço estável de canais e abre o
//! download no navegador.
//!
//! O arquivo diz qual é a versão e onde está o instalador de cada plataforma;
//! nenhum dono ou repositório fica gravado aqui. Falhou ao ler, não há
//! atualização. O app não baixa nem troca nada por dentro, então o `sha256`
//! (quando vem) fica guardado em `Versao` para quem baixar conferir com
//! `confere_sha256`.

use std::sync::Mutex;

/// Plataforma deste app em `arquivos` do `versao.json`.
const PLATAFORMA: &str = "windows";

#[derive(Clone, Debug)]
pub struct Versao {
    pub numero: String,
    pub notas: String,
    pub link: String,
    #[allow(dead_code)]
    pub bytes: u64,
    #[allow(dead_code)]
    pub sha256: Option<String>,
}

static ADIADA: Mutex<Option<String>> = Mutex::new(None);

/// "v1.2.3" e "1.2" viram "1.2.3" e "1.2"; qualquer outra coisa, nada.
fn numero_da_tag(tag: &str) -> Option<String> {
    let limpo = tag.trim_start_matches(['v', 'V']);
    let partes: Vec<&str> = limpo.split('.').collect();
    if !(2..=3).contains(&partes.len()) || !partes.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    Some(limpo.to_string())
}

/// Compara 1.10.0 com 1.9.3 por parte, e não como texto.
pub fn mais_nova(candidata: &str, atual: &str) -> bool {
    let numeros = |v: &str| -> Vec<u32> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (a, b) = (numeros(candidata), numeros(atual));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

pub fn procurar() -> Option<Versao> {
    let dados = crate::rede::json(&format!("{}versao.json", crate::origens::CANAIS))?;
    let achada = ler_versao(&dados)?;
    if !mais_nova(&achada.numero, crate::VERSAO) {
        return None;
    }
    if ADIADA.lock().unwrap().as_deref() == Some(achada.numero.as_str()) {
        return None;
    }
    Some(achada)
}

pub fn adiar(versao: &Versao) {
    *ADIADA.lock().unwrap() = Some(versao.numero.clone());
}

/// Lê o `versao.json`; qualquer campo faltando ou link fora da lista segura
/// devolve `None`.
fn ler_versao(dados: &serde_json::Value) -> Option<Versao> {
    let numero = numero_da_tag(dados.get("versao").and_then(|v| v.as_str())
        .or_else(|| dados.get("tag").and_then(|v| v.as_str()))?)?;
    let arq = dados.get("arquivos")?.get(PLATAFORMA)?;
    let link = arq.get("url")?.as_str()?.to_string();
    if !link_oficial(&link) {
        return None;
    }
    let sha256 = arq.get("sha256").and_then(|v| v.as_str())
        .map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty());
    if let Some(h) = &sha256 {
        if h.len() != 64 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
    }
    Some(Versao {
        numero,
        notas: dados.get("notas").and_then(|b| b.as_str()).unwrap_or("").to_string(),
        link,
        bytes: arq.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0),
        sha256,
    })
}

/// Só abre downloads HTTPS do GitHub: `github.com/<dono>/<repo>/releases/download/…`
/// ou os servidores para onde o GitHub redireciona os arquivos. Qualquer dono
/// serve, para o repositório poder mudar de conta.
fn link_oficial(link: &str) -> bool {
    if link.chars().any(|c| c.is_control() || c.is_whitespace() || c == '\\') {
        return false;
    }
    let Some(resto) = link.strip_prefix("https://") else { return false };
    let (host, caminho) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, ""),
    };
    match host {
        "github.com" => {
            let partes: Vec<&str> = caminho.trim_start_matches('/').split('/').collect();
            partes.len() >= 5
                && !partes[0].is_empty()
                && !partes[1].is_empty()
                && partes[2] == "releases"
                && partes[3] == "download"
                && !partes[4].is_empty()
        }
        "objects.githubusercontent.com" | "release-assets.githubusercontent.com" => caminho.len() > 1,
        _ => false,
    }
}

/// Confere o SHA-256 (hex) de um arquivo baixado com o esperado.
#[allow(dead_code)]
pub fn confere_sha256(dados: &[u8], esperado: &str) -> bool {
    sha256_hex(dados) == esperado.trim().to_ascii_lowercase()
}

pub fn sha256_hex(dados: &[u8]) -> String {
    sha256(dados).iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 (FIPS 180-4).
fn sha256(dados: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let mut msg = dados.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((dados.len() as u64) * 8).to_be_bytes());
    for bloco in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([bloco[i * 4], bloco[i * 4 + 1], bloco[i * 4 + 2], bloco[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g; g = f; f = e;
            e = d.wrapping_add(t1);
            d = c; c = b; b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut fora = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        fora[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    fora
}

pub fn abrir_no_navegador(link: &str) {
    if !link_oficial(link) { return; }
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(hwnd: *mut c_void, operation: *const u16,
                file: *const u16, parameters: *const u16, directory: *const u16,
                show: i32) -> *mut c_void;
        }
        let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        let url: Vec<u16> = link.encode_utf16().chain(Some(0)).collect();
        let result = unsafe { ShellExecuteW(std::ptr::null_mut(), operation.as_ptr(),
            url.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
        if result as isize <= 32 {
            eprintln!("Não foi possível abrir o navegador (Windows: {}).", result as isize);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("open").arg(link).spawn();
    }
}

#[cfg(test)]
mod testes_links {
    use super::*;
    use serde_json::json;

    #[test]
    fn restringe_downloads_ao_github() {
        assert!(link_oficial("https://github.com/dono/repo/releases/download/v1.7.7/SaimoTV-Windows.zip"));
        assert!(link_oficial("https://objects.githubusercontent.com/github-production-release-asset/abc?x=1"));
        assert!(link_oficial("https://release-assets.githubusercontent.com/github-production-release-asset/abc"));
        for url in ["file:///C:/bad.exe", "http://github.com/dono/repo/releases/download/v1/a.zip",
                    "https://github.com.evil.test/dono/repo/releases/download/v1/a.zip",
                    "https://github.com@evil.test/dono/repo/releases/download/v1/a.zip",
                    "https://evil.test/github.com/dono/repo/releases/download/v1/a.zip",
                    "https://objects.githubusercontent.com.evil.test/abc",
                    "https://github.com/dono/repo/releases/download/v1/a.zip\ncalc.exe",
                    "https://github.com/dono/repo/releases/latest",
                    "https://github.com/dono/repo/raw/main/a.zip",
                    "https://github.com/dono/releases/download/v1/a.zip",
                    "https://github.com/", ""] {
            assert!(!link_oficial(url), "{url}");
        }
    }

    fn exemplo() -> serde_json::Value {
        json!({ "versao": "2.4.0", "tag": "v2.4.0", "notas": "novidades",
            "arquivos": { "windows": { "nome": "SaimoTV-Windows.zip",
                "url": "https://github.com/dono/repo/releases/download/v2.4.0/SaimoTV-Windows.zip",
                "bytes": 123, "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad" },
              "mac": { "url": "https://evil.test/x" } } })
    }

    #[test]
    fn le_versao_json() {
        let v = ler_versao(&exemplo()).unwrap();
        assert_eq!(v.numero, "2.4.0");
        assert_eq!(v.notas, "novidades");
        assert_eq!(v.bytes, 123);
        assert!(v.link.ends_with("SaimoTV-Windows.zip"));
        assert!(v.sha256.is_some());
    }

    #[test]
    fn versao_json_invalido_vira_nada() {
        assert!(ler_versao(&json!({})).is_none());
        assert!(ler_versao(&json!({"versao": "lixo", "arquivos": {}})).is_none());
        let mut sem_windows = exemplo();
        sem_windows["arquivos"].as_object_mut().unwrap().remove("windows");
        assert!(ler_versao(&sem_windows).is_none());
        let mut link_ruim = exemplo();
        link_ruim["arquivos"]["windows"]["url"] = json!("https://github.com.evil.test/a/b/releases/download/v/x");
        assert!(ler_versao(&link_ruim).is_none());
        let mut hash_ruim = exemplo();
        hash_ruim["arquivos"]["windows"]["sha256"] = json!("zz");
        assert!(ler_versao(&hash_ruim).is_none());
        let mut sem_hash = exemplo();
        sem_hash["arquivos"]["windows"].as_object_mut().unwrap().remove("sha256");
        assert!(ler_versao(&sem_hash).unwrap().sha256.is_none());
    }

    #[test]
    fn sha256_conhecidos() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(&[b'a'; 1000]), sha256_hex(&vec![b'a'; 1000]));
        assert!(confere_sha256(b"abc", "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"));
        assert!(!confere_sha256(b"abd", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    }

    #[test]
    fn compara_versoes() {
        assert!(mais_nova("2.4.0", "2.3.9"));
        assert!(mais_nova("1.10.0", "1.9.3"));
        assert!(!mais_nova("2.3.0", "2.3"));
    }
}
