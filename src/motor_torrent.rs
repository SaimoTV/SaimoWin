//! Tocador de torrent embutido: toca fonte de addon que só tem `infoHash`, sem
//! depender de Debrid (como o Stremio faz).
//!
//! O motor é o `saimo-torrent` (Rust, librqbit), que vai na mesma pasta do app.
//! Ele abre um servidor só em 127.0.0.1 e o mpv toca
//! `http://127.0.0.1:<porta>/tocar?m=<magnet>&f=<arquivo>` como um link comum:
//! baixa primeiro o trecho que o player pede (inclusive ao avançar) e o resto
//! depois. O motor sai junto com o app (quando a entrada dele fecha) e apaga o
//! que baixou.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

struct Motor {
    processo: Child,
    porta: u16,
}

fn motor() -> &'static Mutex<Option<Motor>> {
    static M: OnceLock<Mutex<Option<Motor>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(None))
}

fn binario() -> Option<PathBuf> {
    let nome = if cfg!(windows) { "saimo-torrent.exe" } else { "saimo-torrent" };
    let p = std::env::current_exe().ok()?.parent()?.join(nome);
    p.is_file().then_some(p)
}

pub fn disponivel() -> bool {
    binario().is_some()
}

fn iniciar() -> Result<u16, String> {
    let mut guarda = motor().lock().unwrap();
    if let Some(m) = guarda.as_mut() {
        if matches!(m.processo.try_wait(), Ok(None)) {
            return Ok(m.porta);
        }
    }
    let bin = binario().ok_or("Motor de torrent ausente.")?;
    let pasta = std::env::temp_dir().join("SaimoTV").join("torrent");
    let mut cmd = Command::new(bin);
    cmd.args(["--pasta", &pasta.to_string_lossy(), "--porta", "0", "--limite-mb", "8192"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: sem janela de console
    }
    let mut filho = cmd.spawn().map_err(|e| format!("Não foi possível iniciar o motor de torrent: {e}"))?;
    let saida = filho.stdout.take().ok_or("motor sem saída")?;
    let mut leitor = BufReader::new(saida);
    let mut linha = String::new();
    leitor.read_line(&mut linha).map_err(|e| e.to_string())?;
    let porta: u16 = linha
        .trim()
        .strip_prefix("PRONTO ")
        .and_then(|p| p.parse().ok())
        .ok_or("O motor de torrent não iniciou.")?;
    // O resto da saída é descartado numa thread (senão o cano enche e trava).
    std::thread::spawn(move || {
        let mut l = String::new();
        while leitor.read_line(&mut l).map(|n| n > 0).unwrap_or(false) {
            l.clear();
        }
    });
    *guarda = Some(Motor { processo: filho, porta });
    Ok(porta)
}

fn escapar(texto: &str) -> String {
    texto
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Endereço local para o mpv tocar o torrent.
pub fn endereco(magnet: &str, file_idx: Option<usize>) -> Result<String, String> {
    let porta = iniciar()?;
    let mut url = format!("http://127.0.0.1:{porta}/tocar?m={}", escapar(magnet));
    if let Some(f) = file_idx {
        url.push_str(&format!("&f={f}"));
    }
    Ok(url)
}

pub fn eh_endereco_do_motor(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:") && url.contains("/tocar?m=")
}

/// O filme fechou: apaga o que baixou. O motor continua no ar.
pub fn limpar() {
    let porta = motor().lock().unwrap().as_ref().map(|m| m.porta);
    if let Some(p) = porta {
        std::thread::spawn(move || {
            let _ = ureq_parar(p);
        });
    }
}

fn ureq_parar(porta: u16) -> std::io::Result<()> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", porta))?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    s.write_all(b"GET /parar HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;
    let mut lixo = Vec::new();
    let _ = s.read_to_end(&mut lixo);
    Ok(())
}
