# changelog

## 0.4.0

- modo econômico: um toque baixa só o áudio em opus a cerca de 24 kbps, perto de 0,5 MB numa faixa de três minutos. faixas longas podem passar disso e soar como rádio. o mp4 continua nas outras predefinições e na busca
- analisar outra playlist não deixa a anterior empilhada. dá para fechar um card ou fechar todas, e só a playlist mais recente fica aberta
- apagar uma faixa remove o arquivo do disco, a entrada do histórico e a faixa da biblioteca, da fila e do que está tocando
- playlist baixada vai para uma subpasta com o nome dela
- biblioteca por faixas, artistas, álbuns e pastas, com busca. playlists locais, curtidas, recentes, rádio local, m3u e um manifesto simples
- fila de reprodução separada da biblioteca: tocar a seguir, adicionar e limpar. aleatório e repetir continuam valendo nessa fila
- a reprodução lembra o ponto da faixa e a última sessão
- faixas iguais (mesmo arquivo, mesmo id do youtube ou mesma música) não aparecem duas vezes
- letras: arquivo, .lrc ao lado, lrcmux, karalyr, lrclib e lyrics.ovh. o nome usado na busca tira o ruído de título do youtube
- crossfade de 2 ou 3 segundos, nivelamento leve, atalhos e teclas de mídia, timer de sono e a retrospectiva local (semana, mês, ano e 12 meses): minutos, sequência e tops. nada disso sai do computador
- no windows, a miniatura da barra de tarefas tem anterior, reproduzir ou pausar, e próxima. título, artista e capa acompanham a faixa, e as teclas de mídia do sistema usam esses controles

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
