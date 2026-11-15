//! Resolução Debrid no cliente (client-side REST via `ureq`) para streams
//! torrent (`infoHash`, `fileIdx`, `sources`, `clientResolve`).
//!
//! Suporta **TorBox**, **Real-Debrid**, **AllDebrid** e **Premiumize**.
//! Zero torrent P2P local (`libtorrent`). Por padrão, as chaves não saem do
//! dispositivo (`sincronizarConnect: false`), a menos que o usuário habilite.

use crate::addons::{codificar_segmento, AddonStream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

const ARQUIVO_DEBRID: &str = "saimo_debrid_v1.json";

pub const AVISO_SEM_CHAVE: &str = "Este stream é um torrent (infoHash) e precisa de uma chave Debrid ativa (TorBox, Real-Debrid, AllDebrid ou Premiumize). Configure em Ajustes → Addons e Debrid ou pelo QR Code.";

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProvedorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, rename = "apiKey", alias = "api_key")]
    pub api_key: String,
}

impl ProvedorConfig {
    pub fn ativo(&self) -> bool {
        self.enabled && !self.api_key.trim().is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ConfigDebrid {
    #[serde(default, rename = "sincronizarConnect", alias = "sincronizar_connect")]
    pub sincronizar_connect: bool,
    #[serde(default)]
    pub torbox: ProvedorConfig,
    #[serde(default, rename = "realDebrid", alias = "real_debrid")]
    pub real_debrid: ProvedorConfig,
    #[serde(default, rename = "allDebrid", alias = "all_debrid")]
    pub all_debrid: ProvedorConfig,
    #[serde(default)]
    pub premiumize: ProvedorConfig,
}

impl ConfigDebrid {
    pub fn tem_ativo(&self) -> bool {
        self.torbox.ativo()
            || self.real_debrid.ativo()
            || self.all_debrid.ativo()
            || self.premiumize.ativo()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamResolvido {
    pub url: String,
    pub cabecalhos: Vec<(String, String)>,
    pub provedor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArquivoTorrent {
    pub id_interno: String,
    pub nome: String,
    pub bytes: u64,
}

static CONFIG: OnceLock<RwLock<ConfigDebrid>> = OnceLock::new();

fn arquivo_config() -> PathBuf {
    crate::catalogo::pasta().join(ARQUIVO_DEBRID)
}

fn cache() -> &'static RwLock<ConfigDebrid> {
    CONFIG.get_or_init(|| RwLock::new(carregar_em(&arquivo_config())))
}

pub fn carregar_em(caminho: &Path) -> ConfigDebrid {
    std::fs::read_to_string(caminho)
        .ok()
        .and_then(|t| serde_json::from_str::<ConfigDebrid>(&t).ok())
        .unwrap_or_default()
}

pub fn salvar_em(caminho: &Path, cfg: &ConfigDebrid) -> Result<(), String> {
    if let Some(pai) = caminho.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    let texto = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("Falha ao serializar config Debrid: {e}"))?;
    std::fs::write(caminho, texto).map_err(|e| format!("Falha ao salvar config Debrid: {e}"))
}

pub fn config() -> ConfigDebrid {
    cache().read().map(|g| g.clone()).unwrap_or_default()
}

pub fn salvar_config(cfg: ConfigDebrid) -> Result<(), String> {
    salvar_em(&arquivo_config(), &cfg)?;
    if let Ok(mut g) = cache().write() {
        *g = cfg;
    }
    Ok(())
}

pub fn aplicar_do_connect_em(caminho: &Path, json: &Value) -> ConfigDebrid {
    let mut atual = carregar_em(caminho);
    let parsed: ConfigDebrid = serde_json::from_value(json.clone()).unwrap_or_default();
    if parsed.sincronizar_connect {
        atual = parsed;
    } else {
        // Mescla apenas se chaves não vazias foram enviadas explicitamente.
        if !parsed.torbox.api_key.trim().is_empty() {
            atual.torbox = parsed.torbox;
        }
        if !parsed.real_debrid.api_key.trim().is_empty() {
            atual.real_debrid = parsed.real_debrid;
        }
        if !parsed.all_debrid.api_key.trim().is_empty() {
            atual.all_debrid = parsed.all_debrid;
        }
        if !parsed.premiumize.api_key.trim().is_empty() {
            atual.premiumize = parsed.premiumize;
        }
    }
    let _ = salvar_em(caminho, &atual);
    atual
}

pub fn aplicar_do_connect(json: &Value) {
    let caminho = arquivo_config();
    let novo = aplicar_do_connect_em(&caminho, json);
    if let Ok(mut g) = cache().write() {
        *g = novo;
    }
}

pub fn eh_video(nome: &str) -> bool {
    let min = nome.trim().to_ascii_lowercase();
    [".mkv", ".mp4", ".m4v", ".mov", ".avi", ".webm", ".ts", ".m2ts"]
        .iter()
        .any(|ext| min.ends_with(ext))
}

/// Escolhe o arquivo de vídeo principal de um torrent respeitando `fileIdx`
/// ou caindo no maior arquivo de vídeo (ignorando `.sample` se houver outro).
pub fn selecionar_arquivo(arquivos: &[ArquivoTorrent], file_idx: Option<usize>) -> Option<usize> {
    if arquivos.is_empty() {
        return None;
    }
    if let Some(idx) = file_idx {
        if let Some(a) = arquivos.get(idx) {
            if eh_video(&a.nome) || arquivos.len() == 1 {
                return Some(idx);
            }
        }
    }
    let mut videos: Vec<(usize, &ArquivoTorrent)> = arquivos
        .iter()
        .enumerate()
        .filter(|(_, a)| eh_video(&a.nome))
        .collect();
    if videos.is_empty() {
        return arquivos
            .iter()
            .enumerate()
            .max_by_key(|(_, a)| a.bytes)
            .map(|(i, _)| i);
    }
    let nao_sample: Vec<(usize, &ArquivoTorrent)> = videos
        .iter()
        .copied()
        .filter(|(_, a)| !a.nome.to_ascii_lowercase().contains("sample"))
        .collect();
    if !nao_sample.is_empty() {
        videos = nao_sample;
    }
    videos
        .into_iter()
        .max_by_key(|(_, a)| a.bytes)
        .map(|(i, _)| i)
}

fn codificar_query_param(valor: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(valor.len());
    for byte in valor.as_bytes() {
        let ok = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if ok {
            out.push(*byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0F) as usize] as char);
        }
    }
    out
}

pub fn montar_magnet(stream: &AddonStream) -> Option<String> {
    if let Some(ref cr) = stream.client_resolve {
        if let Some(m) = cr.get("magnet").and_then(Value::as_str) {
            if m.starts_with("magnet:") {
                return Some(m.to_string());
            }
        }
    }
    let hash_bruto = stream
        .info_hash
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .or_else(|| {
            stream
                .client_resolve
                .as_ref()
                .and_then(|v| v.get("infoHash").and_then(Value::as_str))
        })?;
    let hash = hash_bruto
        .strip_prefix("urn:btih:")
        .unwrap_or(hash_bruto)
        .to_ascii_lowercase();
    let mut magnet = format!("magnet:?xt=urn:btih:{hash}");
    if let Some(nome) = stream.behavior_hints.filename.as_deref().or(stream.name.as_deref()) {
        let limpo = nome.lines().next().unwrap_or(nome).trim();
        if !limpo.is_empty() {
            magnet.push_str("&dn=");
            magnet.push_str(&codificar_query_param(limpo));
        }
    }
    if let Some(ref sources) = stream.sources {
        for src in sources {
            if let Some(tr) = src.strip_prefix("tracker:") {
                magnet.push_str("&tr=");
                magnet.push_str(&codificar_query_param(tr));
            }
        }
    }
    Some(magnet)
}

/// Resolve um `AddonStream` em URL reproduzível + headers HTTP.
/// Streams HTTP diretos passam direto; streams `infoHash` usam o provedor
/// Debrid configurado pelo usuário.
pub fn resolver_stream(stream: &AddonStream) -> Result<StreamResolvido, String> {
    if let Some(ref url) = stream.url {
        let limpa = url.trim();
        if !limpa.is_empty() {
            return Ok(StreamResolvido {
                url: limpa.to_string(),
                cabecalhos: stream.cabecalhos_requisicao(),
                provedor: None,
            });
        }
    }
    let magnet = montar_magnet(stream)
        .ok_or_else(|| "Stream sem URL direta e sem infoHash válido.".to_string())?;
    let cfg = config();
    if !cfg.tem_ativo() {
        // Sem Debrid: toca pelo motor de torrent embutido, como o Stremio.
        if crate::motor_torrent::disponivel() {
            return Ok(StreamResolvido {
                url: crate::motor_torrent::endereco(&magnet, stream.file_idx)?,
                cabecalhos: Vec::new(),
                provedor: Some("Torrent".into()),
            });
        }
        return Err(AVISO_SEM_CHAVE.into());
    }
    let mut erros = Vec::new();
    if cfg.torbox.ativo() {
        match resolver_torbox(cfg.torbox.api_key.trim(), &magnet, stream.file_idx) {
            Ok(url) => {
                return Ok(StreamResolvido {
                    url,
                    cabecalhos: stream.cabecalhos_requisicao(),
                    provedor: Some("TorBox".into()),
                })
            }
            Err(e) => erros.push(format!("TorBox: {e}")),
        }
    }
    if cfg.real_debrid.ativo() {
        match resolver_real_debrid(cfg.real_debrid.api_key.trim(), &magnet, stream.file_idx) {
            Ok(url) => {
                return Ok(StreamResolvido {
                    url,
                    cabecalhos: stream.cabecalhos_requisicao(),
                    provedor: Some("Real-Debrid".into()),
                })
            }
            Err(e) => erros.push(format!("Real-Debrid: {e}")),
        }
    }
    if cfg.all_debrid.ativo() {
        match resolver_all_debrid(cfg.all_debrid.api_key.trim(), &magnet, stream.file_idx) {
            Ok(url) => {
                return Ok(StreamResolvido {
                    url,
                    cabecalhos: stream.cabecalhos_requisicao(),
                    provedor: Some("AllDebrid".into()),
                })
            }
            Err(e) => erros.push(format!("AllDebrid: {e}")),
        }
    }
    if cfg.premiumize.ativo() {
        match resolver_premiumize(cfg.premiumize.api_key.trim(), &magnet, stream.file_idx) {
            Ok(url) => {
                return Ok(StreamResolvido {
                    url,
                    cabecalhos: stream.cabecalhos_requisicao(),
                    provedor: Some("Premiumize".into()),
                })
            }
            Err(e) => erros.push(format!("Premiumize: {e}")),
        }
    }
    Err(format!(
        "Não foi possível resolver este stream no Debrid:\n{}",
        erros.join("\n")
    ))
}

fn agente_http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(crate::AGENTE)
        .http_status_as_error(false)
        .build()
        .into()
}

fn form_body(pares: &[(&str, &str)]) -> String {
    pares
        .iter()
        .map(|(k, v)| format!("{}={}", codificar_segmento(k), codificar_segmento(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn http_json(
    metodo: &str,
    url: &str,
    bearer: Option<&str>,
    form: Option<&[(&str, &str)]>,
) -> Result<(u16, Value), String> {
    let ag = agente_http();
    let resp = match (metodo, bearer, form) {
        ("GET", Some(tok), _) => ag
            .get(url)
            .header("authorization", &format!("Bearer {tok}"))
            .call(),
        ("GET", None, _) => ag.get(url).call(),
        ("POST", Some(tok), Some(pares)) => ag
            .post(url)
            .header("authorization", &format!("Bearer {tok}"))
            .header("content-type", "application/x-www-form-urlencoded")
            .send(&form_body(pares)),
        ("POST", None, Some(pares)) => ag
            .post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .send(&form_body(pares)),
        ("DELETE", Some(tok), _) => ag
            .delete(url)
            .header("authorization", &format!("Bearer {tok}"))
            .call(),
        _ => return Err("Método HTTP não suportado.".into()),
    }
    .map_err(|e| format!("Erro de rede: {e}"))?;

    let status = resp.status().as_u16();
    let texto = resp
        .into_body()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_string()
        .unwrap_or_default();
    let json = serde_json::from_str(&texto).unwrap_or(Value::Null);
    Ok((status, json))
}

// MARK: - TorBox

pub fn parse_torbox_torrent(json: &Value, file_idx: Option<usize>) -> Result<(bool, String), String> {
    if !json.get("success").and_then(Value::as_bool).unwrap_or(true) {
        let msg = json
            .get("detail")
            .or_else(|| json.get("error"))
            .and_then(Value::as_str)
            .unwrap_or("erro no TorBox");
        return Err(msg.to_string());
    }
    let data = json.get("data").unwrap_or(json);
    let item = if let Some(arr) = data.as_array() {
        arr.first().ok_or("Torrent não encontrado no TorBox.")?
    } else {
        data
    };
    let ready = item
        .get("download_finished")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || item
            .get("download_present")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let files = item
        .get("files")
        .and_then(Value::as_array)
        .ok_or("Lista de arquivos ausente no TorBox.")?;
    let lista: Vec<ArquivoTorrent> = files
        .iter()
        .filter_map(|f| {
            let id = f.get("id").and_then(Value::as_u64)?.to_string();
            let nome = f
                .get("short_name")
                .or_else(|| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let bytes = f.get("size").and_then(Value::as_u64).unwrap_or(0);
            Some(ArquivoTorrent { id_interno: id, nome, bytes })
        })
        .collect();
    let idx = selecionar_arquivo(&lista, file_idx)
        .ok_or("Nenhum arquivo de vídeo encontrado no torrent.")?;
    Ok((ready, lista[idx].id_interno.clone()))
}

fn resolver_torbox(key: &str, magnet: &str, file_idx: Option<usize>) -> Result<String, String> {
    let (status, criado) = http_json(
        "POST",
        "https://api.torbox.app/v1/api/torrents/createtorrent",
        Some(key),
        Some(&[("magnet", magnet), ("seed", "1"), ("allow_zip", "false")]),
    )?;
    if status >= 400 || !criado.get("success").and_then(Value::as_bool).unwrap_or(false) {
        let msg = criado
            .get("detail")
            .or_else(|| criado.get("error"))
            .and_then(Value::as_str)
            .unwrap_or("Chave TorBox inválida ou limite atingido");
        return Err(msg.to_string());
    }
    let tid = criado
        .get("data")
        .and_then(|d| d.get("torrent_id"))
        .and_then(Value::as_u64)
        .ok_or("TorBox não retornou torrent_id.")?;

    let mut file_id = None;
    for _ in 0..4 {
        let url_info = format!("https://api.torbox.app/v1/api/torrents/mylist?id={tid}&bypass_cache=true");
        let (_, info) = http_json("GET", &url_info, Some(key), None)?;
        if let Ok((ready, fid)) = parse_torbox_torrent(&info, file_idx) {
            if ready {
                file_id = Some(fid);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(900));
    }
    let fid = file_id.ok_or("Torrent ainda não está pronto em cache no TorBox.")?;
    let dl_url = format!(
        "https://api.torbox.app/v1/api/torrents/requestdl?token={}&torrent_id={tid}&file_id={fid}&zip_link=false",
        codificar_segmento(key)
    );
    let (dl_status, dl_json) = http_json("GET", &dl_url, Some(key), None)?;
    if dl_status >= 400 || !dl_json.get("success").and_then(Value::as_bool).unwrap_or(false) {
        return Err("Falha ao obter link direto no TorBox.".into());
    }
    dl_json
        .get("data")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Link vazio retornado pelo TorBox.".into())
}

// MARK: - Real-Debrid

#[derive(Debug, PartialEq, Eq)]
pub struct RdSelecao {
    pub file_id: String,
    pub selecionado_index: usize,
}

pub fn parse_rd_selecao(info: &Value, file_idx: Option<usize>) -> Result<RdSelecao, String> {
    let files = info
        .get("files")
        .and_then(Value::as_array)
        .ok_or("Real-Debrid não retornou arquivos do torrent.")?;
    let lista: Vec<ArquivoTorrent> = files
        .iter()
        .filter_map(|f| {
            let id = f.get("id").and_then(Value::as_u64)?.to_string();
            let nome = f.get("path").and_then(Value::as_str).unwrap_or("").to_string();
            let bytes = f.get("bytes").and_then(Value::as_u64).unwrap_or(0);
            Some(ArquivoTorrent { id_interno: id, nome, bytes })
        })
        .collect();
    let idx = selecionar_arquivo(&lista, file_idx)
        .ok_or("Nenhum vídeo encontrado neste torrent.")?;
    Ok(RdSelecao {
        file_id: lista[idx].id_interno.clone(),
        selecionado_index: idx,
    })
}

fn resolver_real_debrid(key: &str, magnet: &str, file_idx: Option<usize>) -> Result<String, String> {
    let (st, add) = http_json(
        "POST",
        "https://api.real-debrid.com/rest/1.0/torrents/addMagnet",
        Some(key),
        Some(&[("magnet", magnet)]),
    )?;
    if st >= 400 {
        let msg = add.get("error").and_then(Value::as_str).unwrap_or("Chave Real-Debrid inválida");
        return Err(msg.to_string());
    }
    let tid = add
        .get("id")
        .and_then(Value::as_str)
        .ok_or("Real-Debrid não retornou ID do torrent.")?
        .to_string();

    let limpar = |id: &str| {
        let del = format!("https://api.real-debrid.com/rest/1.0/torrents/delete/{id}");
        let _ = http_json("DELETE", &del, Some(key), None);
    };

    let info_url = format!("https://api.real-debrid.com/rest/1.0/torrents/info/{tid}");
    let mut info = Value::Null;
    for _ in 0..4 {
        let (_, json) = http_json("GET", &info_url, Some(key), None)?;
        info = json;
        if info.get("files").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(700));
    }

    let sel = match parse_rd_selecao(&info, file_idx) {
        Ok(s) => s,
        Err(e) => {
            limpar(&tid);
            return Err(e);
        }
    };

    let sel_url = format!("https://api.real-debrid.com/rest/1.0/torrents/selectFiles/{tid}");
    let (st_sel, _) = http_json("POST", &sel_url, Some(key), Some(&[("files", &sel.file_id)]))?;
    if st_sel >= 400 {
        limpar(&tid);
        return Err("Falha ao selecionar arquivo no Real-Debrid.".into());
    }

    let mut link_escolhido = None;
    for _ in 0..4 {
        let (_, json) = http_json("GET", &info_url, Some(key), None)?;
        let status_rd = json.get("status").and_then(Value::as_str).unwrap_or("");
        if status_rd == "downloaded" {
            if let Some(links) = json.get("links").and_then(Value::as_array) {
                link_escolhido = links.first().and_then(Value::as_str).map(str::to_string);
                if link_escolhido.is_some() {
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(800));
    }
    let rd_link = match link_escolhido {
        Some(l) => l,
        None => {
            limpar(&tid);
            return Err("Torrent não está em cache imediato no Real-Debrid.".into());
        }
    };

    let (st_unr, unr) = http_json(
        "POST",
        "https://api.real-debrid.com/rest/1.0/unrestrict/link",
        Some(key),
        Some(&[("link", &rd_link)]),
    )?;
    if st_unr >= 400 {
        return Err("Falha ao gerar link direto no Real-Debrid.".into());
    }
    unr.get("download")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Resposta sem URL final no Real-Debrid.".into())
}

// MARK: - AllDebrid

pub fn parse_alldebrid_link(status_json: &Value, file_idx: Option<usize>) -> Result<Option<String>, String> {
    if status_json.get("status").and_then(Value::as_str) != Some("success") {
        let msg = status_json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("erro no AllDebrid");
        return Err(msg.to_string());
    }
    let magnets = status_json.get("data").and_then(|d| d.get("magnets"));
    let item = magnets
        .and_then(|m| if m.is_array() { m.as_array()?.first() } else { Some(m) })
        .ok_or("Resposta AllDebrid sem magnet.")?;
    let ready = item.get("ready").and_then(Value::as_bool).unwrap_or(false);
    if !ready {
        return Ok(None);
    }
    let links = item
        .get("links")
        .and_then(Value::as_array)
        .ok_or("AllDebrid pronto, mas sem links.")?;
    let lista: Vec<ArquivoTorrent> = links
        .iter()
        .filter_map(|l| {
            let link = l.get("link").and_then(Value::as_str)?.to_string();
            let nome = l.get("filename").and_then(Value::as_str).unwrap_or("").to_string();
            let bytes = l.get("size").and_then(Value::as_u64).unwrap_or(0);
            Some(ArquivoTorrent { id_interno: link, nome, bytes })
        })
        .collect();
    let idx = selecionar_arquivo(&lista, file_idx)
        .ok_or("Nenhum arquivo de vídeo encontrado no AllDebrid.")?;
    Ok(Some(lista[idx].id_interno.clone()))
}

fn resolver_all_debrid(key: &str, magnet: &str, file_idx: Option<usize>) -> Result<String, String> {
    let (_, upload) = http_json(
        "POST",
        "https://api.alldebrid.com/v4/magnet/upload?agent=SaimoPlayer",
        Some(key),
        Some(&[("magnets[]", magnet)]),
    )?;
    if upload.get("status").and_then(Value::as_str) != Some("success") {
        let msg = upload
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("Chave AllDebrid inválida");
        return Err(msg.to_string());
    }
    let id = upload
        .get("data")
        .and_then(|d| d.get("magnets"))
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|m| m.get("id"))
        .and_then(Value::as_u64)
        .ok_or("AllDebrid não retornou id do magnet.")?;

    let mut link_bruto = None;
    for _ in 0..4 {
        let id_str = id.to_string();
        let (_, st) = http_json(
            "POST",
            "https://api.alldebrid.com/v4/magnet/status?agent=SaimoPlayer",
            Some(key),
            Some(&[("id", &id_str)]),
        )?;
        match parse_alldebrid_link(&st, file_idx) {
            Ok(Some(l)) => {
                link_bruto = Some(l);
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(800)),
            Err(e) => return Err(e),
        }
    }
    let raw = link_bruto.ok_or("Torrent não está pronto em cache no AllDebrid.")?;
    let unlock_url = format!(
        "https://api.alldebrid.com/v4/link/unlock?agent=SaimoPlayer&link={}",
        codificar_segmento(&raw)
    );
    let (_, unlocked) = http_json("GET", &unlock_url, Some(key), None)?;
    unlocked
        .get("data")
        .and_then(|d| d.get("link"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Falha ao destravar link no AllDebrid.".into())
}

// MARK: - Premiumize

pub fn parse_premiumize_directdl(json: &Value, file_idx: Option<usize>) -> Result<String, String> {
    if json.get("status").and_then(Value::as_str) != Some("success") {
        let msg = json
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("erro no Premiumize");
        return Err(msg.to_string());
    }
    let content = json
        .get("content")
        .and_then(Value::as_array)
        .ok_or("Premiumize não retornou conteúdo.")?;
    let lista: Vec<ArquivoTorrent> = content
        .iter()
        .filter_map(|c| {
            let link = c
                .get("link")
                .or_else(|| c.get("stream_link"))
                .and_then(Value::as_str)?
                .to_string();
            let nome = c.get("path").and_then(Value::as_str).unwrap_or("").to_string();
            let bytes = c.get("size").and_then(Value::as_u64).unwrap_or(0);
            Some(ArquivoTorrent { id_interno: link, nome, bytes })
        })
        .collect();
    let idx = selecionar_arquivo(&lista, file_idx)
        .ok_or("Nenhum arquivo de vídeo encontrado no Premiumize.")?;
    Ok(lista[idx].id_interno.clone())
}

fn resolver_premiumize(key: &str, magnet: &str, file_idx: Option<usize>) -> Result<String, String> {
    let (_, json) = http_json(
        "POST",
        "https://www.premiumize.me/api/transfer/directdl",
        None,
        Some(&[("apikey", key), ("src", magnet)]),
    )?;
    parse_premiumize_directdl(&json, file_idx)
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_padrao_desabilita_connect_e_rejeita_infohash_sem_chave() {
        let cfg = ConfigDebrid::default();
        assert!(!cfg.sincronizar_connect);
        assert!(!cfg.tem_ativo());
        assert_eq!(AVISO_SEM_CHAVE.contains("infoHash"), true);
    }

    #[test]
    fn selecionar_arquivo_prioriza_file_idx_ou_maior_video_nao_sample() {
        let arquivos = vec![
            ArquivoTorrent { id_interno: "1".into(), nome: "Sample.mkv".into(), bytes: 50_000 },
            ArquivoTorrent { id_interno: "2".into(), nome: "Poster.jpg".into(), bytes: 9_000_000_000 },
            ArquivoTorrent { id_interno: "3".into(), nome: "Movie.1080p.mkv".into(), bytes: 4_000_000 },
        ];
        assert_eq!(selecionar_arquivo(&arquivos, None), Some(2));
        // fileIdx apontando para Poster.jpg ignora não-vídeo e pega o vídeo principal.
        assert_eq!(selecionar_arquivo(&arquivos, Some(1)), Some(2));
        // fileIdx válido em arquivo de vídeo é respeitado.
        let eps = vec![
            ArquivoTorrent { id_interno: "1".into(), nome: "Ep1.mkv".into(), bytes: 1_000 },
            ArquivoTorrent { id_interno: "2".into(), nome: "Ep2.mkv".into(), bytes: 900 },
        ];
        assert_eq!(selecionar_arquivo(&eps, Some(1)), Some(1));
    }

    #[test]
    fn parse_respostas_dos_quatro_provedores() {
        let tb = json!({
            "success": true,
            "data": {
                "download_finished": true,
                "files": [
                    {"id": 0, "short_name": "Legenda.srt", "size": 100},
                    {"id": 1, "short_name": "Filme.2160p.mkv", "size": 8000}
                ]
            }
        });
        assert_eq!(parse_torbox_torrent(&tb, None).unwrap(), (true, "1".into()));

        let rd = json!({
            "files": [
                {"id": 1, "path": "/Ep1.mp4", "bytes": 2000},
                {"id": 2, "path": "/Ep2.mp4", "bytes": 1800}
            ]
        });
        assert_eq!(parse_rd_selecao(&rd, Some(1)).unwrap().file_id, "2");

        let ad = json!({
            "status": "success",
            "data": {
                "magnets": {
                    "ready": true,
                    "links": [
                        {"link": "https://alldebrid/dl/a", "filename": "Filme.mkv", "size": 5000}
                    ]
                }
            }
        });
        assert_eq!(
            parse_alldebrid_link(&ad, None).unwrap().as_deref(),
            Some("https://alldebrid/dl/a")
        );

        let pm = json!({
            "status": "success",
            "content": [
                {"path": "Video.mkv", "size": 7000, "link": "https://pm.me/video.mkv"}
            ]
        });
        assert_eq!(
            parse_premiumize_directdl(&pm, None).unwrap(),
            "https://pm.me/video.mkv"
        );
    }

    #[test]
    fn montar_magnet_inclui_trackers() {
        let s = AddonStream {
            name: Some("Filme".into()),
            info_hash: Some("0123456789ABCDEF0123456789ABCDEF01234567".into()),
            sources: Some(vec!["tracker:udp://tracker.exemplo:6969/announce".into()]),
            ..Default::default()
        };
        let magnet = montar_magnet(&s).unwrap();
        assert!(magnet.starts_with("magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567"));
        assert!(magnet.contains("&tr=udp%3A%2F%2Ftracker.exemplo%3A6969%2Fannounce"));
    }
}
