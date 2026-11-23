//! Acessibilidade (Ajustes > Aparência e acessibilidade): tamanho da tela
//! (Normal, Grande, Muito grande, Enorme), texto em negrito e foco reforçado
//! (contorno amarelo grosso). A tela de Ajustes mostra uma prévia; só
//! "Aplicar" grava e muda o app.

use std::sync::Mutex;

pub const ESCALAS: [f32; 4] = [1.0, 1.15, 1.3, 1.45];
pub const NOMES: [&str; 4] = ["Normal", "Grande", "Muito grande", "Enorme"];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Escolha {
    pub nivel: usize,
    pub negrito: bool,
    pub foco_forte: bool,
}

static ATUAL: Mutex<Option<Escolha>> = Mutex::new(None);

fn arquivo() -> std::path::PathBuf {
    crate::catalogo::pasta().join("acessibilidade.txt")
}

/// A escolha guardada ("nivel negrito foco", ex.: "2 1 0").
pub fn atual() -> Escolha {
    if let Some(e) = *ATUAL.lock().unwrap() {
        return e;
    }
    let t = std::fs::read_to_string(arquivo()).unwrap_or_default();
    let p: Vec<&str> = t.split_whitespace().collect();
    let e = Escolha {
        nivel: p.first().and_then(|v| v.parse().ok()).unwrap_or(0).min(ESCALAS.len() - 1),
        negrito: p.get(1) == Some(&"1"),
        foco_forte: p.get(2) == Some(&"1"),
    };
    *ATUAL.lock().unwrap() = Some(e);
    e
}

pub fn aplicar(ctx: &egui::Context, e: Escolha) {
    *ATUAL.lock().unwrap() = Some(e);
    let _ = std::fs::write(arquivo(), format!("{} {} {}", e.nivel, e.negrito as u8, e.foco_forte as u8));
    ctx.request_repaint();
}

pub fn escala() -> f32 {
    ESCALAS[atual().nivel]
}

pub fn resumo() -> String {
    let e = atual();
    let mut partes = vec![NOMES[e.nivel].to_string()];
    if e.negrito {
        partes.push("negrito".into());
    }
    if e.foco_forte {
        partes.push("foco reforçado".into());
    }
    partes.join(" · ")
}
