//! "Minhas listas": o usuário cola o link de uma lista .m3u comum e os canais
//! entram no app, na seção "Minhas listas". Fica só neste computador.
//!
//! Três partes: o leitor de .m3u (`ler`, idêntico ao dos outros aplicativos e
//! conferido contra o gabarito), o download (`baixar_texto`) e o armazenamento
//! (`indice.json` + um `<id>.json` por lista, em `SaimoTV/minhas-listas`).
//! O link da pessoa nunca vai para log nem para a tela.

use crate::catalogo::{self, Canal, Fonte};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub const SECAO: &str = "Minhas listas";
pub const MAXIMO_DE_LISTAS: usize = 10;
const LIMITE_CANAIS: usize = 20_000;
const LIMITE_BYTES: u64 = 30 * 1024 * 1024;
const VALIDADE_S: u64 = 12 * 3600;
const AGENTE_VLC: &str = "VLC/3.0.20 LibVLC/3.0.20";

// MARK: - Leitor

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FonteM3u {
    pub url: String,
    pub referer: Option<String>,
    pub ua: Option<String>,
    #[serde(rename = "keyId")]
    pub key_id: Option<String>,
    pub key: Option<String>,
    pub qualidade: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanalM3u {
    pub nome: String,
    pub grupo: Option<String>,
    pub logo: Option<String>,
    pub fontes: Vec<FonteM3u>,
}

/// minúsculas, sem acentos, só `[a-z0-9]`.
fn chave(nome: &str) -> String {
    nome.chars().flat_map(char::to_lowercase).map(sem_acento).filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn sem_acento(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        outro => outro,
    }
}

fn atributo(linha: &str, nome: &str) -> Option<String> {
    let procurado = format!("{nome}=\"");
    let mut de = 0;
    while let Some(i) = linha[de..].find(&procurado) {
        let ini = de + i;
        let antes_ok = linha[..ini].chars().next_back().is_none_or(|c| c.is_whitespace() || c == ':');
        let resto = &linha[ini + procurado.len()..];
        if antes_ok {
            if let Some(fim) = resto.find('"') {
                return Some(resto[..fim].trim().to_string());
            }
        }
        de = ini + procurado.len();
    }
    None
}

fn nome_exibido(linha: &str) -> String {
    let mut dentro = false;
    for (i, c) in linha.char_indices() {
        if c == '"' {
            dentro = !dentro;
        } else if c == ',' && !dentro {
            return linha[i + 1..].trim().to_string();
        }
    }
    String::new()
}

fn e_palavra(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `\bpalavra\b` em texto já em minúsculas.
fn tem_palavra(texto: &str, palavra: &str) -> bool {
    let mut de = 0;
    while let Some(i) = texto[de..].find(palavra) {
        let ini = de + i;
        let fim = ini + palavra.len();
        let antes = texto[..ini].chars().next_back().is_none_or(|c| !e_palavra(c));
        let depois = texto[fim..].chars().next().is_none_or(|c| !e_palavra(c));
        if antes && depois {
            return true;
        }
        de = ini + palavra.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// A primeira resolução `\d{3,4}[pi]` do texto (como o regex a acharia).
fn resolucao(texto: &str) -> Option<String> {
    let c: Vec<char> = texto.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_ascii_digit() {
            let ini = i;
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            let n = i - ini;
            if n >= 3 && i < c.len() && matches!(c[i], 'p' | 'P' | 'i' | 'I') {
                let de = i - n.min(4);
                let mut s: String = c[de..i].iter().collect();
                s.push(c[i].to_ascii_lowercase());
                return Some(s);
            }
        } else {
            i += 1;
        }
    }
    None
}

fn tag_ruim(conteudo: &str) -> bool {
    let t = conteudo.to_lowercase();
    resolucao(&t).is_some()
        || t.contains("not 24/7")
        || t.contains("geoblocked")
        || t.contains("geo-blocked")
        || t.contains("backup")
        || t.contains("vertical")
        || tem_palavra(&t, "sd")
        || tem_palavra(&t, "hd")
        || t.contains("fhd")
        || tem_palavra(&t, "4k")
}

/// Tira do fim os "(720p)", "[Not 24/7]"...; devolve o nome e a resolução.
fn tirar_tags(nome: &str) -> (String, Option<String>) {
    let mut nome = nome.to_string();
    let mut qualidade = None;
    loop {
        let t = nome.trim_end();
        let Some(ultimo) = t.chars().next_back() else { break };
        if ultimo != ')' && ultimo != ']' {
            break;
        }
        let corpo = &t[..t.len() - 1];
        let Some((pos, abre)) = corpo.char_indices().rev().find(|(_, c)| matches!(c, '(' | '[' | ')' | ']')) else { break };
        if abre != '(' && abre != '[' {
            break;
        }
        let conteudo = &corpo[pos + 1..];
        if !tag_ruim(conteudo) {
            break;
        }
        if qualidade.is_none() {
            qualidade = resolucao(conteudo);
        }
        nome = corpo[..pos].to_string();
    }
    (nome.trim().to_string(), qualidade)
}

fn comeca_com_http(s: &str) -> bool {
    let b = s.as_bytes();
    let pre = |p: &str| b.len() >= p.len() && b[..p.len()].eq_ignore_ascii_case(p.as_bytes());
    pre("http://") || pre("https://")
}

fn nao_vazio(s: Option<&String>) -> Option<String> {
    s.filter(|s| !s.is_empty()).cloned()
}

fn chave_clearkey(tipo: &str, valor: &str) -> Option<(String, String)> {
    if !tipo.contains("clearkey") {
        return None;
    }
    let (a, b) = valor.split_once(':')?;
    let hex = |s: &str| s.len() == 32 && s.bytes().all(|c| c.is_ascii_hexdigit());
    (hex(a) && hex(b)).then(|| (a.to_lowercase(), b.to_lowercase()))
}

#[derive(Default)]
struct Pendente {
    exibido: String,
    tvg_name: Option<String>,
    tvg_id: Option<String>,
    logo: Option<String>,
    grupo: Option<String>,
}

#[derive(Default)]
struct Opcoes {
    extgrp: Option<String>,
    referer: Option<String>,
    ua: Option<String>,
    lic_tipo: String,
    lic_chave: String,
}

/// Lê um texto .m3u. Nunca falha: texto sem canais devolve lista vazia.
pub fn ler(texto: &str) -> Vec<CanalM3u> {
    let texto = texto.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let mut canais: Vec<CanalM3u> = Vec::new();
    let mut indice: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut pend: Option<Pendente> = None;
    let mut opts = Opcoes::default();
    for bruta in texto.split('\n') {
        let linha = bruta.trim();
        if linha.is_empty() {
            continue;
        }
        if linha.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("#EXTM3U")) {
            continue;
        }
        if linha.starts_with("#EXTINF") {
            pend = Some(Pendente {
                exibido: nome_exibido(linha),
                tvg_name: atributo(linha, "tvg-name"),
                tvg_id: atributo(linha, "tvg-id"),
                logo: atributo(linha, "tvg-logo"),
                grupo: atributo(linha, "group-title"),
            });
            opts = Opcoes::default();
            continue;
        }
        if let Some(g) = linha.strip_prefix("#EXTGRP:") {
            opts.extgrp = Some(g.trim().to_string());
            continue;
        }
        if let Some(kv) = linha.strip_prefix("#EXTVLCOPT:") {
            if let Some((k, v)) = kv.split_once('=') {
                match k.trim().to_lowercase().as_str() {
                    "http-referrer" | "http-referer" => opts.referer = Some(v.trim().to_string()),
                    "http-user-agent" => opts.ua = Some(v.trim().to_string()),
                    _ => {}
                }
            }
            continue;
        }
        if let Some(kv) = linha.strip_prefix("#KODIPROP:") {
            if let Some((k, v)) = kv.split_once('=') {
                match k.trim().to_lowercase().as_str() {
                    "inputstream.adaptive.license_type" => opts.lic_tipo = v.trim().to_lowercase(),
                    "inputstream.adaptive.license_key" => opts.lic_chave = v.trim().to_string(),
                    _ => {}
                }
            }
            continue;
        }
        if linha.starts_with('#') {
            continue;
        }
        // Linha de endereço.
        let mut url = linha;
        let mut extra: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if let Some((antes, depois)) = linha.split_once('|') {
            if depois.contains('=') {
                url = antes.trim();
                for par in depois.split('&') {
                    if let Some((k, v)) = par.split_once('=') {
                        extra.insert(k.trim().to_lowercase(), v.trim().to_string());
                    }
                }
            }
        }
        if !comeca_com_http(url) {
            pend = None;
            opts = Opcoes::default();
            continue;
        }
        let p = pend.take().unwrap_or_default();
        let bruto = [Some(p.exibido.clone()), p.tvg_name.clone(), p.tvg_id.clone()]
            .into_iter()
            .flatten()
            .find(|s| !s.is_empty())
            .unwrap_or_default();
        let (mut nome, qual) = if bruto.is_empty() { (String::new(), None) } else { tirar_tags(&bruto) };
        if nome.is_empty() {
            nome = format!("Canal {}", canais.len() + 1);
        }
        let mut grupo = nao_vazio(p.grupo.as_ref()).or_else(|| nao_vazio(opts.extgrp.as_ref()));
        if let Some(g) = &grupo {
            if g.contains(';') {
                grupo = Some(g.split(';').next().unwrap_or("").trim().to_string());
            }
        }
        let grupo = grupo.filter(|g| !g.is_empty());
        let logo = p.logo.filter(|l| comeca_com_http(l));
        let (key_id, key) = match chave_clearkey(&opts.lic_tipo, &opts.lic_chave) {
            Some((a, b)) => (Some(a), Some(b)),
            None => (None, None),
        };
        let fonte = FonteM3u {
            url: url.to_string(),
            referer: nao_vazio(opts.referer.as_ref()).or_else(|| nao_vazio(extra.get("referer"))),
            ua: nao_vazio(opts.ua.as_ref()).or_else(|| nao_vazio(extra.get("user-agent"))),
            key_id,
            key,
            qualidade: qual,
        };
        let k = chave(&nome);
        if let Some(&i) = indice.get(&k) {
            let c = &mut canais[i];
            if !c.fontes.iter().any(|f| f.url == fonte.url) {
                c.fontes.push(fonte);
            }
            if c.grupo.is_none() {
                c.grupo = grupo;
            }
            if c.logo.is_none() {
                c.logo = logo;
            }
        } else if canais.len() < LIMITE_CANAIS {
            indice.insert(k, canais.len());
            canais.push(CanalM3u { nome, grupo, logo, fontes: vec![fonte] });
        }
        opts = Opcoes::default();
    }
    canais
}

// MARK: - Restritos

/// Grupo ou nome de conteúdo adulto (sem diferenciar caixa nem acento).
/// "Adult Swim" é um canal comum e não conta.
pub fn e_restrito(grupo: Option<&str>, nome: &str) -> bool {
    [grupo.unwrap_or(""), nome].iter().any(|t| texto_restrito(t))
}

fn texto_restrito(texto: &str) -> bool {
    let t: String = texto.chars().flat_map(char::to_lowercase).map(sem_acento).collect();
    let t = t.replace("adult swim", " ");
    if t.contains("+18") || t.contains("18+") {
        // "+18" e "18+" soltos, não no meio de um número maior.
        let bytes: Vec<char> = t.chars().collect();
        for i in 0..bytes.len().saturating_sub(2) {
            let s: String = bytes[i..i + 3].iter().collect();
            if s == "+18" && bytes.get(i + 3).is_none_or(|c| !c.is_ascii_digit()) {
                return true;
            }
            if s == "18+" && (i == 0 || !bytes[i - 1].is_ascii_digit()) {
                return true;
            }
        }
    }
    t.split(|c: char| !c.is_alphanumeric()).any(|p| {
        matches!(
            p,
            "adult" | "adulto" | "adultos" | "xxx" | "porn" | "porno" | "sexo" | "sex" | "erotic" | "erotico" | "hentai"
        )
    })
}

// MARK: - Baixar

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Erro {
    Esquema,
    Download,
    NaoELista,
    SemCanais,
    Limite,
    Arquivo,
}

impl Erro {
    pub fn mensagem(self) -> &'static str {
        match self {
            Erro::Esquema => "O link precisa começar com http:// ou https://",
            Erro::Download => "Não consegui baixar essa lista",
            Erro::NaoELista => "Isso não parece uma lista .m3u",
            Erro::SemCanais => "Essa lista não tem canais",
            Erro::Limite => "Você já tem 10 listas. Remova uma para adicionar outra",
            Erro::Arquivo => "Não consegui ler esse arquivo",
        }
    }
}

/// Aceita a resposta se começa (depois do BOM e dos espaços) com `#EXTM3U` ou tem algum `#EXTINF`.
fn parece_lista(texto: &str) -> bool {
    let t = texto.trim_start_matches('\u{feff}').trim_start();
    t.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("#EXTM3U")) || t.contains("#EXTINF")
}

fn baixar_texto(link: &str) -> Result<String, Erro> {
    if !comeca_com_http(link) {
        return Err(Erro::Esquema);
    }
    let agente: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(AGENTE_VLC)
        .build()
        .into();
    let resposta = agente.get(link).call().map_err(|_| Erro::Download)?;
    let mut bytes = Vec::new();
    resposta
        .into_body()
        .into_reader()
        .take(LIMITE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Erro::Download)?;
    if bytes.len() as u64 > LIMITE_BYTES {
        return Err(Erro::Download);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Lê o arquivo da pessoa: até 30 MB, UTF-8 (byte inválido vira "�" em vez de falhar).
pub fn ler_arquivo(caminho: &Path) -> Result<String, Erro> {
    let mut f = std::fs::File::open(caminho).map_err(|_| Erro::Arquivo)?;
    let mut bytes = Vec::new();
    (&mut f).take(LIMITE_BYTES + 1).read_to_end(&mut bytes).map_err(|_| Erro::Arquivo)?;
    if bytes.len() as u64 > LIMITE_BYTES {
        return Err(Erro::Arquivo);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Texto baixado -> canais, com os erros da especificação.
pub fn interpretar(texto: &str) -> Result<Vec<CanalM3u>, Erro> {
    if !parece_lista(texto) {
        return Err(Erro::NaoELista);
    }
    let canais = ler(texto);
    if canais.is_empty() {
        return Err(Erro::SemCanais);
    }
    Ok(canais)
}

// MARK: - Guardar

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lista {
    pub id: String,
    pub nome: String,
    /// `None` quando a lista veio de arquivo (não há o que baixar de novo).
    #[serde(default)]
    pub link: Option<String>,
    #[serde(rename = "atualizadaEm")]
    pub atualizada_em: u64,
    pub ativa: bool,
    /// Quantos canais a lista tem (para mostrar sem abrir o arquivo dela).
    #[serde(default)]
    pub canais: usize,
}

static TRAVA: Mutex<()> = Mutex::new(());

fn pasta() -> PathBuf {
    let p = catalogo::pasta().join("minhas-listas");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn agora() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn ler_indice() -> Vec<Lista> {
    std::fs::read_to_string(pasta().join("indice.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn gravar_indice(listas: &[Lista]) {
    if let Ok(t) = serde_json::to_string(listas) {
        let _ = std::fs::write(pasta().join("indice.json"), t);
    }
}

fn arquivo_da(id: &str) -> PathBuf {
    pasta().join(format!("{id}.json"))
}

fn gravar_canais(id: &str, canais: &[CanalM3u]) {
    if let Ok(t) = serde_json::to_string(canais) {
        let _ = std::fs::write(arquivo_da(id), t);
    }
}

fn ler_canais(id: &str) -> Vec<CanalM3u> {
    std::fs::read_to_string(arquivo_da(id))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn listar() -> Vec<Lista> {
    let _g = TRAVA.lock();
    ler_indice()
}

/// Adiciona uma lista pelo link (rede: fora da thread do desenho).
pub fn adicionar(link: &str, nome: &str) -> Result<(), Erro> {
    let link = link.trim();
    if !comeca_com_http(link) {
        return Err(Erro::Esquema);
    }
    if listar().len() >= MAXIMO_DE_LISTAS {
        return Err(Erro::Limite);
    }
    let canais = interpretar(&baixar_texto(link)?)?;
    guardar_nova(&canais, Some(link.to_string()), nome)
}

/// Adiciona uma lista a partir de um arquivo .m3u/.m3u8 (sem link guardado).
pub fn adicionar_arquivo(caminho: &Path, nome: &str) -> Result<(), Erro> {
    if listar().len() >= MAXIMO_DE_LISTAS {
        return Err(Erro::Limite);
    }
    let canais = interpretar(&ler_arquivo(caminho)?)?;
    guardar_nova(&canais, None, nome)
}

fn guardar_nova(canais: &[CanalM3u], link: Option<String>, nome: &str) -> Result<(), Erro> {
    let _g = TRAVA.lock();
    let mut listas = ler_indice();
    if listas.len() >= MAXIMO_DE_LISTAS {
        return Err(Erro::Limite);
    }
    let mut id = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    while listas.iter().any(|l| l.id == id) {
        id.push('a');
    }
    let nome = nome.trim();
    let nome = if nome.is_empty() { format!("Lista {}", listas.len() + 1) } else { nome.to_string() };
    gravar_canais(&id, canais);
    listas.push(Lista { id, nome, link, atualizada_em: agora(), ativa: true, canais: canais.len() });
    gravar_indice(&listas);
    Ok(())
}

/// Baixa de novo. Se falhar, a versão guardada continua.
pub fn atualizar(id: &str) -> Result<(), Erro> {
    let link = listar().into_iter().find(|l| l.id == id).and_then(|l| l.link).ok_or(Erro::Download)?;
    let canais = interpretar(&baixar_texto(&link)?)?;
    substituir_canais(id, &canais);
    Ok(())
}

/// Atualiza uma lista de arquivo com outro arquivo. Se falhar, a versão guardada continua.
pub fn atualizar_arquivo(id: &str, caminho: &Path) -> Result<(), Erro> {
    let canais = interpretar(&ler_arquivo(caminho)?)?;
    substituir_canais(id, &canais);
    Ok(())
}

fn substituir_canais(id: &str, canais: &[CanalM3u]) {
    let _g = TRAVA.lock();
    let mut listas = ler_indice();
    if let Some(l) = listas.iter_mut().find(|l| l.id == id) {
        gravar_canais(id, canais);
        l.atualizada_em = agora();
        l.canais = canais.len();
        gravar_indice(&listas);
    }
}

pub fn definir_ativa(id: &str, ativa: bool) {
    let _g = TRAVA.lock();
    let mut listas = ler_indice();
    if let Some(l) = listas.iter_mut().find(|l| l.id == id) {
        l.ativa = ativa;
        gravar_indice(&listas);
    }
}

pub fn remover(id: &str) {
    let _g = TRAVA.lock();
    let mut listas = ler_indice();
    listas.retain(|l| l.id != id);
    gravar_indice(&listas);
    let _ = std::fs::remove_file(arquivo_da(id));
}

/// As listas ativas com mais de 12 h.
pub fn vencidas() -> Vec<String> {
    let agora = agora();
    listar().into_iter().filter(|l| l.ativa && l.link.is_some() && agora.saturating_sub(l.atualizada_em) > VALIDADE_S).map(|l| l.id).collect()
}

/// "atualizada há 3 h" e companhia.
pub fn ha_quanto_tempo(atualizada_em: u64) -> String {
    let s = agora().saturating_sub(atualizada_em);
    match s {
        0..=59 => "atualizada agora".to_string(),
        60..=3599 => format!("atualizada há {} min", s / 60),
        3600..=86399 => format!("atualizada há {} h", s / 3600),
        _ => format!("atualizada há {} d", s / 86400),
    }
}

// MARK: - Para o app

/// Converte uma lista lida para o `Canal` do app. Fontes com ClearKey saem (o
/// mpv não monta licença, como no catálogo). Restritos vão para "Adulto", que
/// o app já esconde até o desbloqueio.
pub fn para_canais(id: &str, lidos: &[CanalM3u]) -> Vec<Canal> {
    let _ = id; // o id do canal é minha-<id>-<chave>; o app identifica por nome
    lidos
        .iter()
        .filter_map(|c| {
            let fontes: Vec<Fonte> = c
                .fontes
                .iter()
                .filter(|f| f.key.is_none())
                .map(|f| Fonte {
                    url: f.url.clone(),
                    qualidade: f.qualidade.clone(),
                    nome: None,
                    referer: f.referer.clone(),
                    agente: f.ua.clone(),
                    chave: None,
                })
                .collect();
            if fontes.is_empty() {
                return None;
            }
            let restrito = e_restrito(c.grupo.as_deref(), &c.nome);
            Some(Canal {
                nome: c.nome.clone(),
                logo: c.logo.clone(),
                categoria: Some(if restrito { "Adulto" } else { SECAO }.to_string()),
                fontes,
            })
        })
        .collect()
}

/// Os canais de todas as listas ativas (restritos incluídos, na seção "Adulto").
pub fn canais_ativos() -> Vec<Canal> {
    let _g = TRAVA.lock();
    let mut tudo = Vec::new();
    for l in ler_indice().into_iter().filter(|l| l.ativa) {
        tudo.extend(para_canais(&l.id, &ler_canais(&l.id)));
    }
    tudo
}

/// O id do canal no formato combinado com os outros aplicativos.
#[allow(dead_code)]
pub fn id_do_canal(id_da_lista: &str, nome: &str) -> String {
    format!("minha-{id_da_lista}-{}", chave(nome))
}

#[cfg(test)]
mod testes {
    use super::*;

    const GABARITOS: &str = "/private/tmp/claude-501/-Volumes-SSD-1TB-DEV-Saimo-SaimoPlayer/c5ddbfb0-60d8-4a33-a367-79cbdbb1be68/scratchpad/m3u";

    fn confere(m3u: &str, json: &str) {
        let texto = std::fs::read_to_string(m3u).unwrap();
        let esperado: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(ler(&texto)).unwrap(), esperado);
    }

    #[test]
    fn gabarito_edge() {
        confere("tests/dados/edge.m3u", "tests/dados/edge.golden.json");
    }

    /// Só roda onde o br.m3u existe (ele não vai para o git).
    #[test]
    fn gabarito_br() {
        let m3u = format!("{GABARITOS}/br.m3u");
        if !std::path::Path::new(&m3u).exists() {
            return;
        }
        confere(&m3u, &format!("{GABARITOS}/br.golden.json"));
    }

    #[test]
    fn lixo_e_vazio_nao_quebram() {
        assert!(ler("").is_empty());
        assert!(ler("\u{feff}").is_empty());
        let binario: String = (0..5000u32).map(|i| char::from_u32(i % 300).unwrap_or('x')).collect();
        let _ = ler(&binario);
        let enorme = format!("#EXTINF:-1 tvg-name=\"{}\",{}\nhttps://a/b.m3u8", "x".repeat(1 << 20), "y".repeat(1 << 20));
        assert_eq!(ler(&enorme).len(), 1);
        assert!(ler(&"\"".repeat(1 << 20)).is_empty());
        assert!(ler("#EXTINF:-1,Sem endereco\n").is_empty());
    }

    #[test]
    fn parece_lista_e_erros() {
        assert_eq!(interpretar("<html>oi</html>").unwrap_err(), Erro::NaoELista);
        assert_eq!(interpretar("").unwrap_err(), Erro::NaoELista);
        assert_eq!(interpretar("#EXTM3U\n").unwrap_err(), Erro::SemCanais);
        assert_eq!(interpretar("#EXTM3U\n#EXTINF:-1,A\nrtmp://x/y\n").unwrap_err(), Erro::SemCanais);
        assert_eq!(interpretar("#EXTINF:-1,A\nhttp://x/y.m3u8\n").unwrap().len(), 1);
        assert_eq!(baixar_texto("ftp://x/y").unwrap_err(), Erro::Esquema);
        assert_eq!(baixar_texto("exemplo.com/lista.m3u").unwrap_err(), Erro::Esquema);
    }

    #[test]
    fn restritos() {
        assert!(e_restrito(Some("Adult"), "Qualquer"));
        assert!(e_restrito(Some("XXX"), "Qualquer"));
        assert!(e_restrito(None, "Canal 18+"));
        assert!(e_restrito(None, "Hot +18"));
        assert!(e_restrito(Some("Erótico"), "x"));
        assert!(e_restrito(None, "Adult Movies"));
        assert!(!e_restrito(None, "Adult Swim"));
        assert!(!e_restrito(Some("Adult Swim"), "Adult Swim Brasil"));
        assert!(!e_restrito(Some("Filmes"), "Globo"));
        assert!(!e_restrito(Some("Essex"), "Sussex TV"));
    }

    #[test]
    fn clearkey_sai_e_restrito_vai_para_adulto() {
        let m3u = "#EXTM3U\n#EXTINF:-1 group-title=\"Adult\",Noite\nhttps://a/1.m3u8\n#EXTINF:-1 group-title=\"Filmes\",Cine (720p)\nhttps://a/2.m3u8\n\
                   #EXTINF:-1,Drm\n#KODIPROP:inputstream.adaptive.license_type=clearkey\n#KODIPROP:inputstream.adaptive.license_key=0123456789abcdef0123456789abcdef:fedcba9876543210fedcba9876543210\nhttps://a/3.mpd\n";
        let c = para_canais("x", &ler(m3u));
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].secao(), "Adulto");
        assert_eq!(c[1].secao(), SECAO);
        assert_eq!(c[1].fontes[0].qualidade.as_deref(), Some("720p"));
        assert_eq!(id_do_canal("ab", "Cine HD"), "minha-ab-cinehd");
    }

    #[test]
    fn arquivo_le_e_erra() {
        let d = std::env::temp_dir().join(format!("saimo-ml-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let bom = d.join("a.m3u");
        std::fs::write(&bom, "\u{feff}#EXTM3U\n#EXTINF:-1,Canal \u{e9}\nhttps://a/1.m3u8\n").unwrap();
        assert_eq!(interpretar(&ler_arquivo(&bom).unwrap()).unwrap().len(), 1);
        let latin = d.join("l.m3u"); // byte inválido em UTF-8 não derruba
        std::fs::write(&latin, b"#EXTM3U\n#EXTINF:-1,Caf\xe9\nhttps://a/1.m3u8\n").unwrap();
        assert_eq!(interpretar(&ler_arquivo(&latin).unwrap()).unwrap().len(), 1);
        let lixo = d.join("b.txt");
        std::fs::write(&lixo, "ola mundo").unwrap();
        assert_eq!(interpretar(&ler_arquivo(&lixo).unwrap()), Err(Erro::NaoELista));
        let vazio = d.join("c.m3u");
        std::fs::write(&vazio, "#EXTM3U\n").unwrap();
        assert_eq!(interpretar(&ler_arquivo(&vazio).unwrap()), Err(Erro::SemCanais));
        assert_eq!(ler_arquivo(&d.join("nao-existe.m3u")), Err(Erro::Arquivo));
        let grande = d.join("g.m3u");
        std::fs::write(&grande, vec![b'a'; LIMITE_BYTES as usize + 1]).unwrap();
        assert_eq!(ler_arquivo(&grande), Err(Erro::Arquivo));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lista_antiga_com_link_texto_continua_lendo() {
        let l: Lista = serde_json::from_str(r#"{"id":"1","nome":"x","link":"http://a/b.m3u","atualizadaEm":5,"ativa":true}"#).unwrap();
        assert_eq!(l.link.as_deref(), Some("http://a/b.m3u"));
        let l: Lista = serde_json::from_str(r#"{"id":"1","nome":"x","link":null,"atualizadaEm":5,"ativa":true}"#).unwrap();
        assert!(l.link.is_none());
    }

    /// Rede local: `python3 -m http.server 8766` numa pasta com br.m3u e adulto.m3u.
    #[test]
    #[ignore = "usa servidor local na porta 8766"]
    fn baixa_do_servidor_local() {
        for l in listar() {
            remover(&l.id);
        }
        assert!(adicionar("http://127.0.0.1:8766/br.m3u", "").is_ok());
        assert_eq!(adicionar("http://127.0.0.1:8766/nada.m3u", "x"), Err(Erro::Download));
        assert_eq!(adicionar("http://127.0.0.1:8766/", "x"), Err(Erro::NaoELista));
        let l = listar();
        assert_eq!(l.last().unwrap().canais, 398);
        assert!(adicionar("http://127.0.0.1:8766/adulto.m3u", "Teste adulto").is_ok());
        let c = canais_ativos();
        assert!(c.iter().any(|c| c.nome == "Noite Quente" && c.secao() == "Adulto"));
        assert!(c.iter().any(|c| c.nome == "Adult Swim" && c.secao() == SECAO));
        let tudo = listar();
        definir_ativa(&tudo[0].id, false);
        assert_eq!(canais_ativos().len(), 3);
        for l in &tudo {
            remover(&l.id);
        }
        assert!(listar().is_empty() && canais_ativos().is_empty());
    }
}
