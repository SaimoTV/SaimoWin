# Saimo TV para Windows e Linux

<p align="center">
  Saimo TV: <a href="https://github.com/SaimoTV/SaimoTV-Android">TV Box</a> · <a href="https://github.com/SaimoTV/Saimo-Cell-V2">Celular</a> · <b>Windows e Linux</b> · <a href="https://github.com/SaimoTV/SaimoPlayer">Mac e catálogo</a> · <a href="https://github.com/SaimoTV/Saimo-Site">Site</a> · <a href="https://github.com/SaimoTV">todos os apps</a>
</p>

Aplicativo nativo: janela, lista e letreiro desenhados pela GPU (egui sobre
OpenGL) e vídeo pelo mpv. Não há navegador nem página HTML em lugar nenhum.

Versão atual: **3.0.0**. O instalador sai no release único de
[SaimoPlayer](https://github.com/SaimoTV/SaimoPlayer/releases/latest), e o
programa avisa quando há versão nova, lendo o `versao.json` anexado à última release
(`src/atualizacao.rs`, independente das listas)
(só aceita links de release do GitHub).

O app não traz endereço de lista nenhum. Na primeira abertura as telas mostram
"Bem-vindo ao Saimo!" com **Adicionar lista** e **Endereços do Saimo**:

- **Adicionar lista** (`src/minhas_listas_tela.rs`): um link **M3U** (só ele já
  basta) ou **servidor + usuário + senha** (Xtream; usuário e senha são opcionais,
  só para listas que pedem login). Cole direto um link
  `…/get.php?username=…&password=…` e o app o reconhece como conta Xtream
  (`src/xtream.rs`): lê os canais pela API do servidor (`player_api.php`, com as
  categorias) ou, se ela não responde, pelo M3U. Cada canal ganha as fontes
  `.m3u8` e `.ts`. A conta é guardada como esse link, então viaja pelo Saimo
  Connect como qualquer outra lista.
- **Endereços do Saimo** (Ajustes > Listas, `src/listas.rs`): os dois endereços
  no formato do Saimo (o link novo é testado antes); salvar um campo em branco
  desliga aquela lista (arquivo `link-canais.txt`/`link-filmes.txt` com a marca
  `desativado`). Quem já usava um endereço o mantém. Na sincronia pelo Saimo
  Connect, quem nunca escolheu (`listas-em.txt` 0) adota o que o outro enviou.

Lista desligada: estado vazio e nenhum pedido de rede. No formato do Saimo, O endereço de canais serve `catalogo.txt`, `restritos.txt`, `radios.txt` e
`guia.txt` (tabelas do guia por canal); o de filmes serve a pasta `vod/`.

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
  grade de A a Z com busca, ordem (mais novos, A a Z, mais populares, mais bem
  avaliados), gêneros em pílulas e uma segunda régua com ano, idade
  (classificação indicativa) e nota mínima, do `detalhes.txt` do catálogo
  (`src/info_titulos.rs`).
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

## Placa de vídeo e diagnósticos

O sistema operacional define a placa de vídeo em computadores com GPUs
integrada e dedicada. O player usa a decodificação compatível do mpv
(`hwdec=auto-copy-safe`) para diferentes configurações de hardware.

Diagnósticos de abertura e erros ficam na pasta de dados do aplicativo
(`%APPDATA%\\SaimoTV` no Windows).

## Troca de fonte dos canais

- Os canais priorizam a última fonte reproduzida com sucesso
  (`src/fonte_preferida.rs`).
- A rota HLS (`/hls`) identifica listas indisponíveis e permite avançar
  para fontes alternativas.
- Playlists de manutenção são reconhecidas como fontes indisponíveis.
- Uma fonte que cai após reproduzir continuamente é reaberta antes da troca;
  as alternativas são tentadas conforme necessário.
- O Saimo abre na tela inicial e guarda o último canal para retorno ao Ao vivo.

## Gravação direta (DVR)

O gravador captura o fluxo original com FFmpeg em segmentos Matroska de
aproximadamente 30 segundos, sem reencodificação. Os segmentos ficam em
`Gravacoes/_Processando` até a montagem, verificação e publicação do MKV final.
Trechos concluídos podem ser recuperados após interrupções.

- **Manual:** gravação contínua ou com duração definida; o aplicativo mostra
  o nome e o caminho do arquivo validado ao finalizar e oferece **Abrir pasta**.
- **Agendada:** data, hora, duração e programa do guia. Agendamentos futuros e
  gravações em andamento ficam visíveis; conclusões aguardam confirmação (✓)
  ou exclusão da programação (✕). Nenhuma dessas ações apaga o vídeo.
  Agendamentos cancelados ou perdidos saem da lista.
- **Localização:** pasta configurável em Ajustes. Há um botão de pasta no player
  para acessar as gravações.
- **Execução:** até três gravações simultâneas. O Saimo e o computador
  precisam estar ligados no horário programado.

Consulte [DVR direto segmentado](docs/DVR-DIRETO-SEGMENTADO.md) para detalhes
de recuperação, dependências e limitações.

## Saimo Connect (controle pelo celular)

Sincronia pelo Saimo Connect (`{"t":"dados"}`, mesmo formato na TV Box, Mac, Windows e celular): ao conectar e a cada mudança vão onde parou em filmes e episódios, títulos favoritos, canais favoritos, os dois endereços das listas (vale o escolhido por último; quem nunca escolheu fica com o do outro aparelho) e as listas adicionadas por link (união; a que veio de arquivo fica só no aparelho). Código em `src/sincronia.rs`; o horário de cada progresso fica em `progresso-quando.json` e o dos endereços em `listas-em.txt`.

Fontes sem endereço à mostra: as listas de fontes (canais e filmes) mostram só
"Fonte N", o nome da fonte (ex.: "Temporada 3") e a qualidade; nunca o servidor
nem o endereço.

O app Saimo do celular acha este computador e manda nele exatamente como manda
na TV Box: o mesmo servidor do `Connect.kt`, na porta **8999**
(`src/connect/`, e o lado do app em `src/controle.rs`).

- **Descoberta**: o celular varre a rede pedindo `GET /info` a cada endereço
  (`"app": "saimo-tv"`, o id deste computador, gravado em
  `%APPDATA%\SaimoTV\connect-id.txt`, e o nome "Saimo TV - <nome do PC>").
  Não há anúncio mDNS: o app do celular não procura por ele, só pela varredura.
- **Permissão**: igual à TV Box, o celular na mesma rede entra direto, sem PIN
  nem pergunta na tela. Ele fica lembrado
  (`connect-celulares.txt`); **Ajustes > Saimo Connect > Esquecer todos os
  celulares** apaga a lista e derruba quem está ligado.
- **Resiliência da interface**: validação de limites UTF-8 em textos de
  catálogo e proteção de quadros de desenho com `catch_unwind`.
  Eventos de diagnóstico ficam disponíveis em `falhas.txt`.
- **Setas do celular = teclado**: setas, OK e Voltar entram como teclas de
  verdade no quadro seguinte (`teclas_do_celular` + `raw_input_hook`), então
  valem a lógica do app e a navegação de foco do egui, igual ao teclado físico.
  Ajustes (sem foco próprio) foca a primeira linha na primeira seta; a linha
  acende com o foco (`tela::aceso`). Só campo de texto conta como "escrevendo"
  (`text_edit_focused`): antes, qualquer botão em foco perdia o foco na seta.
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

## Multi tela, desempenho e reprodução

- **Multi tela:** dois canais ao vivo lado a lado (botão da barra ou tecla D).
  Cada tela tem fonte e controles próprios; clique ou tecla T altera o foco.
- **Acervo leve:** grades em cache, cartões visíveis renderizados sob demanda
  e imagens baixadas por uma fila limitada de trabalhadores.
- **Navegação:** abrir outra seção interrompe o vídeo em tela cheia;
  voltar a Ao vivo retoma o canal.
- **Legendas:** OpenSubtitles em filmes e séries, com sincronização no menu A/V.
- **Episódios:** marcas de abertura e recapitulação pelo TheIntroDB, sugestão
  para próximo episódio e continuidade automática opcional.
- **Fontes:** o episódio seguinte prioriza a origem do episódio atual;
  fontes alternativas são testadas quando necessário.
- **Ao vivo:** a tecla Espaço e Pausar funcionam somente em filmes e séries.
  Trocas de canal descartam eventos de parada da reprodução anterior.

## Minhas listas

Em Ajustes > Minhas listas a pessoa cola o link de uma lista `.m3u` comum ou toca em "Escolher arquivo"
(`.m3u`, `.m3u8`, `.txt`; até 30 MB); os
canais entram em Ao vivo na seção "Minhas listas" (até 10 listas, 20.000 canais
cada) e tocam pelo mpv como os demais, com referer, agente e filtros do app. A
lista fica só neste computador, em `SaimoTV/minhas-listas` (`indice.json` + um
arquivo por lista), abre sem rede e é baixada de novo ao abrir o app quando passa
de 12 h. Canais com grupo ou nome adulto (exceto "Adult Swim") vão para "Adulto",
que só aparece depois do desbloqueio; fontes ClearKey saem (o mpv não monta
licença). O seletor é o nativo do
Windows (`GetOpenFileNameW`, em `src/seletor.rs`, pela crate `windows` que o projeto já usa;
roda numa thread). Lista de arquivo não guarda caminho nem link, não se atualiza
sozinha e "Atualizar" reabre o seletor.
Código: `src/minhas_listas.rs` (leitor, download, armazenamento; testes contra
`tests/dados/edge.*` e, quando existe, o `br.m3u` do scratchpad) e
`src/minhas_listas_tela.rs` (a janela). `SAIMO_FOTO_TELA=minhas-listas` fotografa a janela.

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
| `src/dvr.rs`, `src/dvr_stream.rs` | agendamentos, gravação direta e recuperação |
| `src/mpv.rs` | ponte com o libmpv, carregado da DLL em tempo de execução |
| `src/connect/` | Saimo Connect: servidor na porta 8999, WebSocket, rota `/hls` e a régua comum |
| `src/controle.rs` | o que o celular manda fazer e o estado que volta para ele |
| `src/abertura.rs` | a abertura animada ao abrir o programa |
| `src/midia.rs` | quadro de "tocando agora" do Windows: título e logo |
| `src/catalogo.rs` | a mesma lista publicada dos outros aplicativos |
| `src/listas.rs` | os dois endereços de lista (padrão de fábrica, troca, desligar): normalização, teste, gravação, migração e apagar cache |
| `src/telemetria.rs` | Saimo Monitor, plataforma `windows` |
| `src/atualizacao.rs` | lê o `versao.json` da última release, download pelo navegador (só links de release do GitHub) |
| `src/pulos.rs` | TheIntroDB: abertura, recapitulação e créditos |
| `src/legendas.rs` | OpenSubtitles pelo id do TMDB (via `vod/imdb/`) |
| `src/vod.rs`, `src/progresso.rs` | acervo e onde cada título parou |
| `src/eventos.rs` | agenda do Saimo com os placares da ESPN |

## Versões

egui/eframe 0.36 (os painéis agora são desenhados dentro do `Ui` que o
`eframe::App::ui` recebe), ureq 3 para a rede e libloading 0.9 para carregar a
libmpv. A validação inclui testes automatizados e compilação release no Windows.

## Montar o pacote

Do próprio Mac, com o mingw-w64 (`brew install mingw-w64`):

```bash
./empacotar.sh
```

Sai `dist/SaimoTV-Instalador.msi`: o executável, a `libmpv-2.dll` e o
instalador que põe os dois em `%LOCALAPPDATA%\Programs\Saimo TV`, com atalho
no menu Iniciar e na Área de Trabalho, ficha em "Aplicativos instalados" e
desinstalação pelo painel do Windows. Instala sem senha de administrador,
porque instala só para quem está usando o computador.

Precisa do `wixl` (`brew install msitools`). As telas do assistente estão em
`instalador/ui`, que é a cópia traduzida das do WiX. O `--baixar` pega a
biblioteca do mpv de novo (compilação oficial do shinchiro/mpv-winbuild-cmake).

## Linux

O mesmo código do Windows, então a tela é a mesma em tudo: abas, fileiras,
Ao vivo com guia, On Demand com ficha e player, Rádios, Eventos, Ajustes,
Minhas listas e Saimo Connect. Muda só o que é do sistema:

| O quê | Windows | Linux |
|---|---|---|
| Vídeo | `libmpv-2.dll` ao lado do .exe | a libmpv do AppImage, ou `libmpv.so.2`/`.so.1` do sistema |
| OpenGL do mpv | `wglGetProcAddress` | `eglGetProcAddress` (Wayland e X11) ou `glXGetProcAddressARB` |
| Fonte | Segoe UI do sistema | Roboto Regular/Medium embutida (a mesma da TV Box e do Roku) |
| Dados | `%APPDATA%\SaimoTV` | `~/.local/share/SaimoTV` (`$XDG_DATA_HOME`) |
| Escolher arquivo de lista | `GetOpenFileNameW` | zenity, kdialog ou yad |
| Abrir link | `ShellExecuteW` | `xdg-open` |
| Atualização (`versao.json`) | `windows` | `linux` (x86_64) e `linux-arm64` |

Pacotes (`./empacotar-linux.sh`, roda no Mac):

- `SaimoTV-Linux-x86_64.AppImage` e `SaimoTV-Linux-aarch64.AppImage`: qualquer
  distro de 2022 para cá, já com a libmpv do Ubuntu 22.04. Fica do sistema o
  que conversa com o driver de vídeo (Mesa, X11, Wayland, DRM, VA-API), o som,
  as fontes e a libstdc++: levar versão velha deles quebra o vídeo de quem tem
  distro nova. As bibliotecas levadas se acham pelo `RUNPATH=$ORIGIN`, sem
  `LD_LIBRARY_PATH`, que vazaria para o navegador e o seletor de arquivo.
- `saimo-tv_<versão>_amd64.deb` e `_arm64.deb` (no release:
  `SaimoTV-Linux-amd64.deb`/`-arm64.deb`): Ubuntu, Debian, Mint, Pop!_OS.
  Usam a libmpv da distribuição (`Depends: libmpv2 | libmpv1`).

O binário é compilado com `cargo zigbuild` para glibc 2.31 (Ubuntu 20.04,
Debian 11) e só depende da libc: janela, OpenGL e mpv são carregados na hora.
Precisa de `cargo install cargo-zigbuild`, `brew install zig squashfs`,
`rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu`, e de
dois insumos fora do git (pasta `mpv/`):

- `mpv/linux-x86_64/` e `mpv/linux-aarch64/`: saem de
  `scripts/montar_mpv_linux.sh`, rodado num Linux com debootstrap (a VM de
  teste: `limactl shell saimo-linux`).
- `mpv/appimage/runtime-x86_64` e `runtime-aarch64`: o runtime oficial do
  AppImage (github.com/AppImage/type2-runtime, release `continuous`).

As duas versões da libmpv valem: o app lê `mpv-version` e só manda o
argumento `index` do `loadfile` do mpv 0.38 em diante, e a estrutura de
inicialização do OpenGL leva o campo `extra_exts` que a libmpv 1.x lê.
`SAIMO_MPV_LOG=arquivo` grava o log do próprio mpv.

Testado numa VM Ubuntu 26.10 ARM64 (Lima, Xvfb): `.deb` e AppImage abrem,
canal ao vivo e filme tocam, teclado e mouse respondem; o x86_64 num Ubuntu
22.04 amd64 emulado abre e carrega a libmpv do pacote.

## Rodar no Mac para conferir a interface

O binário do Mac serve para ver lista, busca e teclado sem um PC por perto:

```bash
cargo run --release
cargo test                                   # inclui a leitura do TheIntroDB
cargo check --target x86_64-pc-windows-gnu   # confere o alvo de verdade
```

`SAIMO_MPV` aponta para outra biblioteca do mpv e `SAIMO_SEM_TELEMETRIA=1`
impede que o teste vire aparelho no painel.

## Telemetria por amostra

O monitor é cobrado por pedido, então só uma fatia estável dos aparelhos fala
com ele (`src/telemetria.rs`). A conta é a mesma do servidor (`amostrado` no
`worker.js`): FNV-1a de 32 bits sobre o `deviceId` em minúsculas, e entra quem
tem `hash % 10000 < amostraPct * 100`. O percentual vem de `telemetria.json`
(arquivo estático do monitor, de graça), relido no máximo uma vez por dia e
guardado em `amostra.json` na pasta do app; sem leitura vale 5%. O `/v1/hello`
também pode mandar `amostraPct`/`amostra`.

- Fora da amostra: nada vai ao monitor (nem olá, nem batida, nem evento), menos
  um relatório de quebra (`crash`), que vale de todos.
- Na amostra: como antes, mas a batida extra (pausa, troca de título, parada)
  sai no máximo uma a cada 30 s; a do relógio segue o `heartbeatSeconds` do
  olá, e a do fechamento sai sempre.
- `/v1/fontes` (servidores desligados): no máximo uma leitura por hora, com a
  última lista boa guardada em `fontes_desativadas.json` — reabrir o app dentro
  da hora não pergunta de novo.
- `/v1/eventos`: ver Eventos abaixo.

Teste da conta: `cargo test telemetria`.

## Eventos

A mesma tela de Eventos da TV Box e do celular (menu do topo ou Tab depois de
Doramas). Duas fontes, buscadas juntas fora da thread do desenho: a agenda do
Saimo (`eventos-extra.json`) diz em que canal cada jogo passa (nenhum site de
streaming de terceiros é consultado), e os placares da ESPN (23 ligas)
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
- Só busca com a tela aberta e o app na frente: na hora ao abrir (se a última
  busca tem mais de um minuto) e depois a cada três minutos.

Para conferir contra a rede: `cargo test agenda_com -- --ignored --nocapture`.

## Guia e acervo

O guia sai das mesmas fontes dos outros aplicativos: meuguia.tv para a TV aberta
e os canais grandes, o guia da própria Pluto TV casado pelo id que está no link
do canal, e dois feeds XMLTV para o resto. Fica guardado por seis horas.

Filmes e séries vêm do acervo publicado em `vod/`, fatiado por letra, com capa
do TMDB (a mesma busca pontuada do TV Box), favoritos e "continuar de onde parou".

A capa é procurada só para as linhas visíveis: a letra "A" tem 2.672 filmes.


## Diagnósticos e modo seguro

A pasta de dados do app (`%APPDATA%\SaimoTV`) guarda as pistas:

- `inicio.txt` (e `inicio-anterior.txt`): as etapas da abertura, com a placa de vídeo e a versão do
  OpenGL que a janela recebeu. A última linha diz até onde o app chegou.
- `falhas.txt`: pânico do Rust, queda nativa (`queda nativa 0xC0000005 em nvoglv64.dll+0x...`, o mesmo
  "módulo com falha" do Visualizador de Eventos) e erro ao abrir a janela (que também mostra uma caixa
  de aviso, em vez de o app só sumir).
- Modo seguro automático: se a abertura anterior não chegou a 90 segundos sem fechar direito, a
  seguinte sobe com menos recursos. Cada queda seguida tira mais um: 1 = decodificação por software
  (`hwdec=no`); 2 = sem os controles de mídia do Windows; 3 = sem o Saimo Connect. Uma abertura de 90
  segundos (ou um fechamento normal) zera a contagem. A queda anterior vai ao monitor (evento `crash`).
