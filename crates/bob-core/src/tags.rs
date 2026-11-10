use std::path::Path;

/// Write title, artist, album and an optional JPEG cover. Existing lyrics stay.
pub fn write_track_tags(
    path: &Path,
    title: &str,
    artist: &str,
    album: &str,
    cover_jpeg: Option<&[u8]>,
) -> Result<(), String> {
    write_release_tags(path, title, artist, album, 0, cover_jpeg)
}

pub fn write_release_tags(
    path: &Path,
    title: &str,
    artist: &str,
    album: &str,
    track: u32,
    cover_jpeg: Option<&[u8]>,
) -> Result<(), String> {
    use lofty::config::WriteOptions;
    use lofty::picture::{MimeType, Picture, PictureType};
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let mut file = Probe::open(path)
        .map_err(|error| error.to_string())?
        .guess_file_type()
        .map_err(|error| error.to_string())?
        .read()
        .map_err(|error| error.to_string())?;
    let tag_type = file.primary_tag_type();
    if file.primary_tag().is_none() {
        file.insert_tag(lofty::tag::Tag::new(tag_type));
    }
    let tag = file.primary_tag_mut().ok_or_else(|| "sem tag".to_string())?;
    if !title.trim().is_empty() {
        tag.set_title(title.trim().to_string());
    }
    if !artist.trim().is_empty() {
        tag.set_artist(artist.trim().to_string());
    }
    if !album.trim().is_empty() {
        tag.set_album(album.trim().to_string());
    }
    if track > 0 {
        tag.set_track(track);
    }
    if let Some(bytes) = cover_jpeg.filter(|bytes| bytes.len() > 32) {
        let mime = if bytes.starts_with(b"\x89PNG") {
            MimeType::Png
        } else {
            MimeType::Jpeg
        };
        let front: Vec<usize> = tag
            .pictures()
            .iter()
            .enumerate()
            .filter(|(_, picture)| picture.pic_type() == PictureType::CoverFront)
            .map(|(index, _)| index)
            .collect();
        for index in front.into_iter().rev() {
            tag.remove_picture(index);
        }
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(mime),
            None,
            bytes.to_vec(),
        ));
    }
    file.save_to_path(path, WriteOptions::new())
        .map_err(|error| error.to_string())
}

/// Front cover embedded in the file, or the first picture if there is no front cover.
pub fn embedded_cover(path: &Path) -> Option<Vec<u8>> {
    use lofty::picture::PictureType;
    use lofty::prelude::TaggedFileExt;
    let tagged = lofty::read_from_path(path).ok()?;
    let mut fallback = None;
    for tag in tagged.tags() {
        for picture in tag.pictures() {
            if picture.data().len() <= 32 {
                continue;
            }
            if picture.pic_type() == PictureType::CoverFront {
                return Some(picture.data().to_vec());
            }
            if fallback.is_none() {
                fallback = Some(picture.data().to_vec());
            }
        }
    }
    fallback
}
