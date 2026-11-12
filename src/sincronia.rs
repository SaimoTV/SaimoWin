//! Sincronia do Saimo Connect com o celular: onde parou em filmes e
//! episódios, títulos favoritos, canais favoritos, os dois endereços das
//! listas e as listas adicionadas. Mesmo formato da TV Box (`{"t":"dados"}`):
//!
//!   progresso:  {tipo:"f"|"s", titulo, ano, letra, temporada, episodio, pos, dur, em}
//!               pos e dur em milissegundos; em = quando foi visto (ms desde 1970)
//!   favTitulos: {tipo, titulo, ano, letra}
//!   favCanais:  nome do canal
//!   listas:     {canais, filmes, em, minhas: [{nome, link}]}
//!
//! Progresso: vale o mais recente. Favoritos: união. Endereços: vale o
//! escolhido por último; quem nunca escolheu fica com o que o outro escolheu.
//! Listas adicionadas: união pelo link.

use crate::{connect, listas, minhas_listas, progresso, radios, App};
use serde_json::{json, Value};

/// "Série · T1 E2" -> ("Série", 1, 2).
fn episodio(chave: &str) -> Option<(String, u32, u32)> {
    let (serie, resto) = chave.rsplit_once(" · T")?;
    let (t, e) = resto.split_once(" E")?;
    Some((serie.to_string(), t.parse().ok()?, e.parse().ok()?))
}

/// "Nome (2020)" -> ("Nome", "2020").
fn separar_ano(nome: &str) -> (String, String) {
    if let Some(abre) = nome.strip_suffix(')').and_then(|s| s.rfind(" (").map(|i| (s, i))) {
        let (s, i) = abre;
        let ano = &s[i + 2..];
        if ano.len() == 4 && ano.chars().all(|c| c.is_ascii_digit()) {
            return (s[..i].to_string(), ano.to_string());
        }
    }
    (nome.to_string(), String::new())
}

fn agora_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// A letra do arquivo do acervo onde o título mora.
fn letra(titulo: &str) -> String {
    match titulo.chars().next().map(|c| c.to_ascii_uppercase()) {
        Some(c) if c.is_ascii_uppercase() => c.to_string(),
        _ => "#".into(),
    }
}

impl App {
    pub(crate) fn pacote_de_dados(&self) -> Value {
        let achado = |titulo: &str| self.acervo.iter().find(|a| a.titulo == titulo);
        let mut lista = Vec::new();
        for (chave, pos, dur, em) in progresso::todas() {
            let (tipo, titulo, ano, temporada, numero) = match episodio(&chave) {
                Some((serie, t, e)) => {
                    let (titulo, ano) = separar_ano(&serie);
                    ("s", titulo, ano, t, e)
                }
                None => ("f", chave.clone(), String::new(), 0, 0),
            };
            let letra = achado(&titulo).map(|a| a.letra.clone()).unwrap_or_else(|| letra(&titulo));
            lista.push(json!({
                "tipo": tipo, "titulo": titulo, "ano": ano, "letra": letra,
                "temporada": temporada, "episodio": numero,
                "pos": (pos * 1000.0).round() as i64, "dur": (dur * 1000.0).round() as i64, "em": em,
            }));
        }
        let fav_titulos: Vec<Value> = self
            .favoritos_vod
            .iter()
            .map(|t| {
                let a = achado(t);
                json!({
                    "tipo": if a.is_some_and(|a| a.serie) { "s" } else { "f" },
                    "titulo": t,
                    "ano": a.map(|a| a.ano.clone()).unwrap_or_default(),
                    "letra": a.map(|a| a.letra.clone()).unwrap_or_else(|| letra(t)),
                })
            })
            .collect();
        let minhas: Vec<Value> = minhas_listas::listar()
            .into_iter()
            .filter_map(|l| l.link.map(|link| json!({"nome": l.nome, "link": link})))
            .collect();
        json!({
            "t": "dados",
            "progresso": lista,
            // Campo opcional novo: clientes antigos podem ignorar sem quebrar
            // o protocolo de progresso/favoritos da TV Box e do celular.
            "historico": progresso::eventos_historico(),
            "favTitulos": fav_titulos,
            "favCanais": self.favoritos,
            "favRadios": radios::favoritas(),
            "listas": {
                "canais": listas::canais().unwrap_or_default(),
                "filmes": listas::filmes().unwrap_or_default(),
                "em": listas::escolhidos_em(),
                "minhas": minhas,
            },
        })
    }

    /// Manda a sincronia ao celular conectado.
    pub(crate) fn mandar_dados(&self) {
        if connect::tem_celular() {
            connect::enviar_dados(&self.pacote_de_dados());
        }
    }

    /// Junta o que veio do celular ao que o Windows tem.
    pub(crate) fn aplicar_dados(&mut self, msg: &Value) {
        let num = |v: &Value, campo: &str| v.get(campo).and_then(Value::as_f64).unwrap_or(0.0);
        let texto = |v: &Value, campo: &str| v.get(campo).and_then(Value::as_str).unwrap_or("").to_string();

        if let Some(remotos) = msg.get("historico") {
            // Sincronização aditiva: IDs iguais não criam reproduções extras.
            if let Ok(eventos) = serde_json::from_value::<Vec<progresso::EventoHistorico>>(remotos.clone()) {
                progresso::importar_historico(&eventos);
            }
        }

        for p in msg.get("progresso").and_then(Value::as_array).into_iter().flatten() {
            let titulo = texto(p, "titulo");
            if titulo.is_empty() {
                continue;
            }
            let chave = if texto(p, "tipo") == "s" {
                let (t, e) = (num(p, "temporada") as u32, num(p, "episodio") as u32);
                if t == 0 || e == 0 {
                    continue;
                }
                // A série aqui é chamada pelo nome do acervo (com o ano quando ele tem).
                let ano = texto(p, "ano");
                let com_ano = format!("{titulo} ({ano})");
                let serie = if !ano.is_empty() && self.acervo.iter().any(|a| a.serie && a.titulo == com_ano) {
                    com_ano
                } else {
                    titulo
                };
                format!("{serie} · T{t} E{e}")
            } else {
                titulo
            };
            progresso::importar(&chave, num(p, "pos") / 1000.0, num(p, "dur") / 1000.0, num(p, "em") as i64);
        }

        let mut mudou_vod = false;
        for f in msg.get("favTitulos").and_then(Value::as_array).into_iter().flatten() {
            let titulo = texto(f, "titulo");
            if !titulo.is_empty() && !self.favoritos_vod.contains(&titulo) {
                self.favoritos_vod.push(titulo);
                mudou_vod = true;
            }
        }
        if mudou_vod {
            crate::gravar_lista("favoritos-vod.txt", &self.favoritos_vod);
        }

        let mut mudou_canais = false;
        for nome in msg.get("favCanais").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            if !nome.is_empty() && !self.favoritos.iter().any(|f| f == nome) {
                self.favoritos.push(nome.to_string());
                mudou_canais = true;
            }
        }
        if mudou_canais {
            crate::gravar_lista("favoritos.txt", &self.favoritos);
            self.reordenar();
        }

        if let Some(nomes) = msg.get("favRadios").and_then(Value::as_array) {
            let remotas: Vec<String> = nomes.iter().filter_map(Value::as_str).map(str::to_string).collect();
            if radios::importar_favoritas(&remotas) {
                self.versao_dados = self.versao_dados.wrapping_add(1);
            }
        }

        if let Some(l) = msg.get("listas") {
            self.aplicar_listas_recebidas(l);
        }
    }

    fn aplicar_listas_recebidas(&mut self, l: &Value) {
        let ler = |campo: &str| -> Option<Option<String>> {
            let v = l.get(campo)?.as_str()?;
            Some(if v.is_empty() { None } else { listas::normalizar(v).ok().flatten() })
        };
        if let (Some(canais), Some(filmes)) = (ler("canais"), ler("filmes")) {
            let em = l.get("em").and_then(Value::as_i64).unwrap_or(0);
            let local = listas::escolhidos_em();
            let (mut c, mut f) = (listas::canais(), listas::filmes());
            if em > local {
                c = canais;
                f = filmes;
            } else if local == 0 {
                // Aqui nunca se escolheu nada: fica o que o outro escolheu (e um
                // endereço que o outro não tem não apaga o daqui).
                if canais.is_some() {
                    c = canais;
                }
                if filmes.is_some() {
                    f = filmes;
                }
            }
            if c != listas::canais() || f != listas::filmes() {
                self.aplicar_listas(vec![(listas::Lista::Canais, c), (listas::Lista::Filmes, f)]);
                listas::definir_escolhidos_em(if em > local { em } else { agora_ms() });
            }
        }
        let ja: Vec<String> = minhas_listas::listar().into_iter().filter_map(|x| x.link).collect();
        let novas: Vec<(String, String)> = l
            .get("minhas")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let link = m.get("link").and_then(Value::as_str)?.trim().to_string();
                let nome = m.get("nome").and_then(Value::as_str).unwrap_or("").to_string();
                ((link.starts_with("http://") || link.starts_with("https://")) && !ja.contains(&link)).then_some((link, nome))
            })
            .collect();
        if !novas.is_empty() && !self.minhas_adicionando {
            // Todas numa thread só, em sequência (o limite de listas vale).
            self.minhas_em_fundo(None, true, move || {
                for (link, nome) in novas {
                    let _ = minhas_listas::adicionar(&link, &nome);
                }
                Ok(())
            });
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_a_chave_do_episodio() {
        assert_eq!(episodio("Ted Lasso · T1 E2"), Some(("Ted Lasso".into(), 1, 2)));
        assert_eq!(episodio("Matrix"), None);
    }

    #[test]
    fn separa_o_ano_do_nome() {
        assert_eq!(separar_ano("Dark (2017)"), ("Dark".into(), "2017".into()));
        assert_eq!(separar_ano("1984"), ("1984".into(), String::new()));
        assert_eq!(separar_ano("Nome (Dublado)"), ("Nome (Dublado)".into(), String::new()));
    }
}
