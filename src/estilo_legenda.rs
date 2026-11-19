//! Estilo da legenda: tamanho, cor, posição e fundo. Guardado em `legenda.txt`
//! e aplicado ao mpv a cada vídeo aberto.

use std::sync::Mutex;

pub const TAMANHOS: [(&str, f64); 4] = [("Pequena", 0.75), ("Média", 1.0), ("Grande", 1.35), ("Enorme", 1.8)];
pub const CORES: [(&str, &str); 4] =
    [("Branca", "#FFFFFF"), ("Amarela", "#FFE633"), ("Ciano", "#4FE3D0"), ("Verde", "#66F266")];
/// `sub-pos`: 100 é a base da tela; menor sobe a legenda.
pub const POSICOES: [(&str, u32); 3] = [("Baixa", 100), ("Média", 92), ("Alta", 82)];

#[derive(Clone, Copy)]
pub struct Estilo {
    pub tamanho: usize,
    pub cor: usize,
    pub posicao: usize,
    pub fundo: bool,
}

static ATUAL: Mutex<Option<Estilo>> = Mutex::new(None);

fn arquivo() -> std::path::PathBuf {
    crate::catalogo::pasta().join("legenda.txt")
}

pub fn estilo() -> Estilo {
    let mut g = ATUAL.lock().unwrap();
    if let Some(e) = *g {
        return e;
    }
    let n: Vec<usize> = std::fs::read_to_string(arquivo())
        .unwrap_or_default()
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    let e = Estilo {
        tamanho: n.first().copied().unwrap_or(1).min(TAMANHOS.len() - 1),
        cor: n.get(1).copied().unwrap_or(0).min(CORES.len() - 1),
        posicao: n.get(2).copied().unwrap_or(0).min(POSICOES.len() - 1),
        fundo: n.get(3).copied().unwrap_or(0) == 0,
    };
    *g = Some(e);
    e
}

fn guardar(e: Estilo) {
    *ATUAL.lock().unwrap() = Some(e);
    let _ = std::fs::write(arquivo(), format!("{},{},{},{}", e.tamanho, e.cor, e.posicao, if e.fundo { 0 } else { 1 }));
}

pub fn proximo_tamanho() { let mut e = estilo(); e.tamanho = (e.tamanho + 1) % TAMANHOS.len(); guardar(e); }
pub fn proxima_cor() { let mut e = estilo(); e.cor = (e.cor + 1) % CORES.len(); guardar(e); }
pub fn proxima_posicao() { let mut e = estilo(); e.posicao = (e.posicao + 1) % POSICOES.len(); guardar(e); }
pub fn alternar_fundo() { let mut e = estilo(); e.fundo = !e.fundo; guardar(e); }

/// As propriedades do mpv para o estilo de agora.
pub fn propriedades() -> Vec<(&'static str, String)> {
    let e = estilo();
    vec![
        ("sub-scale", format!("{}", TAMANHOS[e.tamanho].1)),
        ("sub-color", CORES[e.cor].1.to_string()),
        ("sub-pos", POSICOES[e.posicao].1.to_string()),
        ("sub-back-color", if e.fundo { "#99000000".into() } else { "#00000000".into() }),
        ("sub-border-style", if e.fundo { "background-box".into() } else { "outline-and-shadow".into() }),
    ]
}
