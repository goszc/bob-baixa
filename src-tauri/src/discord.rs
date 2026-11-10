//! Discord Rich Presence through the Windows named pipe `\\.\pipe\discord-ipc-N`.

use std::sync::Mutex;

use bob_core::{activity, frame, handshake, pipe_path};

struct Presence {
    pipe: Option<std::fs::File>,
    client_id: String,
    nonce: u64,
}

static PRESENCE: Mutex<Presence> = Mutex::new(Presence {
    pipe: None,
    client_id: String::new(),
    nonce: 1,
});

pub fn update(client_id: &str, title: &str, artist: &str, playing: bool) -> Result<(), String> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err("cole o id do aplicativo do discord nas configurações.".into());
    }
    #[cfg(not(windows))]
    {
        let _ = (client_id, title, artist, playing, frame(0, &handshake("0")), activity(1, 1, "", "", false), pipe_path(0));
        return Err("o discord rich presence usa o pipe do windows.".into());
    }
    #[cfg(windows)]
    {
        let mut presence = PRESENCE.lock().map_err(|_| "discord ocupado".to_string())?;
        if presence.pipe.is_none() || presence.client_id != client_id {
            let mut file = open_pipe()?;
            let hello = frame(0, &handshake(client_id));
            use std::io::Write;
            file.write_all(&hello).map_err(|error| error.to_string())?;
            if let Ok(reader) = file.try_clone() {
                std::thread::spawn(move || {
                    use std::io::Read;
                    let mut reader = reader;
                    let mut buf = [0u8; 4096];
                    while reader.read(&mut buf).ok().filter(|size| *size > 0).is_some() {}
                });
            }
            presence.pipe = Some(file);
            presence.client_id = client_id.to_string();
        }
        presence.nonce += 1;
        let body = activity(std::process::id(), presence.nonce, title, artist, playing);
        use std::io::Write;
        presence
            .pipe
            .as_mut()
            .unwrap()
            .write_all(&frame(1, &body))
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

pub fn clear() {
    if let Ok(mut presence) = PRESENCE.lock() {
        presence.pipe = None;
        presence.client_id.clear();
    }
}

#[cfg(windows)]
fn open_pipe() -> Result<std::fs::File, String> {
    let mut last = "o discord não está aberto.".to_string();
    for index in 0..10 {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe_path(index)) {
            Ok(file) => return Ok(file),
            Err(error) => last = error.to_string(),
        }
    }
    Err(last)
}
