//! Perfis: cada um com os seus favoritos, títulos favoritos, onde parou, rádios
//! recentes e último canal; listas, fontes e ajustes são do computador todo.
//! Contrato comum dos apps em SaimoPlayer/docs/perfis.md.
//!
//! Armazenamento: o perfil principal usa os arquivos de sempre, na pasta do
//! programa (quem atualiza não perde nada); cada outro perfil tem a pasta
//! `perfis/<id>/` com os mesmos nomes de arquivo. Trocar de perfil reabre o
//! programa, para nada do perfil anterior ficar na memória.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub const PRINCIPAL: &str = "principal";
pub const MAXIMO: usize = 10;
pub const MAX_NOME: usize = 20;

/// Ids de SaimoPlayer/docs/avatares/ids.txt, na ordem da grade.
pub const AVATARES: [&str; 21] = [
    "gato", "cachorro", "raposa", "urso", "panda", "coelho", "coruja", "pinguim", "sapo", "leao", "coala", "robo",
    "alien", "monstro", "dino", "polvo", "pintinho", "tigre", "astronauta", "abelha", "fantasma",
];

/// Os arquivos que são de cada perfil.
pub const ARQUIVOS_DO_PERFIL: [&str; 9] = [
    "favoritos.txt", "favoritos-vod.txt", "ultimo-canal.txt", "radios-recentes.txt", "progresso.json", "progresso-quando.json", "estatisticas.json",
    "novos-episodios.json", "remocoes.json",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Perfil {
    pub id: String,
    pub nome: String,
    pub avatar: String,
    /// Quando nome, avatar ou Kids mudaram por último (ms).
    pub em: f64,
    /// Perfil infantil: só títulos até 12 anos, nada adulto; sair pede o código.
    #[serde(default, skip_serializing_if = "is_false")]
    pub kids: bool,
    /// SHA-256 de `saimo-kids:<id>:<código>`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub codigo: String,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Remocao {
    pub id: String,
    pub em: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Estado {
    #[serde(default)]
    lista: Vec<Perfil>,
    #[serde(default = "principal")]
    ativo: String,
    #[serde(default)]
    removidos: Vec<Remocao>,
    #[serde(default = "sim")]
    perguntar: bool,
}

fn principal() -> String {
    PRINCIPAL.into()
}
fn sim() -> bool {
    true
}

static ESTADO: Mutex<Option<Estado>> = Mutex::new(None);
/// O programa foi aberto já com o perfil escolhido (reaberto pela troca).
static ESCOLHIDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn arquivo() -> PathBuf {
    crate::catalogo::pasta().join("perfis.json")
}

fn normalizar(lista: Vec<Perfil>) -> Vec<Perfil> {
    let mut vistos = std::collections::HashSet::new();
    let mut limpa: Vec<Perfil> = lista.into_iter().filter(|p| !p.id.is_empty() && vistos.insert(p.id.clone())).collect();
    for p in limpa.iter_mut().filter(|p| p.id == PRINCIPAL) {
        p.kids = false;
        p.codigo.clear();
    }
    let principal = limpa.iter().find(|p| p.id == PRINCIPAL).cloned().unwrap_or(Perfil {
        id: PRINCIPAL.into(),
        nome: "Principal".into(),
        avatar: "gato".into(),
        em: 0.0,
        kids: false,
        codigo: String::new(),
    });
    let mut out = vec![principal];
    out.extend(limpa.into_iter().filter(|p| p.id != PRINCIPAL));
    out.truncate(MAXIMO);
    out
}

fn ler() -> Estado {
    let mut g = ESTADO.lock().unwrap();
    if g.is_none() {
        let mut e: Estado = std::fs::read_to_string(arquivo())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Estado { lista: vec![], ativo: PRINCIPAL.into(), removidos: vec![], perguntar: true });
        e.lista = normalizar(e.lista);
        if !e.lista.iter().any(|p| p.id == e.ativo) {
            e.ativo = PRINCIPAL.into();
        }
        *g = Some(e);
    }
    g.clone().unwrap()
}

fn gravar(mut e: Estado) {
    e.lista = normalizar(e.lista);
    if let Ok(t) = serde_json::to_string_pretty(&e) {
        let _ = std::fs::write(arquivo(), t);
    }
    *ESTADO.lock().unwrap() = Some(e);
    MUDOU.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Sobe a cada mudança na lista de perfis (a sincronia avisa o celular).
pub static MUDOU: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn todos() -> Vec<Perfil> {
    ler().lista
}
pub fn removidos() -> Vec<Remocao> {
    ler().removidos
}
pub fn id_ativo() -> String {
    ler().ativo
}
pub fn ativo() -> Perfil {
    let e = ler();
    e.lista.iter().find(|p| p.id == e.ativo).cloned().unwrap_or_else(|| e.lista[0].clone())
}
pub fn kids_ativo() -> bool {
    ativo().kids
}
pub fn perguntar_ao_abrir() -> bool {
    ler().perguntar
}
pub fn definir_perguntar(v: bool) {
    let mut e = ler();
    e.perguntar = v;
    gravar(e);
}
pub fn marcar_escolhido() {
    ESCOLHIDO.store(true, std::sync::atomic::Ordering::Relaxed);
}
pub fn deve_escolher_ao_abrir() -> bool {
    !ESCOLHIDO.load(std::sync::atomic::Ordering::Relaxed)
        && !std::env::args().any(|a| a == "--perfil-escolhido")
        && perguntar_ao_abrir()
        && todos().len() >= 2
}

// ---------- armazenamento ----------

pub fn pasta_de(id: &str) -> PathBuf {
    let base = crate::catalogo::pasta();
    if id == PRINCIPAL {
        return base;
    }
    let p = base.join("perfis").join(id);
    let _ = std::fs::create_dir_all(&p);
    p
}

/// Onde mora um arquivo: o de perfil na pasta do perfil ativo; o resto, na do programa.
pub fn caminho(nome: &str) -> PathBuf {
    if ARQUIVOS_DO_PERFIL.contains(&nome) {
        pasta_de(&id_ativo()).join(nome)
    } else {
        crate::catalogo::pasta().join(nome)
    }
}

// ---------- Kids ----------

/// SHA-256 (hex) em Rust puro: só para o código do Kids e a senha do parental,
/// sem trazer uma dependência a mais.
fn sha(texto: &str) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
        0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
        0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
        0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut dados = texto.as_bytes().to_vec();
    let bits = (dados.len() as u64) * 8;
    dados.push(0x80);
    while dados.len() % 64 != 56 {
        dados.push(0);
    }
    dados.extend_from_slice(&bits.to_be_bytes());
    for bloco in dados.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([bloco[4 * i], bloco[4 * i + 1], bloco[4 * i + 2], bloco[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod testes_sha {
    #[test]
    fn confere_com_o_padrao() {
        assert_eq!(super::sha("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(super::sha(""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}

pub fn hash_codigo(id: &str, codigo: &str) -> String {
    sha(&format!("saimo-kids:{id}:{codigo}"))
}
pub fn pede_codigo(p: &Perfil) -> bool {
    p.kids && !p.codigo.is_empty()
}
pub fn conferir_codigo(p: &Perfil, codigo: &str) -> bool {
    !pede_codigo(p) || hash_codigo(&p.id, codigo) == p.codigo
}
fn quatro_digitos(c: &str) -> bool {
    c.len() == 4 && c.chars().all(|x| x.is_ascii_digit())
}

// ---------- criar, editar, apagar ----------

fn limpar(nome: &str) -> String {
    nome.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NOME).collect()
}

fn agora_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

fn novo_id() -> String {
    let letras = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut semente = agora_ms() as u64 ^ (std::process::id() as u64).rotate_left(17);
    let mut s = String::from("p");
    for _ in 0..9 {
        semente ^= semente << 13;
        semente ^= semente >> 7;
        semente ^= semente << 17;
        s.push(letras[(semente % letras.len() as u64) as usize] as char);
    }
    s
}

/// `kids_codigo`: 4 dígitos ligam o Kids com esse código.
pub fn criar(nome: &str, avatar: &str, kids_codigo: Option<&str>) -> Option<Perfil> {
    let mut e = ler();
    let limpo = limpar(nome);
    if limpo.is_empty() || e.lista.len() >= MAXIMO {
        return None;
    }
    let id = novo_id();
    let mut p = Perfil { id: id.clone(), nome: limpo, avatar: avatar.into(), em: agora_ms(), kids: false, codigo: String::new() };
    if let Some(c) = kids_codigo.filter(|c| quatro_digitos(c)) {
        p.kids = true;
        p.codigo = hash_codigo(&id, c);
    }
    e.lista.push(p.clone());
    gravar(e);
    Some(p)
}

/// `kids_codigo`: None mantém; Some("") desliga o Kids; Some(4 dígitos) liga ou troca o código.
pub fn editar(id: &str, nome: &str, avatar: &str, kids_codigo: Option<&str>) {
    let limpo = limpar(nome);
    if limpo.is_empty() {
        return;
    }
    let mut e = ler();
    if let Some(p) = e.lista.iter_mut().find(|p| p.id == id) {
        p.nome = limpo;
        p.avatar = avatar.into();
        p.em = agora_ms();
        match kids_codigo {
            Some("") => {
                p.kids = false;
                p.codigo.clear();
            }
            Some(c) if id != PRINCIPAL && quatro_digitos(c) => {
                p.kids = true;
                p.codigo = hash_codigo(id, c);
            }
            _ => {}
        }
    }
    gravar(e);
}

fn apagar_local(id: &str) {
    if id == PRINCIPAL {
        return;
    }
    let _ = std::fs::remove_dir_all(crate::catalogo::pasta().join("perfis").join(id));
    let mut e = ler();
    e.lista.retain(|p| p.id != id);
    if e.ativo == id {
        e.ativo = PRINCIPAL.into();
    }
    gravar(e);
}

fn guardar_remocao(r: Remocao) {
    let mut e = ler();
    e.removidos.retain(|x| x.id != r.id);
    e.removidos.insert(0, r);
    e.removidos.sort_by(|a, b| b.em.partial_cmp(&a.em).unwrap_or(std::cmp::Ordering::Equal));
    e.removidos.truncate(50);
    gravar(e);
}

/// Apaga o perfil e os dados dele. O principal não sai.
pub fn apagar(id: &str) {
    if id == PRINCIPAL {
        return;
    }
    apagar_local(id);
    guardar_remocao(Remocao { id: id.into(), em: agora_ms() });
}

/// Entra no perfil. Outro perfil: grava e reabre o programa já nele.
pub fn entrar(id: &str) {
    marcar_escolhido();
    let mut e = ler();
    if e.ativo == id || !e.lista.iter().any(|p| p.id == id) {
        return;
    }
    e.ativo = id.into();
    gravar(e);
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--perfil-escolhido").spawn();
    }
    std::process::exit(0);
}

// ---------- sincronia (Saimo Connect) ----------

/// Junta a lista de perfis vinda do celular. Devolve os ids que existem aqui depois.
pub fn mesclar(remotos: &[Perfil], remocoes: &[Remocao]) -> Vec<String> {
    let locais_removidos: HashMap<String, f64> = removidos().into_iter().map(|r| (r.id, r.em)).collect();
    for r in remocoes.iter().filter(|r| r.id != PRINCIPAL) {
        if let Some(local) = todos().into_iter().find(|p| p.id == r.id) {
            if local.em <= r.em {
                apagar_local(&r.id);
            }
        }
        if locais_removidos.get(&r.id).copied().unwrap_or(0.0) < r.em {
            guardar_remocao(r.clone());
        }
    }
    let removidos_agora: HashMap<String, f64> = removidos().into_iter().map(|r| (r.id, r.em)).collect();
    let mut e = ler();
    for p in remotos.iter().filter(|p| !p.id.is_empty() && !p.nome.is_empty()) {
        if removidos_agora.get(&p.id).is_some_and(|em| *em >= p.em) {
            continue;
        }
        let mut novo = p.clone();
        novo.nome = limpar(&p.nome);
        if novo.id == PRINCIPAL {
            novo.kids = false;
            novo.codigo.clear();
        }
        if let Some(local) = e.lista.iter_mut().find(|x| x.id == p.id) {
            if p.em > local.em {
                *local = novo;
            }
        } else if e.lista.len() < MAXIMO {
            e.lista.push(novo);
        }
    }
    gravar(e);
    todos().into_iter().map(|p| p.id).collect()
}

/// Os perfis no formato do pacote (sem os dados, que a sincronia completa).
pub fn para_pacote() -> Vec<Value> {
    todos()
        .into_iter()
        .map(|p| {
            let mut o = json!({"id": p.id, "nome": p.nome, "avatar": p.avatar, "em": p.em as i64});
            if p.kids {
                o["kids"] = json!(true);
                o["codigo"] = json!(p.codigo);
            }
            o
        })
        .collect()
}

pub fn remocoes_para_pacote() -> Vec<Value> {
    removidos().into_iter().map(|r| json!({"id": r.id, "em": r.em as i64})).collect()
}

/// Lista de texto (favoritos...) de um perfil qualquer.
pub fn ler_lista_de(id: &str, nome: &str) -> Vec<String> {
    std::fs::read_to_string(pasta_de(id).join(nome))
        .map(|t| t.lines().map(str::to_string).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

pub fn gravar_lista_de(id: &str, nome: &str, itens: &[String]) {
    let _ = std::fs::write(pasta_de(id).join(nome), itens.join("\n"));
}

/// Onde parou de um perfil qualquer: chave -> (posição, duração, quando em ms).
pub fn progresso_de(id: &str) -> Vec<(String, f64, f64, i64)> {
    let marcas: HashMap<String, (f64, f64)> = std::fs::read_to_string(pasta_de(id).join("progresso.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let quando: HashMap<String, i64> = std::fs::read_to_string(pasta_de(id).join("progresso-quando.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    marcas.into_iter().map(|(k, (p, d))| (k.clone(), p, d, quando.get(&k).copied().unwrap_or(0))).collect()
}

/// Tira do progresso de um perfil que não está aberto as marcas que `tirar(chave, quando)` aponta.
pub fn esquecer_progresso_de(id: &str, tirar: impl Fn(&str, i64) -> bool) {
    let pasta = pasta_de(id);
    let mut marcas: HashMap<String, (f64, f64)> = std::fs::read_to_string(pasta.join("progresso.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut quando: HashMap<String, i64> = std::fs::read_to_string(pasta.join("progresso-quando.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let fora: Vec<String> = marcas.keys().filter(|k| tirar(k, quando.get(*k).copied().unwrap_or(0))).cloned().collect();
    if fora.is_empty() {
        return;
    }
    for k in &fora {
        marcas.remove(k);
        quando.remove(k);
    }
    if let Ok(t) = serde_json::to_string(&marcas) {
        let _ = std::fs::write(pasta.join("progresso.json"), t);
    }
    if let Ok(t) = serde_json::to_string(&quando) {
        let _ = std::fs::write(pasta.join("progresso-quando.json"), t);
    }
}

/// Junta uma marca vinda do celular ao progresso de um perfil que não está aberto.
pub fn importar_progresso_de(id: &str, chave: &str, pos: f64, dur: f64, em: i64) {
    let pasta = pasta_de(id);
    let mut marcas: HashMap<String, (f64, f64)> = std::fs::read_to_string(pasta.join("progresso.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut quando: HashMap<String, i64> = std::fs::read_to_string(pasta.join("progresso-quando.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if quando.get(chave).copied().unwrap_or(0) >= em || pos <= 0.0 {
        return;
    }
    marcas.insert(chave.to_string(), (pos, dur));
    quando.insert(chave.to_string(), em);
    if let Ok(t) = serde_json::to_string(&marcas) {
        let _ = std::fs::write(pasta.join("progresso.json"), t);
    }
    if let Ok(t) = serde_json::to_string(&quando) {
        let _ = std::fs::write(pasta.join("progresso-quando.json"), t);
    }
}

// ---------- controle parental ----------

/// Senha do controle parental: padrão 0000, trocável nos Ajustes; vale para o
/// computador todo. No perfil Kids o conteúdo adulto nunca destranca.
pub const SENHA_PADRAO: &str = "0000";

fn arquivo_senha() -> PathBuf {
    crate::catalogo::pasta().join("parental.txt")
}

pub fn senha_padrao() -> bool {
    std::fs::read_to_string(arquivo_senha()).map(|t| t.trim().is_empty()).unwrap_or(true)
}

pub fn conferir_senha(senha: &str) -> bool {
    if kids_ativo() {
        return false;
    }
    match std::fs::read_to_string(arquivo_senha()).map(|t| t.trim().to_string()) {
        Ok(h) if !h.is_empty() => sha(&format!("saimo-parental:{senha}")) == h,
        _ => senha == SENHA_PADRAO,
    }
}

pub fn trocar_senha(atual: &str, nova: &str) -> bool {
    if !conferir_senha(atual) || !quatro_digitos(nova) {
        return false;
    }
    std::fs::write(arquivo_senha(), sha(&format!("saimo-parental:{nova}"))).is_ok()
}

/// O PNG de um avatar (embutido no programa).
pub fn png(avatar: &str) -> Option<&'static [u8]> {
    Some(match avatar {
        "gato" => include_bytes!("../assets/avatares/gato.png"),
        "cachorro" => include_bytes!("../assets/avatares/cachorro.png"),
        "raposa" => include_bytes!("../assets/avatares/raposa.png"),
        "urso" => include_bytes!("../assets/avatares/urso.png"),
        "panda" => include_bytes!("../assets/avatares/panda.png"),
        "coelho" => include_bytes!("../assets/avatares/coelho.png"),
        "coruja" => include_bytes!("../assets/avatares/coruja.png"),
        "pinguim" => include_bytes!("../assets/avatares/pinguim.png"),
        "sapo" => include_bytes!("../assets/avatares/sapo.png"),
        "leao" => include_bytes!("../assets/avatares/leao.png"),
        "coala" => include_bytes!("../assets/avatares/coala.png"),
        "robo" => include_bytes!("../assets/avatares/robo.png"),
        "alien" => include_bytes!("../assets/avatares/alien.png"),
        "monstro" => include_bytes!("../assets/avatares/monstro.png"),
        "dino" => include_bytes!("../assets/avatares/dino.png"),
        "polvo" => include_bytes!("../assets/avatares/polvo.png"),
        "pintinho" => include_bytes!("../assets/avatares/pintinho.png"),
        "tigre" => include_bytes!("../assets/avatares/tigre.png"),
        "astronauta" => include_bytes!("../assets/avatares/astronauta.png"),
        "abelha" => include_bytes!("../assets/avatares/abelha.png"),
        "fantasma" => include_bytes!("../assets/avatares/fantasma.png"),
        _ => return None,
    })
}
