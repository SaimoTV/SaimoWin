//! Os dois endereços de onde o app lê as listas: canais, e filmes e séries.
//!
//! Os dois endereços já vêm de fábrica (as duas constantes abaixo, o único
//! lugar do app com endereço de lista) e ativos. Quem usa pode trocá-los em
//! Ajustes > Listas; guardar vazio desliga a lista (marca "desativado" no
//! arquivo, diferente de "nunca mexeu", que é arquivo ausente). Desligada, não
//! há pedido de rede para a lista.

use std::path::PathBuf;
use std::sync::RwLock;

/// Endereço de fábrica da lista de canais.
pub const PADRAO_CANAIS: &str = "https://canais-saimo.pages.dev/";
/// Endereço de fábrica da lista de filmes e séries.
pub const PADRAO_FILMES: &str = "https://filmes-saimo.pages.dev/";
const DESATIVADO: &str = "desativado";

const ARQUIVO_CANAIS: &str = "link-canais.txt";
const ARQUIVO_FILMES: &str = "link-filmes.txt";
const MARCA_MIGRADO: &str = "migrado-sem-listas.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lista {
    Canais,
    Filmes,
}

impl Lista {
    fn arquivo(self) -> &'static str {
        match self {
            Lista::Canais => ARQUIVO_CANAIS,
            Lista::Filmes => ARQUIVO_FILMES,
        }
    }

    pub fn padrao(self) -> &'static str {
        match self {
            Lista::Canais => PADRAO_CANAIS,
            Lista::Filmes => PADRAO_FILMES,
        }
    }

    /// O arquivo que prova que o endereço serve a lista: `GET <base><arquivo>` deve dar 200.
    pub fn arquivo_de_teste(self) -> &'static str {
        match self {
            Lista::Canais => "catalogo.txt",
            Lista::Filmes => "indice.txt",
        }
    }
}

/// O endereço digitado não é um endereço http(s) válido.
#[derive(Debug, PartialEq, Eq)]
pub struct EnderecoInvalido;

/// Normaliza o que a pessoa digitou:
/// vazio = sem endereço (`Ok(None)`); sem `://` ganha `https://`; só http e
/// https; sem espaço no meio nem host vazio; sem `?query` e `#fragmento`;
/// o último trecho do caminho com ponto (um arquivo) é descartado; termina em `/`.
pub fn normalizar(entrada: &str) -> Result<Option<String>, EnderecoInvalido> {
    let texto = entrada.trim();
    if texto.is_empty() {
        return Ok(None);
    }
    if texto.chars().any(char::is_whitespace) {
        return Err(EnderecoInvalido);
    }
    let completo = if texto.contains("://") { texto.to_string() } else { format!("https://{texto}") };
    let (esquema, resto) = completo.split_once("://").ok_or(EnderecoInvalido)?;
    let esquema = esquema.to_ascii_lowercase();
    if esquema != "http" && esquema != "https" {
        return Err(EnderecoInvalido);
    }
    let resto = resto.split(['?', '#']).next().unwrap_or("");
    let (host, caminho) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    let sem_porta = host.rsplit_once(':').map(|(h, p)| if p.chars().all(|c| c.is_ascii_digit()) { h } else { host }).unwrap_or(host);
    if sem_porta.is_empty() || host.contains('@') || sem_porta.starts_with('.') {
        return Err(EnderecoInvalido);
    }
    let mut caminho = caminho.to_string();
    if !caminho.ends_with('/') {
        let inicio = caminho.rfind('/').map(|i| i + 1).unwrap_or(0);
        if caminho[inicio..].contains('.') {
            caminho.truncate(inicio);
        } else {
            caminho.push('/');
        }
    }
    Ok(Some(format!("{esquema}://{host}{caminho}")))
}

/// Em que ponto está a janela "Endereço das listas".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fase {
    Editando,
    Testando,
    /// Algum endereço não abriu: Tentar de novo ou Salvar mesmo assim.
    Falhou,
}

/// O estado da janela "Endereço das listas" (primeira abertura e Ajustes > Listas).
#[derive(Clone, Debug)]
pub struct Dialogo {
    pub canais: String,
    pub filmes: String,
    pub fase: Fase,
    /// Os endereços que não passaram no teste (ou nem são endereços).
    pub falhas: Vec<Lista>,
    /// Há algum endereço que nem é http(s) válido: não dá para "salvar mesmo assim".
    pub invalido: bool,
    pub focar: bool,
}

impl Dialogo {
    pub fn novo() -> Self {
        Dialogo {
            canais: canais().unwrap_or_default(),
            filmes: filmes().unwrap_or_default(),
            fase: Fase::Editando,
            falhas: Vec::new(),
            invalido: false,
            focar: true,
        }
    }
}

static LIDOS: RwLock<Option<(Option<String>, Option<String>)>> = RwLock::new(None);

fn pasta() -> PathBuf {
    crate::catalogo::pasta()
}

fn ler_do_disco(lista: Lista) -> Option<String> {
    let texto = std::fs::read_to_string(pasta().join(lista.arquivo())).ok();
    interpretar(texto.as_deref(), lista)
}

/// Sem arquivo (ou arquivo ilegível): o endereço de fábrica. Marca
/// "desativado": lista desligada. Senão, o endereço guardado.
fn interpretar(texto: Option<&str>, lista: Lista) -> Option<String> {
    let Some(texto) = texto else { return Some(lista.padrao().to_string()) };
    if texto.trim() == DESATIVADO {
        return None;
    }
    // Normaliza de novo: um arquivo editado à mão não pode quebrar a leitura.
    match normalizar(texto) {
        Ok(Some(base)) => Some(base),
        _ => Some(lista.padrao().to_string()),
    }
}

fn carregar_se_preciso() {
    if LIDOS.read().map(|l| l.is_some()).unwrap_or(true) {
        return;
    }
    let lidos = (ler_do_disco(Lista::Canais), ler_do_disco(Lista::Filmes));
    if let Ok(mut l) = LIDOS.write() {
        l.get_or_insert(lidos);
    }
}

/// O endereço-base (com `/` no fim) da lista, se a pessoa informou.
pub fn endereco(lista: Lista) -> Option<String> {
    carregar_se_preciso();
    let l = LIDOS.read().ok()?;
    let (canais, filmes) = l.as_ref()?;
    match lista {
        Lista::Canais => canais.clone(),
        Lista::Filmes => filmes.clone(),
    }
}

pub fn canais() -> Option<String> {
    endereco(Lista::Canais)
}

pub fn filmes() -> Option<String> {
    endereco(Lista::Filmes)
}

/// Guarda o endereço (já normalizado). `None` desliga a lista (marca
/// "desativado"); o endereço de fábrica volta a ser "nunca mexeu" (sem arquivo).
pub fn guardar(lista: Lista, base: Option<&str>) {
    carregar_se_preciso();
    let caminho = pasta().join(lista.arquivo());
    match base {
        Some(b) if b == lista.padrao() => {
            let _ = std::fs::remove_file(&caminho);
        }
        Some(b) => {
            let _ = std::fs::write(&caminho, b);
        }
        None => {
            let _ = std::fs::write(&caminho, DESATIVADO);
        }
    }
    if let Ok(mut l) = LIDOS.write() {
        let par = l.get_or_insert((None, None));
        match lista {
            Lista::Canais => par.0 = base.map(str::to_string),
            Lista::Filmes => par.1 = base.map(str::to_string),
        }
    }
}

/// Confere se o endereço serve a lista (200 no arquivo de teste). Rede: fora da thread do desenho.
pub fn testar(lista: Lista, base: &str) -> bool {
    crate::rede::existe(&format!("{base}{}", lista.arquivo_de_teste()))
}

/// Apaga do disco o que foi baixado daquela lista. Favoritos, progresso,
/// histórico e ajustes ficam.
pub fn apagar_dados(lista: Lista) {
    let pasta = pasta();
    match lista {
        Lista::Canais => {
            for nome in ["catalogo.txt", "canais.txt", "restritos.txt", "radios.txt", "guia.json", "guia-tabelas.txt"] {
                let _ = std::fs::remove_file(pasta.join(nome));
            }
        }
        Lista::Filmes => {
            let _ = std::fs::remove_file(pasta.join("destaques.txt"));
            let _ = std::fs::remove_dir_all(pasta.join("vod"));
        }
    }
}

/// Na primeira execução desta versão apaga os caches de lista das anteriores.
pub fn migrar() {
    let marca = pasta().join(MARCA_MIGRADO);
    if marca.exists() {
        return;
    }
    apagar_dados(Lista::Canais);
    apagar_dados(Lista::Filmes);
    let _ = std::fs::write(marca, "1");
}

#[cfg(test)]
mod testes {
    use super::*;

    fn n(s: &str) -> Result<Option<String>, EnderecoInvalido> {
        normalizar(s)
    }
    fn ok(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn padrao_sem_arquivo_desativado_com_marca() {
        assert_eq!(interpretar(None, Lista::Canais), ok(PADRAO_CANAIS));
        assert_eq!(interpretar(None, Lista::Filmes), ok(PADRAO_FILMES));
        assert_eq!(interpretar(Some("desativado"), Lista::Canais), None);
        assert_eq!(interpretar(Some("http://localhost:8101"), Lista::Canais), ok("http://localhost:8101/"));
        assert_eq!(interpretar(Some("lixo ftp://"), Lista::Filmes), ok(PADRAO_FILMES));
        assert_eq!(n(PADRAO_CANAIS), Ok(ok(PADRAO_CANAIS)));
        assert_eq!(n(PADRAO_FILMES), Ok(ok(PADRAO_FILMES)));
    }

    #[test]
    fn vazio_e_sem_endereco() {
        assert_eq!(n(""), Ok(None));
        assert_eq!(n("   "), Ok(None));
    }

    #[test]
    fn ganha_https_e_barra() {
        assert_eq!(n("saimo-canais.pages.dev"), Ok(ok("https://saimo-canais.pages.dev/")));
        assert_eq!(n("  http://localhost:8101  "), Ok(ok("http://localhost:8101/")));
        assert_eq!(n("localhost:8101/listas"), Ok(ok("https://localhost:8101/listas/")));
    }

    #[test]
    fn tira_arquivo_query_e_fragmento() {
        assert_eq!(n("https://x.dev/listas/catalogo.txt"), Ok(ok("https://x.dev/listas/")));
        assert_eq!(n("https://x.dev/catalogo.txt?v=2#topo"), Ok(ok("https://x.dev/")));
        assert_eq!(n("https://x.dev/a/b/?q=1"), Ok(ok("https://x.dev/a/b/")));
        assert_eq!(n("https://x.dev/v1.2/"), Ok(ok("https://x.dev/v1.2/")));
        assert_eq!(n("https://x.dev"), Ok(ok("https://x.dev/")));
    }

    #[test]
    fn rejeita_o_invalido() {
        assert_eq!(n("ftp://x.dev/"), Err(EnderecoInvalido));
        assert_eq!(n("file:///etc/passwd"), Err(EnderecoInvalido));
        assert_eq!(n("https://x .dev/"), Err(EnderecoInvalido));
        assert_eq!(n("https:///lista/"), Err(EnderecoInvalido));
        assert_eq!(n("https://:8080/"), Err(EnderecoInvalido));
        assert_eq!(n("https://"), Err(EnderecoInvalido));
    }
}
