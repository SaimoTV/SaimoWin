# Saimo TV para Windows

<p align="center">
  Saimo TV: <a href="https://github.com/gabrielsaimo/SaimoTV-Android">TV Box</a> · <a href="https://github.com/gabrielsaimo/Saimo-Cell-V2">Celular</a> · <b>Windows</b> · <a href="https://github.com/gabrielsaimo/SaimoPlayer">Mac e catálogo</a> · <a href="https://github.com/gabrielsaimo/Saimo-TV">Site</a> · <a href="https://github.com/gabrielsaimo">todos os apps</a>
</p>

Aplicativo nativo: janela, lista e letreiro desenhados pela GPU (egui sobre
OpenGL) e vídeo pelo mpv. Não há navegador nem página HTML em lugar nenhum.

Versão atual: **2.0.0**. O instalador sai no release único de
[SaimoPlayer](https://github.com/gabrielsaimo/SaimoPlayer/releases/latest), e o
programa avisa quando há versão nova.

## Visual

Mesma identidade da TV Box: azul-marinho com acento ciano, menu do topo com
todas as seções (Início, Ao vivo, Eventos, Filmes, Séries, Animes, Doramas, Favoritos)
em todas as telas, lista de canais com seções em chips e número do canal, e o
acervo sobre a imagem de fundo do app (`assets/fundo.jpg`). Filmes e Séries
abrem com a fileira "Em alta" e a grade vem dos lançamentos para trás.

## Abertura

Ao abrir, a marca animada (a mesma abertura aprovada, `assets/abertura.mp4`,
1920x1080, 4,6 s) toca na janela inteira pelo mpv do app, sem som e cobrindo
a tela, sobre o azul-marinho do fundo do vídeo; no fim a marca se dissolve no
azul e o app aparece por baixo. Clique ou qualquer tecla pula. O vídeo vai
dentro do executável e é gravado no Temp na primeira vez, então não há arquivo
para o pacote esquecer; se o mpv não começar em 1,5 s a abertura sai sozinha,
e o último canal só volta a tocar depois dela (`src/abertura.rs`).
`SAIMO_SEM_ABERTURA=1` abre sem ela.

## Tocando agora no Windows

O que está no ar aparece no quadro de mídia do Windows (junto do volume, na
tela de bloqueio e em programas que mostram o que está tocando): canal com a
logo e o programa do guia, filme e série com a logo do título, do TMDB, como
na TV Box (pt, senão en, senão sem idioma; a capa quando não há logo)
(`src/midia.rs`, `src/ficha.rs`). Filme e
série pausam por ali; ao vivo o quadro não oferece pausa.

## Novidades da 2.1.2

- **Multi tela**: dois canais ao vivo lado a lado (botão da barra ou tecla D).
  A tela em foco tem a borda colorida, o som, a lista e o teclado; clicar na
  outra (ou T) troca. Cada tela troca de fonte sozinha quando a dela cai. Cada
  mpv desenha num framebuffer próprio, copiado para a metade dele da janela.
- **Acervo leve**: a grade e as fileiras são montadas uma vez e guardadas até
  algo mudar (antes, filtrar e ordenar trinta mil títulos a cada quadro);
  as fileiras do Início só desenham os cartões à vista; as capas baixam por
  seis baixadores, a mais recente primeiro, em vez de uma thread por capa.
- Abrir filmes, séries ou qualquer seção do acervo para o que está tocando;
  voltar ao "Ao vivo" retoma o canal.
- Ir a Animes ou Doramas e voltar a Séries não deixa mais a série esperando
  para abrir; as coleções ficam guardadas e não são baixadas de novo.

## Novidades da 2.0

- **Legendas do OpenSubtitles** em filmes e séries (pt-BR, pt-PT, inglês, espanhol), com
  ajuste de sincronia, no menu A/V (`src/legendas.rs`); sem chave nem cadastro
- **Pular abertura e recapitulação** com os tempos do
  [TheIntroDB](https://theintrodb.org) (`src/pulos.rs`): o botão aparece só
  quando o trecho foi marcado para aquele episódio. Enter pula.
- **Próximo episódio** nos créditos, seguindo sozinho em 10 s. Enter assiste
  agora, Esc dispensa. Sem créditos marcados, o cartão sobe nos últimos 30 s.
- Episódio que acaba emenda no seguinte, na mesma versão (dublado/legendado).
- Fonte que "termina" nos primeiros 30 s é tratada como quebrada: vai para a
  próxima em vez de fechar.
- Abre no último canal assistido.
- Ao vivo não pausa: o botão de pausa e o Espaço só valem para filme e série.
- Trocar de canal não derruba mais a fonte nova: o fim do arquivo anterior
  (motivo STOP do mpv) era lido como queda, e o canal passava por todas as
  fontes em um segundo até dizer que estava fora do ar. `mpv::Vigia` só conta
  o fim do arquivo que está no ar, e por erro ou fim de verdade.
- Guia sem "No Data" no lugar do programa.

## Testes

```bash
cargo test                                      # lógica, sem rede
cargo test bateria -- --ignored --nocapture     # contra a rede de verdade
```

A bateria sonda a lista publicada (até três fontes por canal), uma amostra do
acervo e o TheIntroDB, e lista o que está fora e por quê. Serve para separar
"o app está quebrado" de "a fonte caiu".

Para ver as telas no Mac sem o mpv: `SAIMO_DEMO=1 SAIMO_ABA=filmes cargo run`.

| Peça | O que faz |
|---|---|
| `src/main.rs` | estado do app, teclado, troca de fonte |
| `src/tela.rs` | desenho: vídeo no fundo, lista e letreiro por cima |
| `src/mpv.rs` | ponte com o libmpv, carregado da DLL em tempo de execução |
| `src/abertura.rs` | a abertura animada ao abrir o programa |
| `src/midia.rs` | quadro de "tocando agora" do Windows: título e logo |
| `src/catalogo.rs` | a mesma lista publicada dos outros aplicativos |
| `src/telemetria.rs` | Saimo Monitor, plataforma `windows` |
| `src/atualizacao.rs` | versão nova no GitHub, download pelo navegador |
| `src/pulos.rs` | TheIntroDB: abertura, recapitulação e créditos |
| `src/legendas.rs` | OpenSubtitles pelo id do TMDB (via `vod/imdb/`) |
| `src/vod.rs`, `src/progresso.rs` | acervo e onde cada título parou |
| `src/eventos.rs` | agenda da embedtv e do monitor com os placares da ESPN |

## Versões

egui/eframe 0.36 (os painéis agora são desenhados dentro do `Ui` que o
`eframe::App::ui` recebe), ureq 3 para a rede e libloading 0.9 para carregar a
libmpv. Compila sem nenhum aviso, e o `cargo clippy` também passa limpo.

## Montar o pacote

Do próprio Mac, com o mingw-w64 (`brew install mingw-w64`):

```bash
cd "/Volumes/SSD 1TB/DEV/Saimo/SaimoWin" && ./empacotar.sh
```

Sai `dist/SaimoTV-Instalador.msi`: o executável, a `libmpv-2.dll` e o
instalador que põe os dois em `%LOCALAPPDATA%\Programs\Saimo TV`, com atalho
no menu Iniciar e na Área de Trabalho, ficha em "Aplicativos instalados" e
desinstalação pelo painel do Windows. Instala sem senha de administrador,
porque instala só para quem está usando o computador.

Precisa do `wixl` (`brew install msitools`). As telas do assistente estão em
`instalador/ui`, que é a cópia traduzida das do WiX. O `--baixar` pega a
biblioteca do mpv de novo (compilação oficial do shinchiro/mpv-winbuild-cmake).

## Rodar no Mac para conferir a interface

O binário do Mac serve para ver lista, busca e teclado sem um PC por perto:

```bash
cargo run --release
cargo test                                   # inclui a leitura do TheIntroDB
cargo check --target x86_64-pc-windows-gnu   # confere o alvo de verdade
```

`SAIMO_MPV` aponta para outra biblioteca do mpv e `SAIMO_SEM_TELEMETRIA=1`
impede que o teste vire aparelho no painel.

## Eventos

A mesma tela de Eventos da TV Box e do celular (menu do topo ou Tab depois de
Doramas). Duas fontes, buscadas juntas fora da thread do desenho: a agenda da
embedtv diz em que canal cada jogo passa, a do monitor (`/v1/eventos`) traz os
jogos dos canais de streaming (CazéTV, Disney+, Prime, Paramount+, GOAT, ge tv,
N Sports, SportyNet) sem repetir os da embedtv, e os placares da ESPN (23 ligas)
dizem se já começou, o placar e o tempo de jogo, e trazem escudos melhores.

- Só entra jogo cujo canal está na lista (o fim do endereço do player, sem
  hífen, contra o nome do canal sem espaço; igual primeiro, depois "contém").
  Canal adulto fica de fora; jogo sem canal ou encerrado sai.
- O jogo da ESPN é o dos mesmos times, em qualquer ordem (nome sem acento e sem
  siglas como FC, EC, SP), começando a menos de quatro horas.
- Escudos e logos do 365scores são pedidos em 256 px e qualidade máxima, em vez
  dos 24/34 px borrados que a agenda manda.
- Grupos: Ao vivo agora, Começa em breve (até 2 h), Hoje mais tarde, Amanhã e
  Próximos dias, com o resumo "N jogos nos canais da lista · M ao vivo".
- O cartão mostra a liga, o selo "AO VIVO" com o tempo de jogo (ou "21:30",
  "amanhã 16:00", "sáb 16:00"), os escudos (a inicial quando falta), o placar
  ou o X, e o canal. Clique ou Enter abre o canal ao vivo.
- Com a tela aberta, a agenda é buscada de novo a cada minuto.

Para conferir contra a rede: `cargo test agenda_com -- --ignored --nocapture`.

## Guia e acervo

O guia sai das mesmas fontes dos outros aplicativos: meuguia.tv para a TV aberta
e os canais grandes, o guia da própria Pluto TV casado pelo id que está no link
do canal, e dois feeds XMLTV para o resto. Fica guardado por seis horas.

Filmes e séries vêm do acervo publicado em `vod/`, fatiado por letra, com capa
do TMDB (a mesma busca pontuada do TV Box), favoritos e "continuar de onde parou".

A capa é procurada só para as linhas visíveis: a letra "A" tem 2.672 filmes.
