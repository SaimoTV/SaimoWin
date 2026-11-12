# Gravação direta e agendamento — SaimoWin

O Saimo TV grava a transmissão original com FFmpeg (`-c copy`), sem
captura de tela nem reencodificação de áudio ou vídeo.

## Organização e publicação

A pasta `Gravacoes` contém os arquivos MKV publicados. Durante o trabalho,
os segmentos ficam isolados em `_Processando`:

```text
Gravacoes/
  Canal_2026-11-12_02-54-38.mkv
  _Processando/
    Outro Canal_2026-11-12_03-00-00/
      parte_00000.mkv
      parte_00001.mkv
      partes.ffconcat
      sessao.json
      tentativa_1.ffmpeg.log
```

Cada segmento MKV tem aproximadamente 30 segundos, conforme os keyframes
da fonte. Ao encerrar, o Saimo reúne os trechos, valida a presença de
vídeo e áudio com ffprobe e publica o MKV com nome exclusivo. Só então
remove os temporários conhecidos. Arquivos que não pertençam à sessão
são preservados. Sem validação, os segmentos e logs permanecem na pasta
de processamento para recuperação. Arquivos finais existentes nunca
são sobrescritos.

## Gravação manual

O botão de gravação no player permite iniciar sem limite de tempo ou
definir uma duração. O estado visual distingue preparação, gravação,
processamento, conclusão e erro.

Depois da publicação validada, o aplicativo exibe o nome e o caminho
completo do arquivo com a opção **Abrir pasta**. No Windows, o Explorer
destaca o arquivo finalizado quando disponível; no Linux, a pasta
é aberta pelo gerenciador de arquivos. O botão de pasta ao lado do
controle de gravação também abre o destino.

## Gravações agendadas

O agendador recebe data, hora, duração e, opcionalmente, um programa do
guia. São permitidas durações entre 1 e 480 minutos e agendamentos até
90 dias no futuro. Até três gravações podem ocorrer simultaneamente.

A lista mostra os próximos agendamentos, gravações em andamento e
**conclusões que aguardam confirmação**. Uma gravação concluída fica na
lista até o usuário clicar em ✓ (confirmar) ou ✕ (remover o registro).
Esses controles não excluem o arquivo gravado. Cancelamentos,
agendamentos perdidos e execuções sem arquivo final válido não
permanecem na programação.

O computador precisa estar ligado e o Saimo aberto no horário
programado. Não é necessário manter o canal visível durante a gravação.

## Interrupções e recuperação

Na próxima abertura, o gravador identifica sessões interrompidas e tenta
montar os segmentos disponíveis. Uma queda de energia pode comprometer
o segmento que ainda estava sendo escrito; a recuperação integral não
é garantida. Falhas de validação mantêm trechos e logs recuperáveis.

Streams HLS fornecidos com sufixo `.txt` usam detecção explícita do
formato. Há uma tentativa alternativa com extensões permitidas
restritas para streams que usam nomes de segmento atípicos, incluindo
`.woff`. O gravador não contorna DRM nem URLs expiradas.

## Dependências e diretório

Disponibilize `ffmpeg` e `ffprobe` compatíveis ao lado do executável
ou no PATH. O caminho das gravações é configurado em **Ajustes**.
A variável `SAIMO_GRAVACOES_DIR` permite definir um destino absoluto
por dispositivo. A gravação utiliza o fluxo original independentemente
da reprodução visual do player.
