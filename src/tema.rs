//! Cor de destaque do app, escolhida em Ajustes e guardada em `tema.txt`.

use std::sync::atomic::{AtomicUsize, Ordering};

use eframe::egui::Color32;

pub const CORES: [(&str, [u8; 3]); 5] = [
    ("Ciano", [79, 227, 208]),
    ("Violeta", [167, 139, 250]),
    ("Laranja", [251, 146, 60]),
    ("Rosa", [244, 114, 182]),
    ("Verde", [74, 222, 128]),
];

static INDICE: AtomicUsize = AtomicUsize::new(usize::MAX);

fn arquivo() -> std::path::PathBuf {
    crate::catalogo::pasta().join("tema.txt")
}

pub fn indice() -> usize {
    let v = INDICE.load(Ordering::Relaxed);
    if v != usize::MAX {
        return v;
    }
    let lido = std::fs::read_to_string(arquivo()).ok().and_then(|t| t.trim().parse::<usize>().ok()).unwrap_or(0).min(CORES.len() - 1);
    INDICE.store(lido, Ordering::Relaxed);
    lido
}

pub fn acento() -> Color32 {
    let [r, g, b] = CORES[indice()].1;
    Color32::from_rgb(r, g, b)
}

pub fn proximo() {
    let novo = (indice() + 1) % CORES.len();
    INDICE.store(novo, Ordering::Relaxed);
    let _ = std::fs::write(arquivo(), novo.to_string());
}
