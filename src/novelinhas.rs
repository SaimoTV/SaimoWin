//! Novelinhas: minisséries verticais (720x1280, ~1 min por episódio) da
//! RedeFlix, em `novelinhas.txt`:
//!
//!     base: https://.../novelinhas/
//!     id  título  pasta  episódios  áudio(dub|leg)  capa  gêneros  sinopse
//!
//! Episódio N = base + pasta + "/episode-" + N com três dígitos + ".mp4".

#[derive(Clone, Debug)]
pub struct Novelinha {
    pub id: String,
    pub titulo: String,
    pub pasta: String,
    pub episodios: u32,
    pub dublada: bool,
    pub capa: String,
    pub generos: Vec<String>,
    pub sinopse: String,
}

#[derive(Default, Clone)]
pub struct Novelinhas {
    pub base: String,
    pub lista: Vec<Novelinha>,
}

impl Novelinhas {
    pub fn url(&self, n: &Novelinha, ep: u32) -> String {
        format!("{}{}/episode-{:03}.mp4", self.base, n.pasta, ep)
    }

    pub fn por_id(&self, id: &str) -> Option<&Novelinha> {
        self.lista.iter().find(|n| n.id == id)
    }

    /// Todos os episódios a partir de `de`, numa playlist .m3u para o mpv:
    /// no fim de um, o seguinte entra sozinho.
    pub fn playlist(&self, n: &Novelinha, de: u32) -> Option<std::path::PathBuf> {
        let mut texto = String::from("#EXTM3U\n");
        for ep in de.max(1)..=n.episodios {
            texto.push_str(&format!("#EXTINF:-1,{} - Episódio {ep}\n{}\n", n.titulo, self.url(n, ep)));
        }
        let caminho = std::env::temp_dir().join(format!("saimo-novelinha-{}.m3u", n.id));
        std::fs::write(&caminho, texto).ok()?;
        Some(caminho)
    }
}

pub fn baixar() -> Novelinhas {
    let Some(texto) = crate::vod::arquivo_publico("novelinhas.txt") else {
        return Novelinhas::default();
    };
    let mut base = String::new();
    let mut lista = Vec::new();
    for linha in texto.lines() {
        if linha.starts_with('#') || linha.is_empty() {
            continue;
        }
        if let Some(b) = linha.strip_prefix("base:") {
            base = b.trim().to_string();
            continue;
        }
        let c: Vec<&str> = linha.split('\t').collect();
        let eps: u32 = c.get(3).and_then(|e| e.parse().ok()).unwrap_or(0);
        if c.len() < 6 || eps == 0 {
            continue;
        }
        lista.push(Novelinha {
            id: c[0].into(),
            titulo: c[1].into(),
            pasta: c[2].into(),
            episodios: eps,
            dublada: c[4] == "dub",
            capa: c[5].into(),
            generos: c.get(6).unwrap_or(&"").split(',').map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
            sinopse: c.get(7).unwrap_or(&"").to_string(),
        });
    }
    if base.is_empty() {
        return Novelinhas::default();
    }
    Novelinhas { base, lista }
}
