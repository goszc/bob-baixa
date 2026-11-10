# changelog

## 0.6.0

- uma playlist, uma faixa ou as curtidas podem ficar disponíveis offline. a cópia local permanece mesmo com manter só no drive, e toca sem internet
- cada faixa do drive guarda os primeiros 15 segundos neste computador, então o play começa na hora enquanto o resto chega. a próxima da fila é preparada durante a atual
- ao abrir, o app confere letra, capa e ficha do que falta, com calma, e avisa o que completou
- configurações mostra o espaço do bob baixa, o que está offline e o que está só na nuvem, e liberar espaço limpa o cache sem apagar o offline
- apagar uma faixa do drive manda para a lixeira. dá para restaurar ou apagar de vez
- um link público do spotify ou do youtube vira uma lista para revisar. o que não achar no youtube aparece embaixo. o download econômico entra na fila
- rádio continua com faixas parecidas que já estão na biblioteca. descobrir mistura a biblioteca com músicas novas, buscadas e baixadas no modo econômico. dá para pular com não curti
- um download que termina entra na fila. dá para desligar ou mandar tocar a seguir
- a chave do gemini, de aistudio.google.com/apikey, fica guardada como o token do drive. o padrão é gemini-3.5-flash-lite, com gemini-3.1-flash-lite se o primeiro lotar, gemma-4-31b-it para lote e gemini-embedding-2 para busca. com ela, o app sugere o descobrir, limpa título de youtube, monta clima, traduz e explica a letra, escreve uma retrospectiva e monta uma fila a partir de uma frase. sem a chave, valem as regras e o histórico
- a transcrição gemini-3.5-transcribe gera um .lrc marcado como gerada por ia, no botão ou sozinha dentro de um limite diário
- buscar por descrição usa o embedding e, sem chave, as palavras da letra e dos títulos
- o volume da faixa é medido em lufs e aplicado na reprodução. o crossfade vai de 0 a 12 segundos. o timer de sono abaixa o volume antes de parar
- o discord pode mostrar ouvindo a faixa no bob baixa. o mini player fica sempre por cima
- gerar diagnóstico junta um zip na área de trabalho, sem chaves nem tokens
- ouvir no celular é uma página estática em docs/ouvir, para o github pages

## 0.5.4

- um mp4 que ficou só no drive toca o vídeo na capa, em vez de um quadrado cinza. o proxy encaminha o range e responde na hora, com um cache pequeno no disco
- a letra .lrc e a capa sobem junto com a faixa e ficam numa ficha local. sem o arquivo neste computador, a busca ainda tira "official video" e "(pt.1)" e mostra o que tentou
- em configurações, reparar biblioteca do drive refaz ficha, letra e capa do que já foi enviado

## 0.5.3

- o acesso do drive no windows só guarda o refresh token no gerenciador de credenciais. o access token fica na memória. sem o recurso nativo do keyring a leitura voltava vazia, a fila ficava em aguardando e o log parava em "conectado". se o gerenciador falhar, o refresh token vai para um arquivo cifrado com dpapi. sem token, o item vira erro em vez de esperar calado. nomes com caractere fullwidth sobem em json utf-8, e cada passo do envio entra no drive.log

## 0.5.2

- o envio ao drive mostra aguardando, o percentual, o link no drive ou o erro com tentar de novo. o google responde 308 quando o arquivo ainda não acabou, e o cliente tratava isso como redirecionamento, então o envio não terminava e a fila ficava parada. testar conexão mostra o e-mail e a cota. o registro fica em drive.log

## 0.5.1

- entrar com google abre a url inteira. no windows o `cmd /c start` cortava no `&` e o google dizia que faltava `response_type`

## 0.5.0

- antes de buscar a letra, o app tenta reconhecer a faixa: impressão digital do acoustid (com a chave que você cola em configurações) e ficha do musicbrainz. a capa oficial vem do cover art archive ou do itunes, e entra no arquivo
- no fim do download, a legenda manual do youtube vira `.lrc` quando parece letra. a automática, palavra por palavra, fica de fora
- a busca de letra aceita nome parecido, com e sem "feat.", com e sem artista, e uma diferença de duração. versão ao vivo, remix, acústico ou acelerada usa a letra da original se a daquela versão não existir
- no player, **buscar letra** lista candidatos e fixa o escolhido ao lado da faixa. dá para traduzir lado a lado e abrir o karaokê em tela cheia
- a biblioteca procura um trecho da letra. dá para revisar tags do musicbrainz, achar duplicatas e arquivos quebrados, e converter a pasta para o modo econômico (opus ~24 kbps), pulando o que já está econômico
- a fila de download fica no disco. queda de rede ou 429 espera e tenta de novo, também ao reabrir o app
- equalizador com predefinições e bandas, geral ou por faixa. atalhos globais para tocar, próxima, anterior e curtir, sem abrir a janela
- mix do dia a partir do que você ouviu neste computador
- google drive, se a compilação trouxer o cliente: **entrar com google** envia as faixas novas para `bob baixa/músicas/artista/álbum` e os vídeos para `bob baixa/vídeos`, com a letra ao lado. a biblioteca junta o disco e o drive, e a reprodução do drive passa pelo app. dá para manter só no drive. playlists, curtidas e a retrospectiva sincronizam num json. o token fica no gerenciador de credenciais do windows

## 0.4.1

- a biblioteca lista mp3, mp4 e os outros formatos que o player toca que já estão na pasta, também em subpastas. no windows a varredura gravava o caminho com `\\?\` e a lista ficava vazia

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
