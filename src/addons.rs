//! Protocolo de addons do Stremio: normalização de URLs, manifestos HTTP,
//! persistência em `saimo_addons_v1.json`, ponte TMDB <-> IMDb e consultas
//! paralelas de catálogos, metadados, streams e legendas.
//!
//! Nenhum addon de terceiros vem pré-instalado: o usuário começa 100% vazio
//! e adiciona manualmente (ou via LAN QR Code na porta 8999 / Connect).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Duration;

const ARQUIVO_ADDONS: &str = "saimo_addons_v1.json";
const ARQUIVO_PONTE_ID: &str = "tmdb-imdb-cache.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ResourceEntry {
    Texto(String),
    Objeto(ResourceObj),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceObj {
    pub name: String,
    #[serde(default)]
    pub types: Option<Vec<String>>,
    #[serde(default, rename = "idPrefixes")]
    pub id_prefixes: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtraDef {
    pub name: String,
    #[serde(default, rename = "isRequired")]
    pub is_required: bool,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogDef {
    #[serde(rename = "type")]
    pub tipo: String,
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub extra: Vec<ExtraDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BehaviorHints {
    #[serde(default)]
    pub configurable: bool,
    #[serde(default, rename = "configurationRequired")]
    pub configuration_required: bool,
    #[serde(default)]
    pub adult: bool,
    #[serde(default)]
    pub p2p: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default, rename = "idPrefixes")]
    pub id_prefixes: Option<Vec<String>>,
    #[serde(default)]
    pub resources: Vec<ResourceEntry>,
    #[serde(default)]
    pub catalogs: Vec<CatalogDef>,
    #[serde(default, rename = "behaviorHints")]
    pub behavior_hints: Option<BehaviorHints>,
}

impl Manifest {
    pub fn suporta_recurso(&self, recurso: &str, tipo: &str, id: Option<&str>) -> bool {
        for entrada in &self.resources {
            match entrada {
                ResourceEntry::Texto(nome) => {
                    if nome != recurso {
                        continue;
                    }
                    if !self.types.is_empty() && !self.types.iter().any(|t| t == tipo) {
                        continue;
                    }
                    if !casa_prefixo(self.id_prefixes.as_deref(), id) {
                        continue;
                    }
                    return true;
                }
                ResourceEntry::Objeto(obj) => {
                    if obj.name != recurso {
                        continue;
                    }
                    let tipos = obj.types.as_ref().unwrap_or(&self.types);
                    if !tipos.is_empty() && !tipos.iter().any(|t| t == tipo) {
                        continue;
                    }
                    let prefixos = obj.id_prefixes.as_deref().or(self.id_prefixes.as_deref());
                    if !casa_prefixo(prefixos, id) {
                        continue;
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn nomes_recursos(&self) -> Vec<String> {
        self.resources
            .iter()
            .map(|r| match r {
                ResourceEntry::Texto(s) => s.clone(),
                ResourceEntry::Objeto(o) => o.name.clone(),
            })
            .collect()
    }
}

fn casa_prefixo(prefixos: Option<&[String]>, id: Option<&str>) -> bool {
    let Some(id) = id else { return true };
    let Some(lista) = prefixos else { return true };
    if lista.is_empty() {
        return true;
    }
    lista.iter().any(|p| id.starts_with(p.as_str()))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddonInstalado {
    #[serde(rename = "transportUrl")]
    pub transport_url: String,
    #[serde(default = "verdadeiro")]
    pub enabled: bool,
    #[serde(default)]
    pub order: usize,
    pub manifest: Manifest,
}

fn verdadeiro() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct ArquivoAddons {
    #[serde(default = "versao_1")]
    version: u32,
    #[serde(default)]
    addons: Vec<AddonInstalado>,
}

fn versao_1() -> u32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProxyHeaders {
    #[serde(default)]
    pub request: HashMap<String, String>,
    #[serde(default)]
    pub response: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct StreamBehaviorHints {
    #[serde(default, rename = "notWebReady")]
    pub not_web_ready: Option<bool>,
    #[serde(default, rename = "bingeGroup")]
    pub binge_group: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default, rename = "videoSize")]
    pub video_size: Option<u64>,
    #[serde(default, rename = "videoHash")]
    pub video_hash: Option<String>,
    #[serde(default, rename = "proxyHeaders")]
    pub proxy_headers: Option<ProxyHeaders>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamSubtitle {
    #[serde(default)]
    pub id: Option<String>,
    pub url: String,
    #[serde(default)]
    pub lang: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AddonStream {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default, rename = "infoHash")]
    pub info_hash: Option<String>,
    #[serde(default, rename = "fileIdx")]
    pub file_idx: Option<usize>,
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    #[serde(default, rename = "ytId")]
    pub yt_id: Option<String>,
    #[serde(default, rename = "externalUrl")]
    pub external_url: Option<String>,
    #[serde(default)]
    pub subtitles: Vec<StreamSubtitle>,
    #[serde(default, rename = "behaviorHints")]
    pub behavior_hints: StreamBehaviorHints,
    #[serde(default, rename = "clientResolve")]
    pub client_resolve: Option<Value>,
    #[serde(skip)]
    pub addon_id: String,
    #[serde(skip)]
    pub addon_name: String,
}

impl AddonStream {
    pub fn e_torrent(&self) -> bool {
        self.url.as_deref().map(str::trim).filter(|u| !u.is_empty()).is_none()
            && (self.info_hash.as_deref().map(str::trim).filter(|h| !h.is_empty()).is_some()
                || self.client_resolve.is_some())
    }

    pub fn e_reproduzivel(&self) -> bool {
        self.url.as_deref().map(str::trim).filter(|u| !u.is_empty()).is_some() || self.e_torrent()
    }

    pub fn cabecalhos_requisicao(&self) -> Vec<(String, String)> {
        let Some(ref proxy) = self.behavior_hints.proxy_headers else {
            return Vec::new();
        };
        let mut pares: Vec<(String, String)> = proxy
            .request
            .iter()
            .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        pares.sort_by(|a, b| a.0.cmp(&b.0));
        pares
    }

    pub fn texto_principal(&self) -> String {
        let titulo = self
            .title
            .as_deref()
            .or(self.description.as_deref())
            .or(self.name.as_deref())
            .unwrap_or("Stream")
            .trim();
        let primeira = titulo.lines().next().unwrap_or(titulo).trim();
        let cab = self.name.as_deref().map(str::trim).filter(|n| !n.is_empty() && *n != primeira);
        let base = match cab {
            Some(c) => format!("{c} · {primeira}"),
            None => primeira.to_string(),
        };
        if self.e_torrent() {
            format!("[Torrent / Debrid] {base}")
        } else {
            base
        }
    }

    pub fn detalhe_completo(&self) -> String {
        let mut partes = Vec::new();
        if !self.addon_name.is_empty() {
            partes.push(self.addon_name.clone());
        }
        if let Some(ref n) = self.name {
            let n = n.replace('\n', " · ");
            if !n.trim().is_empty() {
                partes.push(n.trim().to_string());
            }
        }
        if let Some(ref desc) = self.title.as_ref().or(self.description.as_ref()) {
            let clean = desc.replace('\n', " · ");
            if !clean.trim().is_empty() {
                partes.push(clean.trim().to_string());
            }
        }
        if let Some(ref bg) = self.behavior_hints.binge_group {
            if !bg.is_empty() {
                partes.push(format!("Grupo: {bg}"));
            }
        }
        partes.join(" — ")
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Default)]
pub struct MetaPreview {
    pub id: String,
    #[serde(rename = "type", default)]
    pub tipo: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub poster: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "releaseInfo")]
    pub release_info: Option<Value>,
    #[serde(default, rename = "imdbRating")]
    pub imdb_rating: Option<String>,
    #[serde(skip)]
    pub addon_url: String,
    #[serde(skip)]
    pub addon_name: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Default)]
pub struct MetaVideo {
    pub id: String,
    #[serde(default, alias = "name")]
    pub title: String,
    #[serde(default)]
    pub season: u32,
    #[serde(default, alias = "number")]
    pub episode: u32,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub released: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Default)]
pub struct MetaItem {
    pub id: String,
    #[serde(rename = "type", default)]
    pub tipo: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub poster: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default, rename = "releaseInfo")]
    pub release_info: Option<Value>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub cast: Vec<String>,
    #[serde(default)]
    pub videos: Vec<MetaVideo>,
}

#[derive(Clone, Debug)]
pub struct ItemTmdb {
    pub id: u32,
    pub serie: bool,
    pub titulo: String,
    pub nota: f32,
    pub poster: Option<String>,
    pub ano: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct PonteCache {
    #[serde(default)]
    tmdb_para_imdb: HashMap<String, String>,
    #[serde(default)]
    imdb_para_tmdb: HashMap<String, (u32, bool)>,
}

static STORE: OnceLock<RwLock<Vec<AddonInstalado>>> = OnceLock::new();
static PONTE: OnceLock<Mutex<PonteCache>> = OnceLock::new();

fn arquivo_addons() -> PathBuf {
    crate::catalogo::pasta().join(ARQUIVO_ADDONS)
}

fn arquivo_ponte() -> PathBuf {
    crate::catalogo::pasta().join(ARQUIVO_PONTE_ID)
}

fn store() -> &'static RwLock<Vec<AddonInstalado>> {
    STORE.get_or_init(|| RwLock::new(carregar_em(&arquivo_addons())))
}

fn ponte() -> &'static Mutex<PonteCache> {
    PONTE.get_or_init(|| {
        let carregado = std::fs::read_to_string(arquivo_ponte())
            .ok()
            .and_then(|t| serde_json::from_str::<PonteCache>(&t).ok())
            .unwrap_or_default();
        Mutex::new(carregado)
    })
}

/// Normaliza uma URL de transporte Stremio preservando `?query` e config de path.
pub fn normalizar_url(bruta: &str) -> Result<String, String> {
    let limpa = bruta.trim();
    if limpa.is_empty() {
        return Err("Informe a URL do addon.".into());
    }
    let sem_frag = limpa.split('#').next().unwrap_or(limpa).trim();
    let convertida = if sem_frag.len() >= 10 && sem_frag[..10].eq_ignore_ascii_case("stremio://") {
        format!("https://{}", &sem_frag[10..])
    } else {
        sem_frag.to_string()
    };
    let lower = convertida.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("URL inválida: use http://, https:// ou stremio://.".into());
    }
    let (caminho_raw, query_opt) = match convertida.split_once('?') {
        Some((p, q)) => (p, Some(q.trim())),
        None => (convertida.as_str(), None),
    };

    let mut base = caminho_raw.trim_end_matches('/').to_string();
    loop {
        let suf = "/manifest.json";
        if base.len() > suf.len() && base[base.len() - suf.len()..].eq_ignore_ascii_case(suf) {
            base.truncate(base.len() - suf.len());
            base = base.trim_end_matches('/').to_string();
        } else {
            break;
        }
    }

    let depois_esquema = base.split_once("://").map(|(_, resto)| resto).unwrap_or("");
    if depois_esquema.is_empty() || depois_esquema.starts_with('/') {
        return Err("URL do addon sem host válido.".into());
    }

    match query_opt {
        Some(q) if !q.is_empty() => Ok(format!("{base}?{q}")),
        _ => Ok(base),
    }
}

pub fn split_base_query(normalizada: &str) -> (&str, Option<&str>) {
    match normalizada.split_once('?') {
        Some((b, q)) if !q.is_empty() => (b.trim_end_matches('/'), Some(q)),
        Some((b, _)) => (b.trim_end_matches('/'), None),
        None => (normalizada.trim_end_matches('/'), None),
    }
}

pub fn url_manifest(normalizada: &str) -> String {
    let (base, query) = split_base_query(normalizada);
    match query {
        Some(q) => format!("{base}/manifest.json?{q}"),
        None => format!("{base}/manifest.json"),
    }
}

/// Codificação percent-encoding RFC 3986 para segmentos de rota.
pub fn codificar_segmento(valor: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(valor.len());
    for byte in valor.as_bytes() {
        let ok = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b':');
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

pub fn formatar_extra(pares: &[(&str, &str)]) -> String {
    pares
        .iter()
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .map(|(k, v)| format!("{}={}", codificar_segmento(k), codificar_segmento(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn construir_url_recurso(normalizada: &str, segmentos: &[&str]) -> String {
    let (base, query) = split_base_query(normalizada);
    let encoded: Vec<String> = segmentos.iter().map(|s| codificar_segmento(s)).collect();
    let caminho = format!("{base}/{}.json", encoded.join("/"));
    match query {
        Some(q) => format!("{caminho}?{q}"),
        None => caminho,
    }
}

pub fn url_catalog(normalizada: &str, tipo: &str, id: &str, extra: Option<&str>) -> String {
    match extra.filter(|e| !e.is_empty()) {
        Some(ex) => {
            let (base, query) = split_base_query(normalizada);
            let t = codificar_segmento(tipo);
            let cid = codificar_segmento(id);
            let caminho = format!("{base}/catalog/{t}/{cid}/{ex}.json");
            match query {
                Some(q) => format!("{caminho}?{q}"),
                None => caminho,
            }
        }
        None => construir_url_recurso(normalizada, &["catalog", tipo, id]),
    }
}

pub fn url_meta(normalizada: &str, tipo: &str, id: &str) -> String {
    construir_url_recurso(normalizada, &["meta", tipo, id])
}

pub fn url_stream(normalizada: &str, tipo: &str, id: &str) -> String {
    construir_url_recurso(normalizada, &["stream", tipo, id])
}

pub fn url_subtitles(normalizada: &str, tipo: &str, id: &str, extra: Option<&str>) -> String {
    match extra.filter(|e| !e.is_empty()) {
        Some(ex) => {
            let (base, query) = split_base_query(normalizada);
            let t = codificar_segmento(tipo);
            let sid = codificar_segmento(id);
            let caminho = format!("{base}/subtitles/{t}/{sid}/{ex}.json");
            match query {
                Some(q) => format!("{caminho}?{q}"),
                None => caminho,
            }
        }
        None => construir_url_recurso(normalizada, &["subtitles", tipo, id]),
    }
}

/// Faz o parse do JSON de manifesto e rejeita manifests de plugins JS do Nuvio.
pub fn parsear_manifesto(json: &Value) -> Result<Manifest, String> {
    if let Some(obj) = json.as_object() {
        let eh_nuvio_js = obj.get("scrapers").is_some() && obj.get("resources").is_none();
        if eh_nuvio_js {
            let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("Nuvio Plugin").to_string();
            let id = obj.get("id").and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("nuvio.{}", name.to_lowercase().replace(' ', "-")));
            let version = obj.get("version").and_then(|v| v.as_str()).unwrap_or("1.0.0").to_string();
            let scrapers_count = obj.get("scrapers").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            let description = obj.get("description").and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Repositório Nuvio ({scrapers_count} scrapers)"));
            let logo = obj.get("logo").and_then(|v| v.as_str()).map(|s| s.to_string());
            return Ok(Manifest {
                id,
                name,
                version,
                description: Some(description),
                logo,
                types: vec!["movie".into(), "series".into()],
                id_prefixes: Some(vec!["tt".into(), "tmdb:".into()]),
                resources: vec![ResourceEntry::Texto("stream".to_string())],
                catalogs: vec![],
                behavior_hints: None,
            });
        }
    } else if json.as_array().is_some_and(|arr| {
        arr.iter()
            .any(|v| v.get("scrapers").is_some() || v.get("filename").is_some())
    }) {
        return Err(
            "Plugins JS do Nuvio não são suportados. Adicione apenas URLs de addons HTTP no padrão Stremio (/manifest.json)."
                .into(),
        );
    }

    let manifest: Manifest = serde_json::from_value(json.clone())
        .map_err(|e| format!("Manifesto Stremio inválido: {e}"))?;
    if manifest.id.trim().is_empty() || manifest.name.trim().is_empty() {
        return Err("Manifesto Stremio inválido: campos 'id' e 'name' são obrigatórios.".into());
    }
    if manifest.resources.is_empty() && manifest.catalogs.is_empty() {
        return Err("Manifesto sem recursos ('resources') ou catálogos ('catalogs').".into());
    }
    Ok(manifest)
}

pub fn carregar_em(caminho: &Path) -> Vec<AddonInstalado> {
    let Ok(texto) = std::fs::read_to_string(caminho) else {
        return Vec::new();
    };
    let mut lista = serde_json::from_str::<ArquivoAddons>(&texto)
        .map(|a| a.addons)
        .unwrap_or_default();
    lista.sort_by_key(|a| a.order);
    lista
}

pub fn salvar_em(caminho: &Path, lista: &[AddonInstalado]) -> Result<(), String> {
    if let Some(pai) = caminho.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    let corpo = serde_json::to_string_pretty(&ArquivoAddons {
        version: 1,
        addons: lista.to_vec(),
    })
    .map_err(|e| format!("Falha ao serializar addons: {e}"))?;
    std::fs::write(caminho, corpo).map_err(|e| format!("Falha ao salvar addons: {e}"))
}

pub fn todos() -> Vec<AddonInstalado> {
    store().read().map(|g| g.clone()).unwrap_or_default()
}

pub fn ativos() -> Vec<AddonInstalado> {
    todos().into_iter().filter(|a| a.enabled).collect()
}

pub fn tem_ativo() -> bool {
    store()
        .read()
        .map(|g| g.iter().any(|a| a.enabled))
        .unwrap_or(false)
}

pub fn adicionar_manifesto_em(
    caminho: &Path,
    transport_url: &str,
    manifest: Manifest,
) -> AddonInstalado {
    let mut lista = carregar_em(caminho);
    if let Some(existente) = lista.iter_mut().find(|a| a.transport_url == transport_url) {
        existente.manifest = manifest;
        existente.enabled = true;
        let out = existente.clone();
        let _ = salvar_em(caminho, &lista);
        return out;
    }
    let ordem = lista.len();
    let novo = AddonInstalado {
        transport_url: transport_url.to_string(),
        enabled: true,
        order: ordem,
        manifest,
    };
    lista.push(novo.clone());
    let _ = salvar_em(caminho, &lista);
    novo
}

fn buscar_json_http(url: &str) -> Result<Value, String> {
    let agente: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .user_agent(crate::AGENTE)
        .http_status_as_error(false)
        .build()
        .into();
    let resp = agente
        .get(url)
        .call()
        .map_err(|e| format!("Não foi possível conectar ao addon: {e}"))?;
    let status = resp.status().as_u16();
    let texto = resp
        .into_body()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Falha ao ler resposta do addon: {e}"))?;
    if status >= 400 {
        return Err(format!("O addon respondeu HTTP {status}."));
    }
    serde_json::from_str(&texto).map_err(|_| "Resposta não é um JSON válido.".into())
}

/// Normaliza a URL, baixa `/manifest.json`, valida e salva em disco.
pub fn adicionar_url(bruta: &str) -> Result<AddonInstalado, String> {
    let transport_url = normalizar_url(bruta)?;
    let manifest_url = url_manifest(&transport_url);
    let json = buscar_json_http(&manifest_url)?;
    let manifest = parsear_manifesto(&json)?;
    if manifest
        .behavior_hints
        .as_ref()
        .is_some_and(|h| h.configuration_required)
    {
        return Err("Este addon exige configuração prévia no site antes de instalar.".into());
    }
    let instalado = adicionar_manifesto_em(&arquivo_addons(), &transport_url, manifest);
    if let Ok(mut g) = store().write() {
        *g = carregar_em(&arquivo_addons());
    }
    Ok(instalado)
}

pub fn alternar(transport_url: &str, enabled: bool) -> bool {
    let caminho = arquivo_addons();
    let Ok(mut g) = store().write() else { return false };
    let Some(item) = g.iter_mut().find(|a| a.transport_url == transport_url) else {
        return false;
    };
    item.enabled = enabled;
    let _ = salvar_em(&caminho, &g);
    true
}

pub fn mover(transport_url: &str, delta: isize) -> bool {
    let caminho = arquivo_addons();
    let Ok(mut g) = store().write() else { return false };
    let Some(pos) = g.iter().position(|a| a.transport_url == transport_url) else {
        return false;
    };
    let novo = (pos as isize + delta).clamp(0, g.len().saturating_sub(1) as isize) as usize;
    if novo == pos {
        return false;
    }
    g.swap(pos, novo);
    for (idx, item) in g.iter_mut().enumerate() {
        item.order = idx;
    }
    let _ = salvar_em(&caminho, &g);
    true
}

pub fn remover(transport_url: &str) -> bool {
    let caminho = arquivo_addons();
    let Ok(mut g) = store().write() else { return false };
    let antes = g.len();
    g.retain(|a| a.transport_url != transport_url);
    if g.len() == antes {
        return false;
    }
    for (idx, item) in g.iter_mut().enumerate() {
        item.order = idx;
    }
    let _ = salvar_em(&caminho, &g);
    true
}

/// Serializa a lista para o pacote `{t:"dados"}` do Saimo Connect.
pub fn para_connect_de(lista: &[AddonInstalado]) -> Vec<Value> {
    lista
        .iter()
        .map(|a| {
            json!({
                "transportUrl": a.transport_url,
                "enabled": a.enabled,
                "order": a.order,
                "cachedId": a.manifest.id,
                "cachedName": a.manifest.name,
                "cachedVersion": a.manifest.version,
                "types": a.manifest.types,
                "resources": a.manifest.resources,
                "idPrefixes": a.manifest.id_prefixes,
                "catalogs": a.manifest.catalogs,
            })
        })
        .collect()
}

pub fn para_connect() -> Vec<Value> {
    para_connect_de(&todos())
}

/// Aplica lista recebida do `{t:"dados"}` no caminho dado.
pub fn aplicar_do_connect_em(caminho: &Path, entrada: &[Value]) -> Vec<AddonInstalado> {
    let existentes = carregar_em(caminho);
    let mut por_url: HashMap<String, AddonInstalado> = existentes
        .into_iter()
        .map(|a| (a.transport_url.clone(), a))
        .collect();

    let mut nova = Vec::with_capacity(entrada.len());
    for (idx, item) in entrada.iter().enumerate() {
        let Some(raw_url) = item.get("transportUrl").and_then(Value::as_str) else {
            continue;
        };
        let Ok(url) = normalizar_url(raw_url) else { continue };
        let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let order = item
            .get("order")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(idx);

        let manifest = if let Some(antigo) = por_url.remove(&url) {
            antigo.manifest
        } else {
            let types: Vec<String> = item
                .get("types")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let resources: Vec<ResourceEntry> = item
                .get("resources")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_else(|| vec![ResourceEntry::Texto("stream".into())]);
            let id_prefixes: Option<Vec<String>> = item
                .get("idPrefixes")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let catalogs: Vec<CatalogDef> = item
                .get("catalogs")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            Manifest {
                id: item
                    .get("cachedId")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&url)
                    .to_string(),
                name: item
                    .get("cachedName")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Addon")
                    .to_string(),
                version: item
                    .get("cachedVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("1.0.0")
                    .to_string(),
                description: None,
                logo: None,
                types,
                id_prefixes,
                resources,
                catalogs,
                behavior_hints: None,
            }
        };
        nova.push(AddonInstalado {
            transport_url: url,
            enabled,
            order,
            manifest,
        });
    }
    nova.sort_by_key(|a| a.order);
    for (idx, item) in nova.iter_mut().enumerate() {
        item.order = idx;
    }
    let _ = salvar_em(caminho, &nova);
    nova
}

pub fn aplicar_do_connect(entrada: &[Value]) {
    let caminho = arquivo_addons();
    let nova = aplicar_do_connect_em(&caminho, entrada);
    if let Ok(mut g) = store().write() {
        *g = nova;
    }
}

/// Faz o parse de uma resposta `/stream/...json` atribuindo origem do addon.
pub fn parsear_streams(json: &Value, addon_id: &str, addon_name: &str) -> Vec<AddonStream> {
    let Some(arr) = json.get("streams").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut saida = Vec::with_capacity(arr.len());
    for item in arr {
        let Ok(mut stream) = serde_json::from_value::<AddonStream>(item.clone()) else {
            continue;
        };
        if !stream.e_reproduzivel() {
            continue;
        }
        stream.addon_id = addon_id.to_string();
        stream.addon_name = addon_name.to_string();
        saida.push(stream);
    }
    saida
}

/// Seleciona stream compatível com `bingeGroup` ao avançar de episódio.
pub fn selecionar_por_binge_group<'a>(
    streams: &'a [AddonStream],
    binge_group: &str,
    addon_preferido: Option<&str>,
) -> Option<&'a AddonStream> {
    if let Some(pref) = addon_preferido {
        if let Some(exato) = streams.iter().find(|s| {
            s.addon_id == pref && s.behavior_hints.binge_group.as_deref() == Some(binge_group)
        }) {
            return Some(exato);
        }
    }
    streams
        .iter()
        .find(|s| s.behavior_hints.binge_group.as_deref() == Some(binge_group))
}

/// Resolve e memoriza `TMDB ID -> IMDb ID (tt...)` usando `/external_ids`.
pub fn tmdb_para_imdb(tmdb_id: u32, serie: bool) -> Option<String> {
    let chave = format!("{}:{tmdb_id}", if serie { "tv" } else { "movie" });
    if let Ok(c) = ponte().lock() {
        if let Some(imdb) = c.tmdb_para_imdb.get(&chave) {
            return Some(imdb.clone());
        }
    }
    let tipo = if serie { "tv" } else { "movie" };
    let url = format!(
        "{}/{tipo}/{tmdb_id}/external_ids?api_key={}",
        crate::ficha::BASE,
        crate::ficha::CHAVE
    );
    let json = crate::rede::json_curto(&url)?;
    let imdb = json
        .get("imdb_id")
        .and_then(Value::as_str)
        .filter(|s| s.starts_with("tt"))?
        .to_string();
    if let Ok(mut c) = ponte().lock() {
        c.tmdb_para_imdb.insert(chave, imdb.clone());
        c.imdb_para_tmdb.insert(imdb.clone(), (tmdb_id, serie));
        if let Ok(texto) = serde_json::to_string(&*c) {
            let _ = std::fs::write(arquivo_ponte(), texto);
        }
    }
    Some(imdb)
}

/// Resolve `IMDb ID (tt...) -> (tmdb_id, serie)` via `/find/{id}`.
pub fn imdb_para_tmdb(imdb_id: &str, serie_hint: Option<bool>) -> Option<(u32, bool)> {
    if !imdb_id.starts_with("tt") {
        return None;
    }
    if let Ok(c) = ponte().lock() {
        if let Some(&par) = c.imdb_para_tmdb.get(imdb_id) {
            return Some(par);
        }
    }
    let url = format!(
        "{}/find/{imdb_id}?api_key={}&external_source=imdb_id&language=pt-BR",
        crate::ficha::BASE,
        crate::ficha::CHAVE
    );
    let json = crate::rede::json_curto(&url)?;
    let pick = |campo: &str| {
        json.get(campo)
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|v| v.get("id"))
            .and_then(Value::as_u64)
            .map(|id| id as u32)
    };
    let resultado = match serie_hint {
        Some(true) => pick("tv_results").map(|id| (id, true)).or_else(|| pick("movie_results").map(|id| (id, false))),
        Some(false) => pick("movie_results").map(|id| (id, false)).or_else(|| pick("tv_results").map(|id| (id, true))),
        None => pick("movie_results").map(|id| (id, false)).or_else(|| pick("tv_results").map(|id| (id, true))),
    }?;
    if let Ok(mut c) = ponte().lock() {
        let chave = format!("{}:{}", if resultado.1 { "tv" } else { "movie" }, resultado.0);
        c.tmdb_para_imdb.insert(chave, imdb_id.to_string());
        c.imdb_para_tmdb.insert(imdb_id.to_string(), resultado);
        if let Ok(texto) = serde_json::to_string(&*c) {
            let _ = std::fs::write(arquivo_ponte(), texto);
        }
    }
    Some(resultado)
}

/// Monta os IDs de consulta para um addon respeitando `tt...` e `tmdb:...`.
pub fn ids_para_addon(
    manifest: &Manifest,
    recurso: &str,
    tipo: &str,
    tmdb_id: Option<u32>,
    imdb_id: Option<&str>,
    extra_id: Option<&str>,
    temporada: Option<u32>,
    episodio: Option<u32>,
) -> Vec<String> {
    let mut candidatos = Vec::new();
    let formatar = |base: &str| -> String {
        if tipo == "series" {
            if let (Some(s), Some(e)) = (temporada, episodio) {
                if recurso != "meta" {
                    return format!("{base}:{s}:{e}");
                }
            }
        }
        base.to_string()
    };

    if let Some(imdb) = imdb_id.filter(|s| !s.is_empty()) {
        let id = formatar(imdb);
        if manifest.suporta_recurso(recurso, tipo, Some(&id)) {
            candidatos.push(id);
        }
    }
    if let Some(tmdb) = tmdb_id {
        let id = formatar(&format!("tmdb:{tmdb}"));
        if !candidatos.contains(&id) && manifest.suporta_recurso(recurso, tipo, Some(&id)) {
            candidatos.push(id);
        }
    }
    if let Some(raw) = extra_id.filter(|s| !s.is_empty()) {
        let id = if raw.contains(':') && tipo == "series" && raw.matches(':').count() >= 2 {
            raw.to_string()
        } else {
            formatar(raw)
        };
        if !candidatos.contains(&id) && manifest.suporta_recurso(recurso, tipo, Some(&id)) {
            candidatos.push(id);
        }
    }
    candidatos
}

/// Consulta streams em paralelo em todos os addons ativos que suportam o título.
pub fn buscar_streams(
    tipo: &str,
    tmdb_id: Option<u32>,
    imdb_id: Option<&str>,
    extra_id: Option<&str>,
    temporada: Option<u32>,
    episodio: Option<u32>,
) -> Vec<AddonStream> {
    let mut resolved_imdb = imdb_id.map(str::to_string);
    if resolved_imdb.is_none() {
        if let Some(tid) = tmdb_id {
            resolved_imdb = tmdb_para_imdb(tid, tipo == "series");
        }
    }
    let instalados = ativos();
    let mut lotes: Vec<Vec<AddonStream>> = std::thread::scope(|escopo| {
        let tarefas: Vec<_> = instalados
            .iter()
            .map(|addon| {
                let imdb_ref = resolved_imdb.as_deref();
                escopo.spawn(move || {
                    let ids = ids_para_addon(
                        &addon.manifest,
                        "stream",
                        tipo,
                        tmdb_id,
                        imdb_ref,
                        extra_id,
                        temporada,
                        episodio,
                    );
                    let mut encontrados = Vec::new();
                    for id in ids {
                        let url = url_stream(&addon.transport_url, tipo, &id);
                        if let Ok(json) = buscar_json_http(&url) {
                            let mut streams = parsear_streams(&json, &addon.manifest.id, &addon.manifest.name);
                            encontrados.append(&mut streams);
                            if !encontrados.is_empty() {
                                break;
                            }
                        }
                    }
                    encontrados
                })
            })
            .collect();
        tarefas.into_iter().map(|t| t.join().unwrap_or_default()).collect()
    });
    let mut todos_streams = Vec::new();
    for lote in &mut lotes {
        todos_streams.append(lote);
    }
    todos_streams
}

/// Busca legendas externas nos addons ativos com recurso `"subtitles"`.
pub fn buscar_subtitles(
    tipo: &str,
    tmdb_id: Option<u32>,
    imdb_id: Option<&str>,
    temporada: Option<u32>,
    episodio: Option<u32>,
    video_hash: Option<&str>,
    video_size: Option<u64>,
    filename: Option<&str>,
) -> Vec<StreamSubtitle> {
    let mut resolved_imdb = imdb_id.map(str::to_string);
    if resolved_imdb.is_none() {
        if let Some(tid) = tmdb_id {
            resolved_imdb = tmdb_para_imdb(tid, tipo == "series");
        }
    }
    let size_str = video_size.map(|n| n.to_string());
    let mut pares_extra: Vec<(&str, &str)> = Vec::new();
    if let Some(h) = video_hash.filter(|s| !s.is_empty()) {
        pares_extra.push(("videoHash", h));
    }
    if let Some(sz) = size_str.as_deref() {
        pares_extra.push(("videoSize", sz));
    }
    if let Some(fnm) = filename.filter(|s| !s.is_empty()) {
        pares_extra.push(("filename", fnm));
    }
    let extra = formatar_extra(&pares_extra);

    let instalados = ativos();
    let lotes: Vec<Vec<StreamSubtitle>> = std::thread::scope(|escopo| {
        let tarefas: Vec<_> = instalados
            .iter()
            .map(|addon| {
                let imdb_ref = resolved_imdb.as_deref();
                let extra_ref = if extra.is_empty() { None } else { Some(extra.as_str()) };
                escopo.spawn(move || {
                    let ids = ids_para_addon(
                        &addon.manifest,
                        "subtitles",
                        tipo,
                        tmdb_id,
                        imdb_ref,
                        None,
                        temporada,
                        episodio,
                    );
                    let mut out = Vec::new();
                    for id in ids {
                        let url = url_subtitles(&addon.transport_url, tipo, &id, extra_ref);
                        if let Ok(json) = buscar_json_http(&url) {
                            if let Some(arr) = json.get("subtitles").and_then(Value::as_array) {
                                for item in arr {
                                    if let Ok(sub) = serde_json::from_value::<StreamSubtitle>(item.clone()) {
                                        if !sub.url.trim().is_empty() {
                                            out.push(sub);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        tarefas.into_iter().map(|t| t.join().unwrap_or_default()).collect()
    });
    lotes.into_iter().flatten().collect()
}

pub fn buscar_catalogo(
    addon: &AddonInstalado,
    catalog: &CatalogDef,
    busca: Option<&str>,
) -> Vec<MetaPreview> {
    let extra = busca
        .filter(|q| !q.trim().is_empty())
        .map(|q| formatar_extra(&[("search", q.trim())]));
    let url = url_catalog(&addon.transport_url, &catalog.tipo, &catalog.id, extra.as_deref());
    let Ok(json) = buscar_json_http(&url) else {
        return Vec::new();
    };
    let Some(arr) = json.get("metas").and_then(Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| serde_json::from_value::<MetaPreview>(v.clone()).ok())
        .map(|mut m| {
            if m.tipo.is_empty() {
                m.tipo = catalog.tipo.clone();
            }
            m.addon_url = addon.transport_url.clone();
            m.addon_name = addon.manifest.name.clone();
            m
        })
        .collect()
}

pub fn buscar_meta(tipo: &str, id: &str) -> Option<MetaItem> {
    for addon in ativos() {
        if !addon.manifest.suporta_recurso("meta", tipo, Some(id)) {
            continue;
        }
        let url = url_meta(&addon.transport_url, tipo, id);
        if let Ok(json) = buscar_json_http(&url) {
            if let Some(meta) = json.get("meta") {
                if let Ok(parsed) = serde_json::from_value::<MetaItem>(meta.clone()) {
                    if !parsed.name.is_empty() {
                        return Some(parsed);
                    }
                }
            }
        }
    }
    None
}

/// Busca listas de descoberta no TMDB (em alta / populares / busca) quando o
/// usuário usa Saimo com Addons sem lista VOD local configurada.
pub fn descobrir_tmdb(caminho: &str) -> Vec<ItemTmdb> {
    let sep = if caminho.contains('?') { '&' } else { '?' };
    let url = format!(
        "{}/{caminho}{sep}api_key={}&language=pt-BR&region=BR",
        crate::ficha::BASE,
        crate::ficha::CHAVE
    );
    let Some(json) = crate::rede::json_curto(&url) else {
        return Vec::new();
    };
    let Some(arr) = json.get("results").and_then(Value::as_array) else {
        return Vec::new();
    };
    let infer_serie = caminho.starts_with("tv/");
    arr.iter()
        .filter_map(|v| {
            let media_type = v.get("media_type").and_then(Value::as_str);
            if media_type == Some("person") {
                return None;
            }
            let serie = match media_type {
                Some("tv") => true,
                Some("movie") => false,
                _ => infer_serie || v.get("name").is_some() && v.get("title").is_none(),
            };
            let id = v.get("id").and_then(Value::as_u64)? as u32;
            let titulo = v
                .get("title")
                .or_else(|| v.get("name"))
                .and_then(Value::as_str)?
                .trim()
                .to_string();
            let poster = v
                .get("poster_path")
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
                .map(|p| format!("{}w342{p}", crate::ficha::IMAGENS));
            let nota = v.get("vote_average").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let data = v
                .get("release_date")
                .or_else(|| v.get("first_air_date"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let ano = data.get(..4).unwrap_or("").to_string();
            Some(ItemTmdb {
                id,
                serie,
                titulo,
                nota,
                poster,
                ano,
            })
        })
        .collect()
}

/// Converte um `MetaItem` (vindo de `/meta` de addon) em `ficha::Ficha`.
pub fn ficha_de_meta(meta: &MetaItem) -> crate::ficha::Ficha {
    let duracao = meta
        .runtime
        .as_deref()
        .and_then(|r| r.split_whitespace().next())
        .and_then(|n| n.parse::<u32>().ok());
    let temporadas = meta
        .videos
        .iter()
        .map(|v| v.season)
        .filter(|&s| s > 0)
        .max();
    let ano = meta
        .release_info
        .as_ref()
        .and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())))
        .unwrap_or_default()
        .chars()
        .take(4)
        .collect();
    let elenco = meta
        .cast
        .iter()
        .take(12)
        .map(|nome| crate::ficha::Pessoa {
            id: 0,
            nome: nome.clone(),
            papel: String::new(),
            foto: None,
        })
        .collect();
    crate::ficha::Ficha {
        sinopse: meta.description.clone().unwrap_or_default(),
        frase: String::new(),
        duracao,
        classificacao: None,
        nota: 0.0,
        ano,
        generos: meta.genres.clone(),
        assinatura: String::new(),
        roteiro: String::new(),
        produtora: String::new(),
        elenco,
        capa: meta.poster.clone(),
        fundo: meta.background.clone(),
        temporadas,
        logo: meta.logo.clone(),
    }
}

/// Conta quantas temporadas tem uma série no TMDB (mínimo 1).
pub fn temporadas_por_tmdb(tmdb_id: u32) -> Vec<u32> {
    let url = format!(
        "{}/tv/{tmdb_id}?api_key={}&language=pt-BR",
        crate::ficha::BASE,
        crate::ficha::CHAVE
    );
    let Some(json) = crate::rede::json_curto(&url) else {
        return vec![1];
    };
    let mut temps: Vec<u32> = json
        .get("seasons")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("season_number").and_then(Value::as_u64).map(|n| n as u32))
                .filter(|&n| n > 0)
                .collect()
        })
        .unwrap_or_default();
    if temps.is_empty() {
        temps.push(1);
    }
    temps
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn normalizacao_preserva_query_config_path_e_stremio_scheme() {
        assert_eq!(
            normalizar_url("stremio://torrentio.strem.fun/providers=yts|realdebrid=XYZ/manifest.json?foo=1&bar=2").unwrap(),
            "https://torrentio.strem.fun/providers=yts|realdebrid=XYZ?foo=1&bar=2"
        );
        assert_eq!(
            normalizar_url("  https://v3-cinemeta.strem.io/manifest.json/  ").unwrap(),
            "https://v3-cinemeta.strem.io"
        );
        assert_eq!(
            normalizar_url("http://127.0.0.1:11470/cfg/").unwrap(),
            "http://127.0.0.1:11470/cfg"
        );
    }

    #[test]
    fn rotas_codificam_segmentos_e_reencaixam_query() {
        let base = normalizar_url("https://addon.exemplo/cfg=abc|123/manifest.json?k=v").unwrap();
        assert_eq!(
            url_stream(&base, "series", "tt0944947:1:2"),
            "https://addon.exemplo/cfg=abc|123/stream/series/tt0944947:1:2.json?k=v"
        );
        let extra = formatar_extra(&[("search", "Matrix 1999"), ("skip", "20")]);
        assert_eq!(
            url_catalog(&base, "movie", "top", Some(&extra)),
            "https://addon.exemplo/cfg=abc|123/catalog/movie/top/search=Matrix%201999&skip=20.json?k=v"
        );
    }

    #[test]
    fn detecta_e_converte_repositorio_nuvio() {
        let nuvio = json!({
            "name": "Nuvio Pack",
            "version": "1.0.0",
            "scrapers": [{"id": "abc", "name": "Foo", "filename": "foo.js"}]
        });
        let manifest = parsear_manifesto(&nuvio).expect("deve converter repositório Nuvio");
        assert_eq!(manifest.name, "Nuvio Pack");
        assert!(manifest.suporta_recurso("stream", "movie", Some("tt1234567")));
    }

    #[test]
    fn parse_de_manifesto_com_resources_string_e_objeto() {
        let raw = json!({
            "id": "org.exemplo.addon",
            "name": "Exemplo Addon",
            "version": "2.1.0",
            "types": ["movie", "series"],
            "idPrefixes": ["tt", "tmdb:"],
            "resources": [
                "catalog",
                {"name": "stream", "types": ["series"], "idPrefixes": ["tt"]},
                {"name": "subtitles"}
            ],
            "catalogs": [
                {"type": "movie", "id": "popular", "name": "Populares"}
            ]
        });
        let m = parsear_manifesto(&raw).unwrap();
        assert!(m.suporta_recurso("catalog", "movie", None));
        assert!(m.suporta_recurso("stream", "series", Some("tt12345:1:1")));
        assert!(!m.suporta_recurso("stream", "movie", Some("tt12345")));
        assert!(!m.suporta_recurso("stream", "series", Some("tmdb:999:1:1")));
        assert!(m.suporta_recurso("subtitles", "movie", Some("tmdb:55")));
    }

    #[test]
    fn parse_de_streams_binge_group_e_proxy_headers() {
        let raw = json!({
            "streams": [
                {
                    "name": "AddonX\n1080p",
                    "title": "Ep. 1 HEVC MKV",
                    "url": "https://cdn.exemplo/ep1.mkv",
                    "subtitles": [{"id": "pt", "url": "https://sub.exemplo/pt.vtt", "lang": "pob"}],
                    "behaviorHints": {
                        "notWebReady": true,
                        "bingeGroup": "grupo-hevc-1080",
                        "proxyHeaders": {
                            "request": {
                                "Referer": "https://origem.exemplo/",
                                "User-Agent": "CustomUA/1.0"
                            }
                        }
                    }
                },
                {
                    "name": "Torrent 4K",
                    "infoHash": "0123456789abcdef0123456789abcdef01234567",
                    "fileIdx": 0,
                    "behaviorHints": {"bingeGroup": "grupo-torrent-4k"}
                }
            ]
        });
        let streams = parsear_streams(&raw, "org.exemplo", "AddonX");
        assert_eq!(streams.len(), 2);
        assert!(!streams[0].e_torrent());
        assert!(streams[1].e_torrent());
        assert_eq!(streams[0].subtitles.len(), 1);
        assert_eq!(
            streams[0].cabecalhos_requisicao(),
            vec![
                ("Referer".to_string(), "https://origem.exemplo/".to_string()),
                ("User-Agent".to_string(), "CustomUA/1.0".to_string()),
            ]
        );
        let match_binge = selecionar_por_binge_group(&streams, "grupo-hevc-1080", Some("org.exemplo")).unwrap();
        assert_eq!(match_binge.url.as_deref(), Some("https://cdn.exemplo/ep1.mkv"));
    }

    #[test]
    fn persistencia_comeca_vazia_e_connect_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("saimo-addons-test-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        assert!(carregar_em(&tmp).is_empty());

        let m = parsear_manifesto(&json!({
            "id": "a.b.c",
            "name": "Teste",
            "version": "1.0.0",
            "types": ["movie"],
            "resources": ["stream"]
        }))
        .unwrap();
        let ins = adicionar_manifesto_em(&tmp, "https://a.example", m);
        assert_eq!(carregar_em(&tmp).len(), 1);

        let connect_json = para_connect_de(&[ins]);
        let aplicado = aplicar_do_connect_em(&tmp, &connect_json);
        assert_eq!(aplicado.len(), 1);
        assert_eq!(aplicado[0].transport_url, "https://a.example");
        let _ = std::fs::remove_file(&tmp);
    }
}
