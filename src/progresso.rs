//! Onde cada filme e episódio parou.
//!
//! A chave é o título (e, na série, temporada e episódio), não o endereço: o
//! mesmo episódio vem de fontes diferentes, e trocar de fonte não pode fazer
//! recomeçar do zero. Fica gravado em disco de propósito — voltar ao ponto em
//! que se parou não é enfeite.

use std::collections::HashMap;
use std::sync::Mutex;

/// Menos de um minuto não é "onde parou", é ter aberto e desistido.
const MINIMO: f64 = 15.0;
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
    // Episódios curtos não podem perder o progresso dois minutos antes do fim.
    let sobra = if duracao > 0.0 { (duracao * 0.03).clamp(15.0, SOBRA) } else { SOBRA };
    let no_fim = duracao > 0.0 && posicao >= duracao - sobra;
    // Não apagar um progresso anterior enquanto o novo vídeo ainda abre:
    // o mpv às vezes informa 0 nos primeiros quadros, antes de retomar.
    if posicao < MINIMO {
        return;
    }
    {
        let mut guarda = marcas();
        let mapa = guarda.as_mut().unwrap();
        if no_fim {
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

/// O último episódio reproduzido, inclusive quando já terminou ou durou menos
/// de um minuto. As marcas parciais não bastam: ao terminar, salvar() as apaga.
static ULTIMOS_EPISODIOS: Mutex<Option<HashMap<String, (u32, u32)>>> = Mutex::new(None);

fn arquivo_ultimos_episodios() -> std::path::PathBuf {
    crate::catalogo::pasta().join("ultimos-episodios.json")
}

fn ultimos_episodios() -> std::sync::MutexGuard<'static, Option<HashMap<String, (u32, u32)>>> {
    let mut guarda = ULTIMOS_EPISODIOS.lock().unwrap();
    if guarda.is_none() {
        let lido = std::fs::read_to_string(arquivo_ultimos_episodios())
            .ok()
            .and_then(|texto| serde_json::from_str::<HashMap<String, (u32, u32)>>(&texto).ok())
            .unwrap_or_default();
        *guarda = Some(lido);
    }
    guarda
}

/// Chamado somente depois que o mpv confirmou a reprodução do episódio.
pub fn lembrar_episodio(serie: &str, temporada: u32, numero: u32) {
    if serie.is_empty() || numero == 0 {
        return;
    }
    let texto = {
        let mut guarda = ultimos_episodios();
        let mapa = guarda.as_mut().unwrap();
        if mapa.get(serie) == Some(&(temporada, numero)) {
            return;
        }
        mapa.insert(serie.to_string(), (temporada, numero));
        serde_json::to_string(mapa).ok()
    };
    if let Some(texto) = texto {
        let _ = std::fs::write(arquivo_ultimos_episodios(), texto);
    }
}

/// A chave de progresso é "Série · T1 E2"; usa números, não ordenação textual
/// (T10 E10 não pode vir antes de T2 E9 por comparação alfabética).
fn numeros_do_episodio(serie: &str, chave: &str) -> Option<(u32, u32)> {
    let resto = chave.strip_prefix(serie)?.strip_prefix(" · T")?;
    let (temporada, numero) = resto.split_once(" E")?;
    Some((temporada.parse().ok()?, numero.parse().ok()?))
}

/// Escolhe o episódio parcial gravado mais recentemente. Para marcas antigas sem
/// data, desempata por temporada/episódio numéricos.
fn marca_da_serie(
    serie: &str,
    mapa: &HashMap<String, (f64, f64)>,
    horarios: Option<&HashMap<String, i64>>,
) -> Option<(String, Marca)> {
    mapa.iter()
        .filter_map(|(chave, &(posicao, duracao))| {
            let (temporada, numero) = numeros_do_episodio(serie, chave)?;
            let quando = horarios.and_then(|q| q.get(chave)).copied().unwrap_or(0);
            Some((chave.clone(), Marca { posicao, duracao }, (quando, temporada, numero)))
        })
        .max_by_key(|(_, _, ordem)| *ordem)
        .map(|(chave, marca, _)| (chave, marca))
}

/// Último episódio registrado no histórico, inclusive o que terminou.
/// Para instalações antigas ainda sem histórico, usa o mapa anterior e marcas.
pub fn episodio_da_serie(serie: &str) -> Option<(u32, u32)> {
    let ultima_reproducao = {
        let guarda = historico_eventos();
        guarda.as_ref()
            .and_then(|lista| lista.iter()
                .filter(|e| e.serie && e.titulo == serie)
                .max_by_key(|e| e.quando)
                .map(|e| (e.temporada, e.episodio)))
    };
    if let Some(episodio) = ultima_reproducao {
        return Some(episodio);
    }
    if let Some(episodio) = ultimos_episodios()
        .as_ref()
        .and_then(|mapa| mapa.get(serie).copied()) {
        return Some(episodio);
    }
    onde_parou_na_serie(serie).and_then(|(chave, _)| numeros_do_episodio(serie, &chave))
}

/// Onde ficou interrompida uma série; mantém compatibilidade com as marcas
/// antigas, sem confundir temporadas de dois dígitos.
pub fn onde_parou_na_serie(titulo: &str) -> Option<(String, Marca)> {
    let guarda = marcas();
    let mapa = guarda.as_ref()?;
    let q = quando();
    marca_da_serie(titulo, mapa, q.as_ref())
}


#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EventoHistorico {
    pub id: String,
    pub titulo: String,
    pub serie: bool,
    pub temporada: u32,
    pub episodio: u32,
    pub quando: i64,
}

#[derive(Clone, Debug)]
pub struct ResumoHistorico {
    pub titulo: String,
    pub serie: bool,
    pub temporada: u32,
    pub episodio: u32,
    pub ultimo_em: i64,
    pub reproducoes: usize,
}

/// Log pequeno de reproduções com IDs exclusivos. Sincronizar o mesmo log de
/// dois aparelhos não multiplica as contagens, pois a fusão elimina IDs repetidos.
static HISTORICO: Mutex<Option<Vec<EventoHistorico>>> = Mutex::new(None);
const MAX_EVENTOS: usize = 300;

fn arquivo_historico() -> std::path::PathBuf {
    crate::catalogo::pasta().join("historico-vod.json")
}

fn historico_eventos() -> std::sync::MutexGuard<'static, Option<Vec<EventoHistorico>>> {
    let mut guarda = HISTORICO.lock().unwrap();
    if guarda.is_none() {
        let eventos = std::fs::read_to_string(arquivo_historico()).ok()
            .and_then(|s| serde_json::from_str::<Vec<EventoHistorico>>(&s).ok())
            .unwrap_or_default();
        *guarda = Some(eventos);
    }
    guarda
}

pub fn eventos_historico() -> Vec<EventoHistorico> {
    historico_eventos().as_ref().cloned().unwrap_or_default()
}

fn id_novo() -> String {
    use std::hash::{BuildHasher, Hasher};
    let aleatorio = std::collections::hash_map::RandomState::new().build_hasher().finish();
    format!("{:x}-{aleatorio:016x}", agora_ms())
}

fn registrar(titulo: &str, serie: bool, temporada: u32, episodio: u32) {
    if titulo.is_empty() { return; }
    let evento = EventoHistorico {
        id: id_novo(), titulo: titulo.to_string(), serie, temporada, episodio, quando: agora_ms()
    };
    let texto = {
        let mut guarda = historico_eventos();
        let lista = guarda.as_mut().unwrap();
        lista.push(evento);
        lista.sort_by_key(|e| std::cmp::Reverse(e.quando));
        lista.truncate(MAX_EVENTOS);
        serde_json::to_string(lista).ok()
    };
    if let Some(texto) = texto {
        let _ = std::fs::write(arquivo_historico(), texto);
        VERSAO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn registrar_filme(titulo: &str) {
    registrar(titulo, false, 0, 0);
}

pub fn registrar_episodio(serie: &str, temporada: u32, episodio: u32) {
    if serie.is_empty() || episodio == 0 { return; }
    lembrar_episodio(serie, temporada, episodio);
    registrar(serie, true, temporada, episodio);
}

/// Une logs de aparelhos sem duplicar reproduções já importadas.
fn mesclar_eventos(local: &mut Vec<EventoHistorico>, remotos: &[EventoHistorico]) -> bool {
    let mut ids: std::collections::HashSet<String> = local.iter().map(|e| e.id.clone()).collect();
    let mut mudou = false;
    for e in remotos.iter().take(MAX_EVENTOS) {
        if e.id.is_empty() || e.id.len() > 100 || e.titulo.is_empty() || e.titulo.len() > 400
            || e.quando <= 0 || e.episodio > 100000 || e.temporada > 100000 { continue; }
        if ids.insert(e.id.clone()) {
            local.push(e.clone());
            mudou = true;
        }
    }
    if mudou {
        local.sort_by_key(|e| std::cmp::Reverse(e.quando));
        local.truncate(MAX_EVENTOS);
    }
    mudou
}

/// Recebe eventos extras, opcionais, de outros aparelhos Saimo atualizados.
/// O ID evita duplicar uma reprodução em trocas repetidas de pacote.
pub fn importar_historico(remotos: &[EventoHistorico]) -> bool {
    let texto = {
        let mut guarda = historico_eventos();
        let local = guarda.as_mut().unwrap();
        if !mesclar_eventos(local, remotos) { return false; }
        serde_json::to_string(local).ok()
    };
    if let Some(texto) = texto {
        let _ = std::fs::write(arquivo_historico(), texto);
        VERSAO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return true;
    }
    false
}

/// Um cartão por título. Histórico completo, não apenas episódios pela metade.
fn resumir_eventos(eventos: &[EventoHistorico]) -> Vec<ResumoHistorico> {
    let mut por_titulo: HashMap<(String, bool), ResumoHistorico> = HashMap::new();
    for e in eventos {
        let chave = (e.titulo.clone(), e.serie);
        let resumo = por_titulo.entry(chave).or_insert_with(|| ResumoHistorico {
            titulo: e.titulo.clone(), serie: e.serie, temporada: e.temporada,
            episodio: e.episodio, ultimo_em: e.quando, reproducoes: 0
        });
        resumo.reproducoes += 1;
        if e.quando >= resumo.ultimo_em {
            resumo.ultimo_em = e.quando;
            resumo.temporada = e.temporada;
            resumo.episodio = e.episodio;
        }
    }
    let mut lista: Vec<ResumoHistorico> = por_titulo.into_values().collect();
    lista.sort_by(|a, b| {
        let pontuacao = |x: &ResumoHistorico| x.ultimo_em as f64
            + (x.reproducoes as f64).ln_1p() * 6.0 * 60.0 * 60.0 * 1000.0;
        pontuacao(b).total_cmp(&pontuacao(a))
    });
    lista
}

/// A ordenação prioriza o recente, com uma bonificação pequena para títulos
/// abertos frequentemente, para manter sugestões úteis sem esconder novidades.
pub fn resumo_historico() -> Vec<ResumoHistorico> {
    resumir_eventos(&eventos_historico())
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

#[cfg(test)]
mod testes_episodios {
    use super::*;

    #[test]
    fn historico_agrupa_episodios_de_uma_mesma_serie_e_conta_vezes() {
        let e = |id: &str, serie: &str, t, ep, quando| EventoHistorico {
            id: id.into(), titulo: serie.into(),
            serie: true, temporada: t, episodio: ep, quando,
        };
        let eventos = vec![
            e("1", "American Dad!", 3, 5, 1000),
            e("2", "Família da Pesada", 2, 10, 1500),
            e("3", "American Dad!", 3, 6, 2000),
        ];
        let resumo = resumir_eventos(&eventos);
        assert_eq!(resumo.len(), 2);
        let american = resumo.iter().find(|r| r.titulo == "American Dad!").unwrap();
        assert_eq!(american.reproducoes, 2);
        assert_eq!((american.temporada, american.episodio), (3, 6));
    }

    #[test]
    fn importar_log_duas_vezes_nao_duplica_reproducoes() {
        let evento = EventoHistorico {
            id: "aparelho-1-evento-7".into(), titulo: "American Dad!".into(),
            serie: true, temporada: 3, episodio: 5, quando: 1234
        };
        let mut local = Vec::new();
        assert!(mesclar_eventos(&mut local, std::slice::from_ref(&evento)));
        assert!(!mesclar_eventos(&mut local, std::slice::from_ref(&evento)));
        assert_eq!(local.len(), 1);
        let json = serde_json::to_string(&evento).unwrap();
        let volta: EventoHistorico = serde_json::from_str(&json).unwrap();
        assert_eq!((volta.temporada, volta.episodio), (3, 5));
    }

    #[test]
    fn interpreta_temporadas_e_episodios_sem_confundir_series() {
        assert_eq!(numeros_do_episodio("American Dad", "American Dad · T12 E3"), Some((12, 3)));
        assert_eq!(numeros_do_episodio("Família da Pesada", "Família da Pesada · T10 E11"), Some((10, 11)));
        assert_eq!(numeros_do_episodio("American Dad", "Rick and Morty · T12 E3"), None);
        assert_eq!(numeros_do_episodio("American Dad", "American Dad · T1"), None);
    }

    #[test]
    fn usa_numeros_em_vez_de_ordem_alfabetica() {
        let marcas = HashMap::from([
            ("American Dad · T2 E9".to_string(), (240.0, 1200.0)),
            ("American Dad · T10 E1".to_string(), (300.0, 1200.0)),
            ("American Dad · T2 E10".to_string(), (320.0, 1200.0)),
        ]);
        let (chave, _) = marca_da_serie("American Dad", &marcas, None).unwrap();
        assert_eq!(chave, "American Dad · T10 E1");
    }

    #[test]
    fn prefere_o_episodio_visto_por_ultimo() {
        let marcas = HashMap::from([
            ("Família da Pesada · T10 E1".to_string(), (300.0, 1200.0)),
            ("Família da Pesada · T2 E5".to_string(), (400.0, 1200.0)),
        ]);
        let horarios = HashMap::from([
            ("Família da Pesada · T10 E1".to_string(), 100_i64),
            ("Família da Pesada · T2 E5".to_string(), 200_i64),
        ]);
        let (chave, _) = marca_da_serie("Família da Pesada", &marcas, Some(&horarios)).unwrap();
        assert_eq!(chave, "Família da Pesada · T2 E5");
    }
}
