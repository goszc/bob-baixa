# bob baixa

baixador de vídeos para windows. cola um link, vê a miniatura e baixa. o motor é o yt-dlp, com ffmpeg para juntar e converter. na primeira abertura o programa obtém os dois sozinho.

funciona com youtube e com os outros sites do yt-dlp: um vídeo, uma playlist ou um canal.

## instalar

1. execute `bob-baixa-setup.exe`
2. o windows pode mostrar o smartscreen, porque o instalador não é assinado. em "mais informações", escolha "executar assim mesmo"
3. deixe marcado o atalho da área de trabalho se quiser
4. abra o bob baixa pelo menu iniciar
5. na primeira vez, com internet, ele baixa o yt-dlp e o ffmpeg (cerca de 100 mb). o windows 11 já traz o webview2. no windows 10, se a janela não abrir, instale o [webview2](https://developer.microsoft.com/microsoft-edge/webview2/)

o programa fica em `%LOCALAPPDATA%\bob baixa`. desinstalar está no menu iniciar, em bob baixa → desinstalar. a pasta de downloads e as configurações não são apagadas.

## usar

- cole um ou vários links. o título e a miniatura aparecem sozinhos. **baixar** manda para a fila com 1080p em mp4
- playlists e canais viram vários itens
- em **mais opções**: qualidade, formato, só áudio (mp3, m4a, opus, flac), recorte (`1:20` até `3:45`), limite de velocidade
- a fila mostra progresso, velocidade e tempo restante. dá para pausar, continuar, cancelar e tentar de novo. pausar retoma de onde parou
- dá para agendar o início da fila. o app precisa estar aberto ou na bandeja
- **buscar** pesquisa no youtube
- **converter** transforma um arquivo que já está no computador
- **monitor** acompanha canais e playlists e baixa vídeos novos enquanto o app está aberto
- **histórico** guarda o que já foi feito
- configurações: pasta, modelo do nome, downloads ao mesmo tempo, legendas, miniatura, metadados, capítulos, sponsorblock, tema (sistema, claro ou escuro) e idioma
- cookies em **auto**: se o vídeo pedir login ou tiver restrição de idade, tenta de novo com o navegador instalado (chrome, edge, brave, firefox, opera, vivaldi ou chromium). se aparecer "feche o navegador e tente de novo", feche o navegador
- `ctrl+shift+b` pega o link copiado e começa a baixar
- soltar um link ou um `.txt` na janela adiciona os endereços. soltar um vídeo abre o conversor
- ao fechar, o app vai para a bandeja se essa opção estiver ligada

### extensão do chrome e do edge

a pasta `extension` fica ao lado do programa (e neste repositório).

1. abra `chrome://extensions` ou `edge://extensions`
2. ative o modo do desenvolvedor
3. carregar sem compactação e escolha a pasta `extension`

na página de um vídeo do youtube aparece **baixar com bob baixa**. o app escuta em `http://127.0.0.1:47821` e no protocolo `bobbaixa://`.

## compilar

precisa de rust estável, node 22 e, para o instalador, o nsis.

no windows:

```bash
npm ci
npm run tauri build -- --bundles none
makensis /DEXE=src-tauri\target\release\bob-baixa.exe /DEXTDIR=extension /DICON=src-tauri\icons\icon.ico packaging\bob-baixa.nsi
```

o instalador sai em `packaging/bob-baixa-setup.exe`.

no linux, o mesmo resultado com o alvo msvc:

```bash
sudo apt install nsis lld llvm clang
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin
npm ci
npm run tauri build -- --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles none
makensis -DEXE=src-tauri/target/x86_64-pc-windows-msvc/release/bob-baixa.exe -DEXTDIR=extension -DICON=src-tauri/icons/icon.ico packaging/bob-baixa.nsi
```

o workflow `.github/workflows/windows.yml` faz esse instalador num runner windows.

testes do motor:

```bash
cargo test -p bob-core
```

interface em desenvolvimento:

```bash
npm run tauri dev
```

## atualizações

com **atualizar sozinho** ligado (é o padrão), o app:

- ao abrir e a cada 3 horas, roda `yt-dlp -U` em silêncio
- confere o ffmpeg nas versões do [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds/releases) e baixa de novo se a publicação for mais nova
- se um download falhar com erro de extrator, atualiza o yt-dlp e tenta outra vez
- procura uma versão nova do próprio app. se achar, baixa o instalador, confere a assinatura e mostra **nova versão disponível. reiniciar?**

em configurações, **verificar agora** faz a mesma checagem na hora. o endereço das atualizações aponta para um `latest.json`. vazio usa a variável `BOB_UPDATE_URL` da compilação.

### publicar uma versão

1. a chave pública já está em `packaging/updater.pub`. a chave privada deste build não entra no git. guarde-a como segredo `TAURI_SIGNING_PRIVATE_KEY` (o conteúdo do arquivo) e, se houver senha, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
2. suba a versão em `package.json`, `src-tauri/tauri.conf.json` e `src-tauri/Cargo.toml`
3. crie uma tag `v1.0.1` e envie. o workflow `.github/workflows/windows.yml` gera o instalador, assina com `tauri signer sign` e publica `bob-baixa-setup.exe` e `latest.json` na release
4. o endereço que o app consulta é `https://github.com/<dono>/<repo>/releases/latest/download/latest.json`. coloque esse endereço em **endereço das atualizações**, ou compile com `BOB_UPDATE_URL` apontando para ele

sem o segredo da chave, o workflow ainda gera o instalador, mas não publica o manifesto assinado. uma atualização sem assinatura válida é ignorada.

## o que entra no programa

yt-dlp e ffmpeg não vão dentro do instalador. o app baixa a versão atual na primeira execução e o botão **atualizar yt-dlp** usa o atualizador do próprio yt-dlp.
