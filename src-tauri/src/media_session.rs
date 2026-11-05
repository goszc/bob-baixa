//! Taskbar thumbnail buttons and the Windows system media session.
//!
//! The webview media session does not put transport buttons on the taskbar
//! preview. These controls are bound to the window: previous, play/pause, next.

pub const PREV_ID: u32 = 41001;
pub const PLAY_ID: u32 = 41002;
pub const NEXT_ID: u32 = 41003;

const GLYPH: usize = 16;

#[cfg_attr(not(windows), allow(dead_code))]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
}

#[derive(Clone, Copy)]
pub enum Glyph {
    Play,
    Pause,
    Previous,
    Next,
}

pub fn command_action(id: u32) -> Option<&'static str> {
    match id {
        PREV_ID => Some("previous"),
        PLAY_ID => Some("toggle"),
        NEXT_ID => Some("next"),
        _ => None,
    }
}

/// White glyph on a transparent 16×16 bitmap, BGRA.
pub fn glyph_bgra(kind: Glyph) -> Vec<u8> {
    let mut pixels = vec![0u8; GLYPH * GLYPH * 4];
    for y in 0..GLYPH {
        for x in 0..GLYPH {
            if ink(kind, x, y) {
                let index = (y * GLYPH + x) * 4;
                pixels[index] = 255;
                pixels[index + 1] = 255;
                pixels[index + 2] = 255;
                pixels[index + 3] = 255;
            }
        }
    }
    pixels
}

fn ink(kind: Glyph, x: usize, y: usize) -> bool {
    let x = x as i32;
    let y = y as i32;
    match kind {
        Glyph::Play => triangle(x, y, 4, 12, true),
        Glyph::Pause => matches!(x, 5 | 6 | 9 | 10) && (4..=11).contains(&y),
        Glyph::Previous => {
            (matches!(x, 3 | 4) && (4..=11).contains(&y)) || triangle(x, y, 6, 12, false)
        }
        Glyph::Next => {
            (matches!(x, 11 | 12) && (4..=11).contains(&y)) || triangle(x, y, 3, 9, true)
        }
    }
}

fn triangle(x: i32, y: i32, left: i32, right: i32, points_right: bool) -> bool {
    if x < left || x > right || right == left {
        return false;
    }
    let span = right - left;
    let half = if points_right {
        (right - x) * 6 / span
    } else {
        (x - left) * 6 / span
    };
    half > 0 && (y - 8).abs() <= half
}

#[cfg(not(windows))]
pub type Artwork = ();

#[cfg(windows)]
pub type Artwork = Option<windows::Storage::Streams::RandomAccessStreamReference>;

#[cfg(not(windows))]
pub fn install(_app: &tauri::AppHandle) {}

#[cfg(not(windows))]
pub fn encode_artwork(_path: Option<&str>) -> Artwork {}

#[cfg(not(windows))]
pub fn apply(_info: NowPlaying, _artwork: Artwork) {}

#[cfg(windows)]
pub fn encode_artwork(path: Option<&str>) -> Artwork {
    win::encode_artwork(path)
}

#[cfg(windows)]
mod win {
    use super::{command_action, glyph_bgra, Glyph, NowPlaying, GLYPH, NEXT_ID, PLAY_ID, PREV_ID};

    const THBN_CLICKED: u32 = 0x1800;
    use std::sync::{Mutex, OnceLock};
    use tauri::{Emitter, Manager};
    use windows::core::{factory, w, HSTRING};
    use windows::Foundation::TypedEventHandler;
    use windows::Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    };
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RPC_E_CHANGED_MODE, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
        DIB_RGB_COLORS, HGDIOBJ, RGBQUAD,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
    use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
    use windows::Win32::UI::Shell::{
        DefSubclassProc, ITaskbarList3, SetWindowSubclass, TaskbarList, THBF_ENABLED, THB_FLAGS, THB_ICON,
        THB_TOOLTIP, THUMBBUTTON, THUMBBUTTONFLAGS, THUMBBUTTONMASK,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateIconIndirect, RegisterWindowMessageW, HICON, ICONINFO, WM_COMMAND,
    };

    struct Session {
        hwnd: HWND,
        controls: Option<SystemMediaTransportControls>,
        taskbar: Option<ITaskbarList3>,
        buttons: bool,
        playing: bool,
        play_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        pause_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        prev_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        next_icon: HICON,
    }

    // The window thread is the only caller. Win32 handles are not marked Send.
    unsafe impl Send for Session {}

    static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
    static SESSION: Mutex<Option<Session>> = Mutex::new(None);
    static CREATED: OnceLock<u32> = OnceLock::new();

    pub fn install(app: &tauri::AppHandle) {
        let _ = APP.set(app.clone());
        bind();
    }

    pub fn encode_artwork(path: Option<&str>) -> Option<RandomAccessStreamReference> {
        let path = path?;
        let bytes = std::fs::read(path).ok()?;
        if bytes.is_empty() || bytes.len() > 2_000_000 {
            return None;
        }
        artwork_stream(&bytes)
    }

    pub fn apply(info: NowPlaying, thumb: Option<RandomAccessStreamReference>) {
        bind();
        let Ok(mut guard) = SESSION.lock() else {
            return;
        };
        let Some(session) = guard.as_mut() else {
            return;
        };
        let playing_changed = session.playing != info.playing;
        session.playing = info.playing;
        if let Some(controls) = session.controls.clone() {
            let _ = push_metadata(&controls, &info, info.playing, thumb.as_ref());
        }
        if !session.buttons || playing_changed {
            let _ = sync_buttons(session);
        }
    }

    fn bind() {
        if SESSION.lock().map(|guard| guard.is_some()).unwrap_or(true) {
            return;
        }
        let Some(app) = APP.get() else {
            return;
        };
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let created = *CREATED.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) });
        unsafe {
            let _ = SetWindowSubclass(hwnd, Some(subclass), 41000, 0);
        }
        let controls = open_controls(app, hwnd);
        let taskbar = open_taskbar();
        let mut session = Session {
            hwnd,
            controls,
            taskbar,
            buttons: false,
            playing: false,
            play_icon: hicon(Glyph::Play),
            pause_icon: hicon(Glyph::Pause),
            prev_icon: hicon(Glyph::Previous),
            next_icon: hicon(Glyph::Next),
        };
        let _ = sync_buttons(&mut session);
        if let Some(controls) = session.controls.clone() {
            let _ = push_metadata(
                &controls,
                &NowPlaying {
                    title: "bob baixa".into(),
                    artist: String::new(),
                    album: String::new(),
                    playing: false,
                },
                false,
                None,
            );
        }
        let _ = created;
        if let Ok(mut guard) = SESSION.lock() {
            *guard = Some(session);
        }
    }

    fn open_taskbar() -> Option<ITaskbarList3> {
        unsafe {
            let taskbar: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_ALL).ok()?;
            taskbar.HrInit().ok()?;
            Some(taskbar)
        }
    }

    fn open_controls(app: &tauri::AppHandle, hwnd: HWND) -> Option<SystemMediaTransportControls> {
        let interop: ISystemMediaTransportControlsInterop =
            factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>().ok()?;
        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd).ok()? };
        controls.SetIsEnabled(true).ok()?;
        let _ = controls.SetIsPlayEnabled(true);
        let _ = controls.SetIsPauseEnabled(true);
        let _ = controls.SetIsNextEnabled(true);
        let _ = controls.SetIsPreviousEnabled(true);
        let _ = controls.SetIsStopEnabled(false);
        let app = app.clone();
        let handler = TypedEventHandler::new(
            move |_: windows::core::Ref<'_, SystemMediaTransportControls>,
                  args: windows::core::Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
                let Some(args) = args.as_ref() else {
                    return Ok(());
                };
                let button = args.Button()?;
                let action = if button == SystemMediaTransportControlsButton::Play {
                    "play"
                } else if button == SystemMediaTransportControlsButton::Pause {
                    "pause"
                } else if button == SystemMediaTransportControlsButton::Next {
                    "next"
                } else if button == SystemMediaTransportControlsButton::Previous {
                    "previous"
                } else {
                    return Ok(());
                };
                let _ = app.emit("media-control", action);
                Ok(())
            },
        );
        let _ = controls.ButtonPressed(&handler);
        Some(controls)
    }

    fn push_metadata(
        controls: &SystemMediaTransportControls,
        info: &NowPlaying,
        playing: bool,
        thumb: Option<&RandomAccessStreamReference>,
    ) -> windows::core::Result<()> {
        let updater = controls.DisplayUpdater()?;
        updater.ClearAll()?;
        updater.SetType(MediaPlaybackType::Music)?;
        let music = updater.MusicProperties()?;
        let title = if info.title.trim().is_empty() {
            "bob baixa"
        } else {
            info.title.trim()
        };
        music.SetTitle(&HSTRING::from(title))?;
        music.SetArtist(&HSTRING::from(info.artist.trim()))?;
        music.SetAlbumTitle(&HSTRING::from(info.album.trim()))?;
        if let Some(thumb) = thumb {
            updater.SetThumbnail(thumb)?;
        }
        updater.Update()?;
        controls.SetPlaybackStatus(if playing {
            MediaPlaybackStatus::Playing
        } else {
            MediaPlaybackStatus::Paused
        })?;
        Ok(())
    }

    fn sync_buttons(session: &mut Session) -> windows::core::Result<()> {
        let hwnd = session.hwnd;
        let already = session.buttons;
        let buttons = [
            thumb_button(PREV_ID, session.prev_icon, "anterior"),
            thumb_button(
                PLAY_ID,
                if session.playing { session.pause_icon } else { session.play_icon },
                if session.playing { "pausar" } else { "reproduzir" },
            ),
            thumb_button(NEXT_ID, session.next_icon, "próxima"),
        ];
        let added = {
            let Some(taskbar) = session.taskbar.as_ref() else {
                return Ok(());
            };
            unsafe {
                if already {
                    taskbar.ThumbBarUpdateButtons(hwnd, &buttons).is_ok()
                } else {
                    taskbar.ThumbBarAddButtons(hwnd, &buttons).is_ok()
                }
            }
        };
        if added {
            session.buttons = true;
        }
        Ok(())
    }

    fn thumb_button(id: u32, icon: HICON, tip: &str) -> THUMBBUTTON {
        let mut button = THUMBBUTTON {
            dwMask: THUMBBUTTONMASK(THB_ICON.0 | THB_TOOLTIP.0 | THB_FLAGS.0),
            iId: id,
            iBitmap: 0,
            hIcon: icon,
            szTip: [0; 260],
            dwFlags: THUMBBUTTONFLAGS(THBF_ENABLED.0),
        };
        for (index, unit) in tip.encode_utf16().take(259).enumerate() {
            button.szTip[index] = unit;
        }
        button
    }

    fn hicon(kind: Glyph) -> HICON {
        icon_from_bgra(&glyph_bgra(kind)).unwrap_or_default()
    }

    fn icon_from_bgra(pixels: &[u8]) -> windows::core::Result<HICON> {
        unsafe {
            let dc = GetDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: GLYPH as i32,
                    biHeight: -(GLYPH as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let color = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
            }
            let mask = CreateCompatibleBitmap(dc, GLYPH as i32, GLYPH as i32);
            let icon = CreateIconIndirect(&ICONINFO {
                fIcon: true.into(),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            })?;
            if !color.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(color.0));
            }
            if !mask.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(mask.0));
            }
            ReleaseDC(None, dc);
            Ok(icon)
        }
    }

    fn artwork_stream(bytes: &[u8]) -> Option<RandomAccessStreamReference> {
        thread_local! {
            static COM: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        let ready = COM.with(|cell| {
            if cell.get() {
                return true;
            }
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr.is_ok() {
                cell.set(true);
                return true;
            }
            let _ = RPC_E_CHANGED_MODE;
            false
        });
        if !ready {
            return None;
        }
        let stream = InMemoryRandomAccessStream::new().ok()?;
        let writer = DataWriter::CreateDataWriter(&stream).ok()?;
        writer.WriteBytes(bytes).ok()?;
        writer.StoreAsync().ok()?.join().ok()?;
        let _ = writer.DetachStream();
        stream.Seek(0).ok()?;
        RandomAccessStreamReference::CreateFromStream(&stream).ok()
    }

    unsafe extern "system" fn subclass(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if msg == WM_COMMAND {
            let command = (wparam.0 & 0xffff) as u32;
            let code = ((wparam.0 >> 16) & 0xffff) as u32;
            if code == THBN_CLICKED {
                if let Some(action) = command_action(command) {
                    if let Some(app) = APP.get() {
                        let _ = app.emit("media-control", action);
                    }
                    return LRESULT(0);
                }
            }
        }
        if let Some(created) = CREATED.get() {
            if msg == *created {
                if let Ok(mut guard) = SESSION.try_lock() {
                    if let Some(session) = guard.as_mut() {
                        let _ = sync_buttons(session);
                    }
                }
            }
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

#[cfg(windows)]
pub use win::{apply, install};

#[cfg(test)]
mod tests {
    use super::{command_action, glyph_bgra, Glyph, NEXT_ID, PLAY_ID, PREV_ID};

    #[test]
    fn thumbnail_buttons_map_to_player_actions() {
        assert_eq!(command_action(PREV_ID), Some("previous"));
        assert_eq!(command_action(PLAY_ID), Some("toggle"));
        assert_eq!(command_action(NEXT_ID), Some("next"));
        assert_eq!(command_action(1), None);
    }

    #[test]
    fn transport_glyphs_are_distinct_and_visible() {
        let play = glyph_bgra(Glyph::Play);
        let pause = glyph_bgra(Glyph::Pause);
        let previous = glyph_bgra(Glyph::Previous);
        let next = glyph_bgra(Glyph::Next);
        assert_eq!(play.len(), 16 * 16 * 4);
        assert!(opaque(&play) > 8);
        assert!(opaque(&pause) > 8);
        assert_ne!(play, pause);
        assert_ne!(previous, next);
        assert_eq!(play[3], 0);
    }

    fn opaque(pixels: &[u8]) -> usize {
        pixels.chunks(4).filter(|pixel| pixel[3] > 0).count()
    }
}
