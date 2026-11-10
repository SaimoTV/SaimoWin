//! Onde cada filme e episódio parou.
//!
//! A chave é o título (e, na série, temporada e episódio), não o endereço: o
//! mesmo episódio vem de fontes diferentes, e trocar de fonte não pode fazer
//! recomeçar do zero. Fica gravado em disco de propósito — voltar ao ponto em
//! que se parou não é enfeite.

use std::collections::HashMap;
use std::sync::Mutex;

/// Menos de um minuto não é "onde parou", é ter aberto e desistido.
const MINIMO: f64 = 60.0;
/// A dois minutos do fim está visto: retomar ali só irrita.
const SOBRA: f64 = 120.0;

#[derive(Clone, Copy, Debug)]
pub struct Marca {
    pub posicao: f64,
    pub duracao: f64,
}

static MARCAS: Mutex<Option<HashMap<String, (f64, f64)>>> = Mutex::new(None);
/// Sobe a cada marca gravada ou apagada: quem guarda uma lista montada com
/// as marcas (a fileira "Continue assistindo") sabe quando refazê-la.
static VERSAO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn versao() -> u64 {
    VERSAO.load(std::sync::atomic::Ordering::Relaxed)
}

fn arquivo() -> std::path::PathBuf {
    crate::catalogo::pasta().join("progresso.json")
}

fn marcas() -> std::sync::MutexGuard<'static, Option<HashMap<String, (f64, f64)>>> {
    let mut guarda = MARCAS.lock().unwrap();
    if guarda.is_none() {
        let lido = std::fs::read_to_string(arquivo())
            .ok()
            .and_then(|t| serde_json::from_str::<HashMap<String, (f64, f64)>>(&t).ok())
            .unwrap_or_default();
        *guarda = Some(lido);
    }
    guarda
}

/// Quando cada marca foi gravada (ms desde 1970): na sincronia pelo Saimo
/// Connect vale a mais recente. Arquivo à parte para não mudar o formato do
/// `progresso.json` que as versões anteriores gravaram.
static QUANDO: Mutex<Option<HashMap<String, i64>>> = Mutex::new(None);

fn arquivo_quando() -> std::path::PathBuf {
    crate::catalogo::pasta().join("progresso-quando.json")
}

fn quando() -> std::sync::MutexGuard<'static, Option<HashMap<String, i64>>> {
    let mut guarda = QUANDO.lock().unwrap();
    if guarda.is_none() {
        let lido = std::fs::read_to_string(arquivo_quando())
            .ok()
            .and_then(|t| serde_json::from_str::<HashMap<String, i64>>(&t).ok())
            .unwrap_or_default();
        *guarda = Some(lido);
    }
    guarda
}

fn gravar_quando() {
    let guarda = quando();
    if let Some(mapa) = guarda.as_ref() {
        if let Ok(texto) = serde_json::to_string(mapa) {
            let _ = std::fs::write(arquivo_quando(), texto);
        }
    }
}

fn agora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Tudo o que está pela metade, com o horário: (título, posição, duração, quando).
pub fn todas() -> Vec<(String, f64, f64, i64)> {
    let lista: Vec<(String, f64, f64)> = {
        let guarda = marcas();
        guarda.as_ref().map(|m| m.iter().map(|(t, (p, d))| (t.clone(), *p, *d)).collect()).unwrap_or_default()
    };
    let q = quando();
    let q = q.as_ref();
    lista
        .into_iter()
        .map(|(t, p, d)| {
            let em = q.and_then(|m| m.get(&t)).copied().unwrap_or(0);
            (t, p, d, em)
        })
        .collect()
}

/// Marca vinda de outro aparelho: entra se for mais recente que a daqui.
pub fn importar(titulo: &str, posicao: f64, duracao: f64, em: i64) -> bool {
    if titulo.is_empty() || !(posicao > 0.0) {
        return false;
    }
    let local = quando().as_ref().and_then(|m| m.get(titulo)).copied().unwrap_or(0);
    let existe = marcas().as_ref().is_some_and(|m| m.contains_key(titulo));
    if existe && local >= em {
        return false;
    }
    marcas().as_mut().unwrap().insert(titulo.to_string(), (posicao, duracao.max(0.0)));
    quando().as_mut().unwrap().insert(titulo.to_string(), em);
    gravar();
    gravar_quando();
    true
}

fn gravar() {
    VERSAO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let guarda = marcas();
    if let Some(mapa) = guarda.as_ref() {
        if let Ok(texto) = serde_json::to_string(mapa) {
            let _ = std::fs::write(arquivo(), texto);
        }
    }
}

/// A fração já vista, de 0 a 1. Zero quando a fonte não diz a duração: aí não
/// há barra a desenhar.
pub fn fracao(marca: &Marca) -> f32 {
    if marca.duracao <= 0.0 {
        return 0.0;
    }
    (marca.posicao / marca.duracao).clamp(0.0, 1.0) as f32
}

pub fn onde_parou(titulo: &str) -> Option<Marca> {
    let guarda = marcas();
    let (posicao, duracao) = guarda.as_ref()?.get(titulo).copied()?;
    Some(Marca { posicao, duracao })
}

/// Guarda a posição. Começo e fim apagam a marca em vez de gravá-la.
///
/// Duração 0 é "a fonte não diz quanto dura" — acontece com o HLS do
/// EmbedPlayer. Exigir a duração fazia o progresso nunca ser gravado nesses
/// filmes; sem ela dá para guardar onde parou do mesmo jeito, só não dá para
/// dizer quanto falta.
pub fn salvar(titulo: &str, posicao: f64, duracao: f64) {
    if titulo.is_empty() || !posicao.is_finite() || duracao < 0.0 {
        return;
    }
    let no_fim = duracao > 0.0 && posicao > duracao - SOBRA;
    {
        let mut guarda = marcas();
        let mapa = guarda.as_mut().unwrap();
        if posicao < MINIMO || no_fim {
            if mapa.remove(titulo).is_none() {
                return;
            }
        } else {
            mapa.insert(titulo.to_string(), (posicao, duracao));
        }
    }
    quando().as_mut().unwrap().insert(titulo.to_string(), agora_ms());
    gravar_quando();
    gravar();
}

/// Onde parou o último episódio visto de uma série.
///
/// A chave do episódio é "Série · T1 E2"; a do cartão é só o nome da série.
/// Sem isto o cartão de série nunca mostrava barra nenhuma.
pub fn onde_parou_na_serie(titulo: &str) -> Option<(String, Marca)> {
    let prefixo = format!("{titulo} · ");
    let guarda = marcas();
    let mapa = guarda.as_ref()?;
    let mut achados: Vec<(&String, &(f64, f64))> = mapa
        .iter()
        .filter(|(chave, _)| chave.starts_with(&prefixo))
        .collect();
    achados.sort_by(|a, b| a.0.cmp(b.0));
    let (chave, (posicao, duracao)) = achados.pop()?;
    Some((chave.clone(), Marca { posicao: *posicao, duracao: *duracao }))
}

/// O que dá para continuar, do mais recente para o mais antigo.
pub fn pendentes() -> Vec<(String, Marca)> {
    let guarda = marcas();
    let Some(mapa) = guarda.as_ref() else { return Vec::new() };
    let mut lista: Vec<(String, Marca)> = mapa
        .iter()
        .map(|(titulo, (posicao, duracao))| {
            (titulo.clone(), Marca { posicao: *posicao, duracao: *duracao })
        })
        .collect();
    lista.sort_by(|a, b| a.0.cmp(&b.0));
    lista
}

pub fn esquecer(titulo: &str) {
    {
        let mut guarda = marcas();
        guarda.as_mut().unwrap().remove(titulo);
    }
    gravar();
}

/// "1 h 12 min" — o quanto falta.
pub fn falta(marca: &Marca) -> String {
    if marca.duracao <= 0.0 {
        // Sem duração, o que dá para dizer é onde parou.
        let visto = marca.posicao.max(0.0) as i64;
        let horas = visto / 3600;
        let minutos = (visto % 3600) / 60;
        return if horas > 0 {
            format!("parou em {horas} h {minutos:02} min")
        } else {
            format!("parou em {minutos} min")
        };
    }
    let restante = (marca.duracao - marca.posicao).max(0.0) as i64;
    let horas = restante / 3600;
    let minutos = (restante % 3600) / 60;
    if horas > 0 {
        format!("faltam {horas} h {minutos:02} min")
    } else {
        format!("faltam {minutos} min")
    }
}
