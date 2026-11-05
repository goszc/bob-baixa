# changelog

## 0.3.0

- player para o que já foi baixado. toca áudio e vídeo, com a barra de agora tocando sempre visível. volume e mudo ficam nessa barra, e o último volume continua salvo
- letras das tags, de um .lrc ao lado ou do lrclib
- a biblioteca do player só mostra o que ele toca (mp3, mp4 e os outros formatos que já aceita) dentro da pasta escolhida em configurações
- na busca, dá para escolher mp3 ou mp4 antes de mandar para a fila
- se o youtube recusar as legendas com erro 429, o download tenta de novo com espera. se continuar limitado, o arquivo termina sem legendas e o app avisa, em vez de falhar
- licença mit

## 0.2.0

- vários downloads ao mesmo tempo. o padrão continua 2, e dá para escolher de 1 a 4. pausar, continuar e tentar de novo respeitam as vagas
- predefinições na hora de colar o link. já vêm música mp3, vídeo 4k e vídeo 1080, e dá para criar, editar e apagar as suas
- menu de contexto do windows, desligado por padrão: baixar link copiado com bob baixa. fica no seu usuário, sem pedir administrador, e sai quando desliga ou desinstala
- depois de atualizar, o app mostra o que há de novo uma vez. o mesmo texto fica em configurações
- correções: a fila não começa pelo item mais novo, pausar ou remover não deixa o processo órfão, yt-dlp não trava com a saída cheia, a checagem de update não dispara várias vezes, um download cortado do yt-dlp ou do ffmpeg não fica pela metade, e o instalador espera o app sair antes de se trocar

## 0.1.0

- primeira versão: baixar, fila, busca, conversão, monitor, histórico e atualização assinada
