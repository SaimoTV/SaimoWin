//! Legendas externas de filmes e séries, pelo OpenSubtitles.
//!
//! O serviço é o addon público do Stremio para o OpenSubtitles
//! (opensubtitles-v3.strem.io): sem chave, sem cadastro, e entrega o arquivo já
//! em UTF-8. Só entende IMDb; o acervo só conhece o id do TMDB, e a ponte está
//! publicada em `vod/imdb/` (gerar_imdb.py) em fragmentos de uns 8 KB — baixa-se
//! o fragmento do título aberto, nunca o mapa inteiro. Mesma fonte do site, do
//! Mac, da TV Box e do celular.
//!
//! Nada é baixado antes da hora: abrir um título custa uma lista de ~30 KB, e o
//! arquivo .srt (~40 KB) só vem quando a pessoa escolhe uma legenda.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

const OPENSUBTITLES: &str = "https://opensubtitles-v3.strem.io/subtitles";
const FRAGMENTOS: u32 = 100;

/// Idiomas oferecidos, na ordem em que aparecem: código, nome, quantas versões.
const IDIOMAS: [(&str, &str, usize); 4] = [
    ("pob", "Português (Brasil)", 5),
    ("por", "Português (Portugal)", 3),
    ("eng", "Inglês", 3),
    ("spa", "Espanhol", 2),
];

#[derive(Clone, Debug)]
pub struct Opcao {
    pub id: String,
    /// "pob", "por", "eng", "spa".
    pub idioma: String,
    pub rotulo: String,
    pub url: String,
}

static FRAGMENTO: Mutex<Option<HashMap<String, HashMap<u32, String>>>> = Mutex::new(None);

/// O IMDb de um título, do fragmento publicado. Consulta a rede na primeira vez.
fn imdb_de(tmdb: u32, serie: bool) -> Option<String> {
    let nome = format!("{}-{:02}", if serie { "s" } else { "f" }, tmdb % FRAGMENTOS);
    if let Some(mapa) = FRAGMENTO.lock().ok()?.get_or_insert_with(HashMap::new).get(&nome) {
        return mapa.get(&tmdb).cloned();
    }
    // Sem o fragmento (rede fora, título ainda não mapeado) não guarda nada:
    // a próxima abertura tenta de novo. Quando não há lista de filmes ou o
    // fragmento não tem o título, cai na ponte TMDB -> IMDb dos addons.
    let via_lista = (|| {
        let base = crate::listas::filmes()?;
        let texto = crate::rede::texto(&format!("{base}imdb/{nome}.txt"))?;
        let mut mapa = HashMap::new();
        for linha in texto.lines() {
            let mut partes = linha.split('\t');
            if let (Some(id), Some(imdb)) = (partes.next(), partes.next()) {
                if let Ok(id) = id.trim().parse::<u32>() {
                    mapa.insert(id, imdb.trim().to_string());
                }
            }
        }
        let achado = mapa.get(&tmdb).cloned();
        FRAGMENTO.lock().ok()?.get_or_insert_with(HashMap::new).insert(nome, mapa);
        achado
    })();
    via_lista.or_else(|| crate::addons::tmdb_para_imdb(tmdb, serie))
}

pub fn normalizar_idioma(lang: &str) -> (&'static str, &'static str) {
    let limpo = lang.trim().to_ascii_lowercase();
    match limpo.as_str() {
        "pob" | "pb" | "pt-br" | "pt_br" | "por-br" | "portuguese-brazil" | "brazilian" => ("pob", "Português (BR)"),
        "por" | "pt" | "portuguese" => ("por", "Português"),
        "eng" | "en" | "en-us" | "en-gb" | "english" => ("eng", "Inglês"),
        "spa" | "es" | "es-la" | "es-es" | "spanish" => ("spa", "Espanhol"),
        _ => ("out", "Outro idioma"),
    }
}

fn sanitizar_id_arquivo(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Mescla legendas embutidas no stream e vindas de addons `subtitles` com a lista base.
pub fn mesclar_com_addons(
    mut base: Vec<Opcao>,
    inline: &[crate::addons::StreamSubtitle],
    extras: &[crate::addons::StreamSubtitle],
) -> Vec<Opcao> {
    let mut vistas: std::collections::HashSet<String> = base.iter().map(|o| o.url.clone()).collect();
    let mut extras_opcoes = Vec::new();
    let adicionar = |itens: &[crate::addons::StreamSubtitle], prefixo: &str, out: &mut Vec<Opcao>, vistas: &mut std::collections::HashSet<String>| {
        for (idx, sub) in itens.iter().enumerate() {
            let url = sub.url.trim();
            if url.is_empty() || !vistas.insert(url.to_string()) {
                continue;
            }
            let (cod, nome_lang) = normalizar_idioma(&sub.lang);
            let rotulo_idioma = if cod == "out" && !sub.lang.trim().is_empty() {
                sub.lang.trim().to_string()
            } else {
                nome_lang.to_string()
            };
            let sufixo_id = sub
                .id
                .as_deref()
                .map(sanitizar_id_arquivo)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("{idx}"));
            let origem = if prefixo == "stream" { "Stream" } else { "Addon" };
            out.push(Opcao {
                id: format!("{cod}-{prefixo}-{sufixo_id}-{idx}"),
                idioma: cod.to_string(),
                rotulo: format!("{rotulo_idioma} · {origem}"),
                url: url.to_string(),
            });
        }
    };
    adicionar(inline, "stream", &mut extras_opcoes, &mut vistas);
    adicionar(extras, "addon", &mut extras_opcoes, &mut vistas);
    extras_opcoes.sort_by_key(|o| match o.idioma.as_str() {
        "pob" => 0,
        "por" => 1,
        "eng" => 2,
        "spa" => 3,
        _ => 4,
    });
    extras_opcoes.append(&mut base);
    extras_opcoes
}

/// As legendas do título, do melhor idioma para o pior. Consulta a rede:
/// chamar fora da thread do desenho.
pub fn buscar(tmdb: u32, serie: bool, temporada: u32, episodio: u32) -> Vec<Opcao> {
    if tmdb == 0 {
        return Vec::new();
    }
    let Some(imdb) = imdb_de(tmdb, serie) else { return Vec::new() };
    let alvo = if serie && temporada > 0 {
        format!("series/{imdb}:{temporada}:{episodio}")
    } else {
        format!("movie/{imdb}")
    };
    let Some(json) = crate::rede::json(&format!("{OPENSUBTITLES}/{alvo}.json")) else { return Vec::new() };
    let Some(todas) = json.get("subtitles").and_then(|s| s.as_array()) else { return Vec::new() };
    let mut saida = Vec::new();
    for (codigo, nome, limite) in IDIOMAS {
        let doidioma = todas
            .iter()
            .filter(|s| s.get("lang").and_then(|l| l.as_str()) == Some(codigo) && s.get("url").is_some())
            .take(limite);
        for (i, s) in doidioma.enumerate() {
            let texto = |campo: &str| s.get(campo).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            let versao = [texto("releaseGroup"), texto("releaseFormat")]
                .into_iter()
                .find(|v| !v.is_empty())
                .unwrap_or_else(|| (i + 1).to_string());
            let id = s.get("id").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or_else(|| i.to_string());
            saida.push(Opcao {
                id: format!("{codigo}-{id}"),
                idioma: codigo.to_string(),
                rotulo: format!("{nome} · {versao}"),
                url: texto("url"),
            });
        }
    }
    saida
}

/// Baixa o arquivo para a pasta temporária e devolve o caminho, que é o que o
/// mpv abre. Consulta a rede: chamar fora da thread do desenho.
pub fn baixar(opcao: &Opcao) -> Option<PathBuf> {
    let ext = if opcao.url.split('?').next().unwrap_or("").to_ascii_lowercase().ends_with(".vtt") {
        "vtt"
    } else if opcao.url.split('?').next().unwrap_or("").to_ascii_lowercase().ends_with(".ass") {
        "ass"
    } else {
        "srt"
    };
    let id_limpo = sanitizar_id_arquivo(&opcao.id);
    let caminho = std::env::temp_dir().join(format!("saimo-legenda-{id_limpo}.{ext}"));
    if caminho.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Some(caminho);
    }
    let texto = crate::rede::texto(&opcao.url)?;
    // Página de erro no lugar do arquivo: melhor sem legenda que com HTML na tela.
    let eh_legenda = texto.contains("-->") || texto.contains("[Script Info]") || texto.contains("Dialogue:");
    if !eh_legenda {
        return None;
    }
    std::fs::write(&caminho, texto).ok()?;
    Some(caminho)
}

fn arquivo_do_idioma() -> PathBuf {
    crate::catalogo::pasta().join("legenda-idioma.txt")
}

/// O idioma escolhido da última vez; vazio é "desligadas".
pub fn idioma_guardado() -> String {
    std::fs::read_to_string(arquivo_do_idioma()).map(|t| t.trim().to_string()).unwrap_or_default()
}

pub fn guardar_idioma(codigo: &str) {
    let _ = std::fs::write(arquivo_do_idioma(), codigo);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sem_id_nao_consulta_a_rede() {
        assert!(buscar(0, false, 0, 0).is_empty());
    }

    #[test]
    fn mescla_legendas_inline_e_addons_ordenando_portugues_primeiro() {
        let inline = vec![crate::addons::StreamSubtitle {
            id: Some("sub:en".into()),
            url: "https://cdn.exemplo/en.vtt".into(),
            lang: "eng".into(),
        }];
        let addons = vec![crate::addons::StreamSubtitle {
            id: Some("sub/pt".into()),
            url: "https://cdn.exemplo/pt.srt".into(),
            lang: "pt-BR".into(),
        }];
        let base = vec![Opcao {
            id: "os-1".into(),
            idioma: "spa".into(),
            rotulo: "Espanhol · OpenSubtitles".into(),
            url: "https://os.exemplo/es.srt".into(),
        }];
        let merged = mesclar_com_addons(base, &inline, &addons);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].idioma, "pob");
        assert_eq!(merged[1].idioma, "eng");
        assert_eq!(merged[2].idioma, "spa");
    }
}
