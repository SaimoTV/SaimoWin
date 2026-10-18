//! Os dois endereços estáveis de onde o app lê catálogo e acervo.
//!
//! Ficam num lugar só: mudar de conta no GitHub não pode quebrar quem já
//! instalou, então nada aqui aponta para o repositório.

/// `catalogo.txt`, `canais.txt`, `restritos.txt`, `radios.txt` e `versao.json`.
pub const CANAIS: &str = "https://saimo-canais.pages.dev/";
/// Espelho da pasta `vod/` (mesmos caminhos relativos).
pub const FILMES: &str = "https://saimo-filmes.pages.dev/";
