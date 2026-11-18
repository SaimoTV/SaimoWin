//! Brilho do app (0 a 100; 100 é o normal): um véu preto por cima da janela,
//! não o brilho do monitor. Em 0 a janela fica preta; qualquer tecla ou clique
//! traz o brilho de volta, devagar (1,5 s): quem não vê nada não acharia o ajuste.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

pub const ESCURO: u8 = 5;
static VALOR: AtomicU8 = AtomicU8::new(255); // 255 = ainda não lido do disco
static VOLTA: std::sync::Mutex<Option<(Instant, f32)>> = std::sync::Mutex::new(None);

fn arquivo() -> std::path::PathBuf {
    crate::catalogo::pasta().join("brilho.txt")
}

/// O guardado (0 a 100).
pub fn valor() -> u8 {
    let v = VALOR.load(Ordering::Relaxed);
    if v != 255 {
        return v;
    }
    let lido = std::fs::read_to_string(arquivo()).ok().and_then(|t| t.trim().parse::<u8>().ok()).unwrap_or(100).min(100);
    VALOR.store(lido, Ordering::Relaxed);
    lido
}

pub fn definir(v: i64) {
    let v = v.clamp(0, 100) as u8;
    VALOR.store(v, Ordering::Relaxed);
    *VOLTA.lock().unwrap() = None;
    let _ = std::fs::write(arquivo(), v.to_string());
}

pub fn escuro() -> bool {
    valor() <= ESCURO && VOLTA.lock().unwrap().is_none()
}

fn voltar_ao_normal() {
    let de = valor() as f32;
    definir(100);
    *VOLTA.lock().unwrap() = Some((Instant::now(), de));
}

/// Chamada no começo de cada quadro, antes da tela: no escuro, tecla e clique
/// só acendem (não chegam ao app).
pub fn antes_do_quadro(ctx: &egui::Context) {
    if !escuro() {
        return;
    }
    let apertou = ctx.input(|i| {
        i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. } | egui::Event::PointerButton { pressed: true, .. } | egui::Event::Text(_)))
    });
    if apertou {
        ctx.input_mut(|i| {
            i.events.clear();
            i.keys_down.clear();
        });
        voltar_ao_normal();
        ctx.request_repaint();
    }
}

/// O véu, por cima de tudo (depois da tela).
pub fn desenhar(ctx: &egui::Context) {
    let mut volta = VOLTA.lock().unwrap();
    let exibido = match *volta {
        Some((inicio, de)) => {
            let t = (inicio.elapsed().as_secs_f32() / 1.5).min(1.0);
            if t >= 1.0 {
                *volta = None;
            } else {
                ctx.request_repaint();
            }
            de + (100.0 - de) * t
        }
        None => valor() as f32,
    };
    drop(volta);
    if exibido >= 100.0 {
        return;
    }
    let alfa = ((1.0 - exibido / 100.0) * 255.0).round() as u8;
    let pintor = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::new("veu-do-brilho")));
    pintor.rect_filled(ctx.content_rect(), 0.0, egui::Color32::from_black_alpha(alfa));
}
