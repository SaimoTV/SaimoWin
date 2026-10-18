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

O mesmo desenho da TV Box, refeito para mouse e teclado (`src/tela/`):

- **Menu do topo** igual em todas as telas: o ícone, Buscar, Início, Ao vivo,
  Eventos, On Demand, Favoritos, Rádios, a engrenagem dos Ajustes e o relógio.
  A aba da tela atual fica num fundo claro; a do mouse, branca com o texto
  escuro, como o foco da TV. Os ícones são os Material da TV Box, lidos do
  mesmo `pathData` e rasterizados no app (`src/tela/icones.rs`).
- **Páginas em fileiras** (Início, On Demand, Filmes, Séries, Animes, Doramas,
  Eventos, Rádios e a busca): o destaque em cima com a imagem larga do título
  em foco, a nota, o ano, as temporadas, os gêneros e a sinopse; embaixo, as
  fileiras de capas, de canais ao vivo, de categorias (colagem de capas com a
  cor de cada uma) e de jogos. Passar o mouse num cartão é o foco da TV; as
  setas das pontas rolam a fileira; o botão direito favorita.
- **On Demand**: as quatro categorias, "Continue assistindo", os destaques, os
  gêneros de filmes e o "Explorar" com cada acervo de A a Z. Cada categoria
  abre a página dela (lançamentos, um gênero por fileira com "Ver todos") e a
  grade de A a Z com busca, ordem (mais novos ou A a Z) e gêneros em pílulas.
- **Ao vivo**: o vídeo na janela inteira; a lista de canais por cima com as
  seções em pílulas, número, logo, programa e andamento; embaixo, o canal com
  o programa, o que vem a seguir e os botões redondos (canais, lista, guia,
  multi tela, fontes, áudio e legendas, preencher, som, janelinha, tela cheia).
  O guia cobre a tela com os canais de um lado e a programação do outro.
- **Player**: a logo do título (TMDB) sobre a linha do tempo, que engrossa no
  mouse e mostra para onde vai; os botões em pílula; e o painel de episódios
  (tecla E) com as temporadas e as imagens das cenas.
- **Ficha e página da série** como a da TV: fundo largo, capa, selos, botões,
  sinopse, elenco; na série, as temporadas em pílulas e os episódios em
  cartões com a imagem da cena, nome, duração, sinopse, andamento, a marca de
  visto e uma pílula por versão (dublado, legendado).
- **Rádios** (`src/radios.rs`): a lista `vod/radios.txt` do SaimoPlayer, nas
  fileiras da TV (ouvidas por último, por estilo e de A a Z). A rádio toca no
  mpv do vídeo só com o som e continua enquanto se navega; o nome dela fica
  perto do relógio, com o botão de parar.
- **Ajustes**: preencher a tela, velocidade, som, idioma preferido da legenda,
  procurar atualização, atalhos e tela cheia.

A escala acompanha a janela: o desenho é pensado para 1280 pontos de largura,
e uma janela maior (ou a tela cheia) aumenta tudo junto, em degraus de 5%.

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

## Placa de vídeo e falhas

O app não escolhe a placa de vídeo: em notebook com placa integrada e
dedicada, quem decide é o Windows (Configurações > Sistema > Tela > Elementos
gráficos). A 2.3.0 forçava a dedicada (`NvOptimusEnablement` /
`AmdPowerXpressRequestHighPerformance` exportados no `.exe`) e decodificava
com D3D11VA/NVDEC em todos os codecs; em alguns notebooks o app fechava
sozinho depois de alguns instantes, e só parava abrindo pela integrada. Voltou
a decodificação segura do mpv (`hwdec=auto-copy-safe`), sem forçar placa.

Se o app cair por um erro dele, a causa fica em `falhas.txt`, na pasta de
dados do app (`%APPDATA%\SaimoTV`). Queda dentro do driver de vídeo não passa
por ali: essa aparece no Visualizador de Eventos do Windows (Logs do Windows >
Aplicativo).

## Saimo Connect (controle pelo celular)

O app Saimo do celular acha este computador e manda nele exatamente como manda
na TV Box: o mesmo servidor do `Connect.kt`, na porta **8999**
(`src/connect/`, e o lado do app em `src/controle.rs`).

- **Descoberta**: o celular varre a rede pedindo `GET /info` a cada endereço
  (`"app": "saimo-tv"`, o id deste computador, gravado em
  `%APPDATA%\SaimoTV\connect-id.txt`, e o nome "Saimo TV - <nome do PC>").
  Não há anúncio mDNS: o app do celular não procura por ele, só pela varredura.
- **Permissão**: celular novo só entra depois de alguém clicar em **Permitir**
  na janela que abre aqui (o ícone pisca na barra de tarefas). Sem resposta em
  um minuto, fica de fora. O aprovado fica lembrado
  (`connect-celulares.txt`); **Ajustes > Saimo Connect > Esquecer todos os
  celulares** apaga a lista e derruba quem está ligado.
- **Controle remoto** (WebSocket `/ws`, escrito à mão como o da TV Box):
  trocar de canal (inclusive em cada metade da multi tela), ligar e desligar a
  multi tela, escolher a tela em foco, abrir filme ou episódio do acervo (pela
  chave `saimo|m|letra|título|ano`; fora do acervo, pelo endereço que o celular
  manda), pausar, avançar, trocar de fonte, volume, digitar na busca e as
  teclas do controle. As teclas do controle passam pelo mesmo caminho do
  teclado (`App::tecla`), então fazem o que o teclado faz — o código da lista
  restrita inclusive só vale digitado, e o estado nunca mostra canal escondido.
- **Espelho**: o estado (o que toca em cada tela, a fonte com referer e agente,
  o programa do guia, filme com posição e duração, volume, quem ouve no
  celular) vai para o celular a cada mudança.
- **Som da TV no celular**: o celular toca o mesmo canal sozinho e acerta o
  passo pela régua comum. Enquanto algum celular ouve, a cada segundo vai
  `{t:"pos", screens:[{idx, st, playing}]}` com o `st` de cada tela, e o
  `ping` devolve a hora daqui para o celular medir a diferença dos relógios.

### A régua comum com o mpv

A TV Box sabe pelo ExoPlayer qual trecho do HLS está tocando; o mpv não diz.
Por isso o ao vivo em HLS daqui toca pela mesma rota `/hls` que o celular usa
(`http://127.0.0.1:8999/hls?u=...&tv=<mpv>`):

1. a lista passa pelo `Historico` (o mesmo da TV Box): dois minutos de trechos
   lembrados e, sem marca de horário no canal, uma marca contínua por trecho a
   partir das durações reais. Celular e computador leem a mesma régua;
2. o app lê os primeiros bytes de cada trecho que o mpv vai tocar e guarda o
   primeiro carimbo de tempo (PTS) do vídeo dele;
3. o mpv abre o canal com `rebase-start-time=no`, então a posição dele
   (`time-pos`) é o próprio PTS. O trecho no ar é o de PTS logo antes da
   posição, e `st` = começo do trecho na régua + o que já tocou dele.

Os pedaços continuam vindo direto da origem: só a lista passa por aqui. Se a
lista não abrir pela rota, o mpv é redirecionado para o endereço original e o
canal toca como antes, só sem a régua. Filme usa a posição; rádio que não é
HLS e fonte que não é HLS ficam sem régua (o celular toca, sem acertar o passo).

Conferido no Mac contra canais reais da lista: o PTS lido pelo app é o mesmo
que o FFmpeg entrega pela lista da rota, e o `time-pos` do mpv 0.41 aberto
como o app abre cai sempre dentro do trecho certo — o `st` andou junto com o
relógio, com menos de 50 ms de variação em 30 s e passando por cinco trechos.

### Firewall do Windows

Na primeira vez que o Saimo TV abre, o Windows pergunta se ele pode usar a
rede. Marque **Redes privadas** e permita: sem isso o celular não acha o
computador (o resto do app, inclusive o ao vivo pela rota local, funciona do
mesmo jeito). Se a pergunta foi recusada: Segurança do Windows > Firewall e
proteção de rede > Permitir um aplicativo pelo firewall > Saimo TV > Privada.

Para testar num computador que já tem algo na porta 8999:
`SAIMO_CONNECT_PORTA=18999` (o celular só procura a 8999). `SAIMO_SEM_CONNECT=1`
abre o app sem o servidor. O servidor de verdade, com um cliente de fora:
`SAIMO_CONNECT_PORTA=18999 cargo test servidor_de_verdade -- --ignored --nocapture`.

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
- O episódio seguinte toca pela mesma fonte que vinha tocando: a do mesmo
  provedor (host), senão a da mesma posição, senão a ordem de sempre; as outras
  ficam de reserva. Vale por série, na sessão (`src/fonte_preferida.rs`).
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

Para ver as telas no Mac sem o mpv: `SAIMO_DEMO=1 SAIMO_ABA=ondemand cargo run`.
A foto de cada tela sai com `SAIMO_FOTO=arquivo.png SAIMO_FOTO_TELA=<tela>`,
onde a tela é `inicio`, `ondemand`, `filmes`, `explorar`, `eventos`, `radios`,
`buscar`, `ajustes`, `ficha`, `serie` (com `SAIMO_FOTO_ANTES=10`), `aovivo`,
`guia` ou `player` (estas três com `SAIMO_DEMO=1`).

| Peça | O que faz |
|---|---|
| `src/main.rs` | estado do app, teclado, troca de fonte |
| `src/tela/` | desenho: menu do topo, páginas em fileiras, grade, ao vivo, player, ficha e série |
| `src/tela/icones.rs` | os ícones Material da TV Box, rasterizados do `pathData` |
| `src/radios.rs` | as rádios da lista publicada, estilos e ouvidas por último |
| `src/mpv.rs` | ponte com o libmpv, carregado da DLL em tempo de execução |
| `src/connect/` | Saimo Connect: servidor na porta 8999, WebSocket, rota `/hls` e a régua comum |
| `src/controle.rs` | o que o celular manda fazer e o estado que volta para ele |
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
