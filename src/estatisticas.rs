//! Estatísticas pessoais do perfil: tempo assistido, filmes e episódios vistos.
//!
//! O tempo soma a cada gravação do ponto, com teto por intervalo para uma
//! pausa longa ou um salto não inflar o total. "Visto" é o título que chegou
//! aos dois minutos finais (ver `progresso::salvar`).

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// A gravação vem a cada 15 s: intervalo maior que isso foi pausa ou outro título.
const MAXIMO_S: f64 = 20.0;

#[derive(Default, Serialize, Deserialize)]
struct Dados {
    segundos: f64,
    vistos: HashSet<String>,
}

static DADOS: Mutex<Option<Dados>> = Mutex::new(None);
static ULTIMO: Mutex<Option<Instant>> = Mutex::new(None);

fn arquivo() -> std::path::PathBuf {
    crate::perfis::caminho("estatisticas.json")
}

fn com<R>(f: impl FnOnce(&mut Dados) -> R) -> R {
    let mut guarda = DADOS.lock().unwrap();
    if guarda.is_none() {
        let lido = std::fs::read_to_string(arquivo())
            .ok()
            .and_then(|t| serde_json::from_str::<Dados>(&t).ok())
            .unwrap_or_default();
        *guarda = Some(lido);
    }
    f(guarda.as_mut().unwrap())
}

fn gravar() {
    let texto = com(|d| serde_json::to_string(d).ok());
    if let Some(t) = texto {
        let _ = std::fs::write(arquivo(), t);
    }
}

/// Chamar a cada gravação do ponto enquanto o vídeo toca.
pub fn tocando() {
    let agora = Instant::now();
    let antes = ULTIMO.lock().unwrap().replace(agora);
    let Some(antes) = antes else { return };
    let delta = agora.duration_since(antes).as_secs_f64();
    if delta > 0.0 && delta <= MAXIMO_S {
        com(|d| d.segundos += delta);
        gravar();
    }
}

pub fn marcar_visto(titulo: &str) {
    if com(|d| d.vistos.insert(titulo.to_string())) {
        gravar();
    }
}

pub fn resumo() -> String {
    com(|d| {
        let min = (d.segundos / 60.0) as u64;
        let episodios = d.vistos.iter().filter(|t| t.contains(" · T")).count();
        let filmes = d.vistos.len() - episodios;
        format!("{} h {} min · {filmes} filmes · {episodios} episódios", min / 60, min % 60)
    })
}
