//! Pistas para quando o app "abre e fecha", e o modo seguro que se liga sozinho.
//!
//! Sem console no Windows, um fechamento sem aviso não deixava rastro nenhum:
//! pânico do Rust só ia para `falhas.txt`, e queda dentro do driver de vídeo ou do
//! mpv (que não é pânico) não ia para lugar algum. Aqui ficam três coisas, todas na
//! pasta de dados do app (`%APPDATA%\SaimoTV`):
//!
//! * `inicio.txt`: as etapas da abertura atual (e `inicio-anterior.txt`, a da vez
//!   anterior). A última linha diz até onde o app chegou.
//! * `falhas.txt`: também recebe a queda nativa (código, módulo e deslocamento — o
//!   mesmo que o Visualizador de Eventos do Windows mostra como "módulo com falha").
//! * o modo seguro: se a abertura anterior não passou de 90 segundos sem fechar
//!   direito, esta sobe com menos recursos, e a cada queda seguida tira mais um:
//!   1 = decodificação por software; 2 = sem os controles de mídia do Windows;
//!   3 = sem o Saimo Connect. Sessão de 90 segundos (ou fechamento normal) zera.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Quanto tempo uma abertura precisa durar para contar como boa.
const SESSAO_BOA: Duration = Duration::from_secs(90);
const TAMANHO_MAXIMO: usize = 40_000;

static NIVEL: AtomicU8 = AtomicU8::new(0);
static ANTERIOR: Mutex<Option<String>> = Mutex::new(None);

fn arquivo(nome: &str) -> PathBuf {
    crate::catalogo::pasta().join(nome)
}

fn hora() -> String {
    chrono::Local::now().format("%H:%M:%S%.3f").to_string()
}

/// Anota até onde a abertura chegou.
pub fn etapa(texto: &str) {
    let caminho = arquivo("inicio.txt");
    if std::fs::metadata(&caminho).map(|m| m.len() as usize > TAMANHO_MAXIMO).unwrap_or(false) {
        let _ = std::fs::remove_file(&caminho);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&caminho) {
        let _ = writeln!(f, "[{}] {texto}", hora());
    }
}

fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn ler_numero(nome: &str) -> u32 {
    std::fs::read_to_string(arquivo(nome)).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

/// O que o modo seguro desligou nesta abertura (0 = nada).
pub fn nivel() -> u8 {
    NIVEL.load(Ordering::Relaxed)
}

/// Como a abertura anterior terminou, se foi mal: a última etapa e a falha nativa.
pub fn queda_anterior() -> Option<String> {
    ANTERIOR.lock().ok().and_then(|g| g.clone())
}

/// Chamar uma vez, no começo do `main`.
pub fn iniciar() {
    let sessao = arquivo("sessao.txt");
    let mut quebras = ler_numero("quebras.txt");
    if sessao.exists() {
        // A abertura anterior não chegou a 90 segundos e não fechou direito.
        quebras += 1;
        let _ = std::fs::write(arquivo("quebras.txt"), quebras.to_string());
        let etapas = std::fs::read_to_string(arquivo("inicio.txt")).unwrap_or_default();
        let ultimas: Vec<&str> = etapas.lines().rev().take(6).collect();
        let falha = std::fs::read_to_string(arquivo("falhas.txt"))
            .ok()
            .and_then(|t| t.lines().last().map(str::to_string))
            .unwrap_or_default();
        let resumo = format!(
            "a abertura anterior terminou sem fechar (queda {quebras} seguida). Últimas etapas: {}. Última falha registrada: {}",
            ultimas.into_iter().rev().collect::<Vec<_>>().join(" | "),
            if falha.is_empty() { "nenhuma".into() } else { falha }
        );
        if let Ok(mut g) = ANTERIOR.lock() {
            *g = Some(resumo);
        }
    }
    let _ = std::fs::copy(arquivo("inicio.txt"), arquivo("inicio-anterior.txt"));
    let _ = std::fs::remove_file(arquivo("inicio.txt"));
    NIVEL.store(quebras.min(3) as u8, Ordering::Relaxed);
    let _ = std::fs::write(&sessao, agora().to_string());
    etapa(&format!("versão {} abrindo; modo seguro nível {}", crate::VERSAO, nivel()));

    std::thread::spawn(|| {
        std::thread::sleep(SESSAO_BOA);
        boa();
    });
    instalar_filtro_de_excecoes();
}

fn boa() {
    let _ = std::fs::remove_file(arquivo("sessao.txt"));
    let _ = std::fs::remove_file(arquivo("quebras.txt"));
}

/// O app fechou de forma normal.
pub fn encerrar() {
    etapa("fechou normalmente");
    boa();
}

/// A janela não pôde nem abrir: sem console, a pessoa só via o app sumir.
pub fn erro_fatal(mensagem: &str) {
    etapa(&format!("ERRO FATAL: {mensagem}"));
    let caminho = arquivo("falhas.txt");
    let antes = std::fs::read_to_string(&caminho).unwrap_or_default();
    let _ = std::fs::write(
        &caminho,
        format!("{antes}[{}] versão {} — não abriu: {mensagem}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), crate::VERSAO),
    );
    #[cfg(windows)]
    janela_de_erro(&format!(
        "O Saimo TV não conseguiu abrir a janela.\n\n{mensagem}\n\nO detalhe ficou em:\n{}\n\nSe o problema continuar, envie esse arquivo (falhas.txt e inicio.txt) e informe a placa de vídeo e a versão do driver.",
        arquivo("falhas.txt").display()
    ));
}

#[cfg(windows)]
fn janela_de_erro(texto: &str) {
    extern "system" {
        fn MessageBoxW(janela: *mut core::ffi::c_void, texto: *const u16, titulo: *const u16, tipo: u32) -> i32;
    }
    let texto: Vec<u16> = texto.encode_utf16().chain(Some(0)).collect();
    let titulo: Vec<u16> = "Saimo TV".encode_utf16().chain(Some(0)).collect();
    // MB_OK | MB_ICONERROR
    unsafe { MessageBoxW(core::ptr::null_mut(), texto.as_ptr(), titulo.as_ptr(), 0x10) };
}

/// Registra a queda nativa (acesso inválido dentro do driver ou do mpv, por exemplo),
/// que não passa pelo gancho de pânico do Rust.
#[cfg(windows)]
fn instalar_filtro_de_excecoes() {
    use core::ffi::c_void;

    #[repr(C)]
    struct RegistroDeExcecao {
        codigo: u32,
        flags: u32,
        encadeada: *mut RegistroDeExcecao,
        endereco: *mut c_void,
    }
    #[repr(C)]
    struct Ponteiros {
        registro: *mut RegistroDeExcecao,
        contexto: *mut c_void,
    }
    extern "system" {
        fn SetUnhandledExceptionFilter(filtro: Option<unsafe extern "system" fn(*mut Ponteiros) -> i32>) -> *mut c_void;
        fn GetModuleHandleExW(flags: u32, endereco: *const c_void, modulo: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(modulo: *mut c_void, nome: *mut u16, tamanho: u32) -> u32;
    }

    unsafe extern "system" fn filtro(p: *mut Ponteiros) -> i32 {
        let mut detalhe = String::from("queda nativa");
        if !p.is_null() && !(*p).registro.is_null() {
            let r = &*(*p).registro;
            let mut modulo: *mut c_void = core::ptr::null_mut();
            // FROM_ADDRESS | UNCHANGED_REFCOUNT
            let achou = GetModuleHandleExW(0x4 | 0x2, r.endereco, &mut modulo) != 0 && !modulo.is_null();
            let (nome, desloc) = if achou {
                let mut buf = [0u16; 260];
                let n = GetModuleFileNameW(modulo, buf.as_mut_ptr(), buf.len() as u32) as usize;
                let caminho = String::from_utf16_lossy(&buf[..n.min(buf.len())]);
                let arq = caminho.rsplit(['\\', '/']).next().unwrap_or("?").to_string();
                (arq, (r.endereco as usize).wrapping_sub(modulo as usize))
            } else {
                ("desconhecido".into(), r.endereco as usize)
            };
            detalhe = format!("queda nativa 0x{:08X} em {nome}+0x{desloc:X}", r.codigo);
        }
        let caminho = arquivo("falhas.txt");
        let antes = std::fs::read_to_string(&caminho).unwrap_or_default();
        let _ = std::fs::write(
            &caminho,
            format!("{antes}[{}] versão {} — {detalhe}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), crate::VERSAO),
        );
        etapa(&detalhe);
        0 // EXCEPTION_CONTINUE_SEARCH: o Windows segue com o tratamento normal.
    }
    unsafe { SetUnhandledExceptionFilter(Some(filtro)) };
}

#[cfg(not(windows))]
fn instalar_filtro_de_excecoes() {}
