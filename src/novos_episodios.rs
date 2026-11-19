//! Avisos de episódios novos nas séries favoritas.
//!
//! O catálogo diz quantos episódios cada série tem. Para cada série favorita
//! guarda-se a contagem "já vista"; se o catálogo passa a ter mais, nasce um
//! aviso que fica em Ajustes até ser dispensado. A primeira olhada numa série
//! só marca a contagem, sem avisar.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct Dados {
    /// título -> contagem já vista.
    vistos: HashMap<String, usize>,
    /// título -> episódios a mais desde a última vez.
    avisos: HashMap<String, usize>,
}

fn arquivo() -> std::path::PathBuf {
    crate::perfis::caminho("novos-episodios.json")
}

fn ler() -> Dados {
    std::fs::read_to_string(arquivo())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn gravar(d: &Dados) {
    if let Ok(t) = serde_json::to_string(d) {
        let _ = std::fs::write(arquivo(), t);
    }
}

pub fn resumo() -> String {
    let d = ler();
    if d.avisos.is_empty() {
        "Nenhum episódio novo".to_string()
    } else {
        format!("{} novos em {} séries · clique dispensa", d.avisos.values().sum::<usize>(), d.avisos.len())
    }
}

pub fn dispensar_todos() {
    let mut d = ler();
    let titulos: Vec<String> = d.avisos.keys().cloned().collect();
    for t in titulos {
        d.vistos.remove(&t);
    }
    d.avisos.clear();
    gravar(&d);
}

/// Confere os favoritos uma vez. Chamado de uma thread: baixa catálogo.
pub fn verificar() {
    let favoritos = crate::ler_lista("favoritos-vod.txt");
    if favoritos.is_empty() {
        return;
    }
    let acervo = crate::vod::busca();
    let mut d = ler();
    let mut por_letra: HashMap<String, Vec<crate::vod::Serie>> = HashMap::new();
    for titulo in favoritos {
        let Some(achado) = acervo.iter().find(|a| a.serie && a.titulo == titulo) else { continue };
        let lista = por_letra.entry(achado.letra.clone()).or_insert_with(|| crate::vod::series(&achado.letra));
        let Some(serie) = lista
            .iter()
            .find(|s| s.titulo == achado.titulo && s.ano == achado.ano)
            .or_else(|| lista.iter().find(|s| s.titulo == achado.titulo))
        else {
            continue;
        };
        if serie.episodios == 0 {
            continue;
        }
        match d.vistos.get(&titulo).copied() {
            None => {
                d.vistos.insert(titulo, serie.episodios);
            }
            Some(visto) if serie.episodios > visto => {
                d.avisos.insert(titulo, serie.episodios - visto);
            }
            Some(visto) if serie.episodios < visto => {
                // O catálogo foi reorganizado: recomeça a conta sem avisar.
                d.avisos.remove(&titulo);
                d.vistos.insert(titulo, serie.episodios);
            }
            _ => {}
        }
    }
    gravar(&d);
}

/// Roda uns segundos depois de abrir o app e a cada 30 minutos.
pub fn iniciar() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            verificar();
            std::thread::sleep(Duration::from_secs(1800));
        }
    });
}
