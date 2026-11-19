//! Mediabestanden toevoegen: soort herkennen, bepalen wat er in de cue komt te staan en
//! (optioneel) kopiëren naar de map `media` naast de show.

use showmate_core::CueKind;
use std::path::{Path, PathBuf};

const AUDIO_EXT: [&str; 8] = ["mp3", "wav", "flac", "ogg", "oga", "m4a", "aac", "opus"];
const VIDEO_EXT: [&str; 9] = [
    "mp4", "mov", "mkv", "avi", "webm", "m4v", "mpg", "mpeg", "wmv",
];

pub fn audio_extensions() -> &'static [&'static str] {
    &AUDIO_EXT
}

pub fn video_extensions() -> &'static [&'static str] {
    &VIDEO_EXT
}

/// Audio of video op basis van de extensie; `None` voor al het andere.
pub fn kind_of(path: &Path) -> Option<CueKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if AUDIO_EXT.contains(&ext.as_str()) {
        Some(CueKind::Audio)
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some(CueKind::Video)
    } else {
        None
    }
}

/// Wat er in `Cue::media` komt en of er nog gekopieerd moet worden.
#[derive(Debug, PartialEq, Eq)]
pub struct Placement {
    pub value: String,
    /// `(bron, doel)` als het bestand nog naar de media-map gekopieerd moet worden.
    pub copy: Option<(PathBuf, PathBuf)>,
}

/// Bepaalt hoe `src` in een show met basismap `base` wordt opgeslagen:
/// - al in `base\media` of `base`: relatief pad, niets kopiëren;
/// - `copy` aan: kopiëren naar `base\media` en alleen de bestandsnaam bewaren;
/// - anders: het absolute pad.
pub fn place(base: &Path, src: &Path, copy: bool) -> Placement {
    for root in [base.join("media"), base.to_path_buf()] {
        if let Ok(rel) = src.strip_prefix(&root) {
            return Placement {
                value: rel.to_string_lossy().into_owned(),
                copy: None,
            };
        }
    }
    if copy && let Some(name) = src.file_name() {
        let dir = base.join("media");
        let dest = unique_dest(&dir, name.to_string_lossy().as_ref(), src);
        let value = dest
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let same = dest.exists(); // zelfde bestand staat er al (zie `unique_dest`)
        return Placement {
            value,
            copy: (!same).then(|| (src.to_path_buf(), dest)),
        };
    }
    Placement {
        value: src.to_string_lossy().into_owned(),
        copy: None,
    }
}

/// Een vrije bestandsnaam in `dir`. Staat er al een bestand met dezelfde naam én grootte als
/// `src`, dan wordt dat hergebruikt; anders volgt een suffix ` (2)`, ` (3)`, …
pub fn unique_dest(dir: &Path, name: &str, src: &Path) -> PathBuf {
    let candidate = dir.join(name);
    let src_len = std::fs::metadata(src).map(|m| m.len()).ok();
    let (stem, ext) = match Path::new(name).extension() {
        Some(e) => (
            Path::new(name)
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
            format!(".{}", e.to_string_lossy()),
        ),
        None => (name.to_string(), String::new()),
    };
    let mut n = 1;
    let mut path = candidate;
    loop {
        match std::fs::metadata(&path) {
            Err(_) => return path,
            Ok(m) if Some(m.len()) == src_len => return path,
            Ok(_) => {
                n += 1;
                path = dir.join(format!("{stem} ({n}){ext}"));
            }
        }
    }
}

/// Kopieert via een tijdelijk bestand en hernoemt daarna, zodat een half gekopieerd bestand
/// nooit onder de echte naam verschijnt (en dus ook niet half wordt afgespeeld).
pub fn copy_atomic(src: &Path, dest: &Path) -> std::io::Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = PathBuf::from(tmp);
    if let Err(e) = std::fs::copy(src, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, dest)
}

/// Welke relatief opgeslagen mediabestanden moeten mee als een show van `old_base` naar
/// `new_base` verhuist (Opslaan als). Absolute paden en streams blijven zoals ze zijn.
pub fn files_to_carry(
    old_base: &Path,
    new_base: &Path,
    media: &[String],
) -> Vec<(PathBuf, PathBuf)> {
    let mut out: Vec<(PathBuf, PathBuf)> = Vec::new();
    if old_base == new_base {
        return out;
    }
    for m in media
        .iter()
        .filter(|m| !m.trim().is_empty() && !m.contains("://"))
    {
        let rel = Path::new(m);
        if rel.is_absolute() {
            continue;
        }
        for sub in [PathBuf::new(), PathBuf::from("media")] {
            let from = old_base.join(&sub).join(rel);
            let to = new_base.join(&sub).join(rel);
            if from.is_file() {
                if !to.exists() && !out.iter().any(|(_, t)| *t == to) {
                    out.push((from, to));
                }
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("showmate-media-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn classifies_by_extension() {
        assert_eq!(kind_of(Path::new("a.MP3")), Some(CueKind::Audio));
        assert_eq!(kind_of(Path::new("a.flac")), Some(CueKind::Audio));
        assert_eq!(kind_of(Path::new("C:\\v\\a.Mp4")), Some(CueKind::Video));
        assert_eq!(kind_of(Path::new("a.mkv")), Some(CueKind::Video));
        assert_eq!(kind_of(Path::new("a.txt")), None);
        assert_eq!(kind_of(Path::new("geen-extensie")), None);
    }

    #[test]
    fn files_inside_the_show_folder_stay_relative() {
        let base = Path::new("/shows/mijn");
        let p = place(base, &base.join("media").join("sfeer.mp3"), true);
        assert_eq!(
            p,
            Placement {
                value: "sfeer.mp3".into(),
                copy: None
            }
        );
        let p = place(base, &base.join("intro.mp4"), true);
        assert_eq!(
            p,
            Placement {
                value: "intro.mp4".into(),
                copy: None
            }
        );
    }

    #[test]
    fn outside_files_are_copied_or_kept_absolute() {
        let base = tmp("place-base");
        let outside = tmp("place-out");
        let src = outside.join("lied.mp3");
        std::fs::write(&src, b"12345").unwrap();
        let p = place(&base, &src, true);
        assert_eq!(p.value, "lied.mp3");
        assert_eq!(
            p.copy,
            Some((src.clone(), base.join("media").join("lied.mp3")))
        );
        let p = place(&base, &src, false);
        assert_eq!(p.value, src.to_string_lossy());
        assert_eq!(p.copy, None);
        std::fs::remove_dir_all(base).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn same_file_is_reused_and_different_file_gets_a_suffix() {
        let base = tmp("uniq-base");
        let outside = tmp("uniq-out");
        let media = base.join("media");
        std::fs::create_dir_all(&media).unwrap();
        let src = outside.join("lied.mp3");
        std::fs::write(&src, b"12345").unwrap();
        // zelfde grootte: hergebruiken
        std::fs::write(media.join("lied.mp3"), b"abcde").unwrap();
        let p = place(&base, &src, true);
        assert_eq!((p.value.as_str(), p.copy.is_none()), ("lied.mp3", true));
        // andere grootte: ander bestand met dezelfde naam → suffix
        std::fs::write(media.join("lied.mp3"), b"veel langer").unwrap();
        let p = place(&base, &src, true);
        assert_eq!(p.value, "lied (2).mp3");
        std::fs::write(media.join("lied (2).mp3"), b"nog langer dan dat").unwrap();
        assert_eq!(place(&base, &src, true).value, "lied (3).mp3");
        std::fs::remove_dir_all(base).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn copy_is_atomic_and_creates_folders() {
        let d = tmp("copy");
        let src = d.join("bron.wav");
        std::fs::write(&src, b"data").unwrap();
        let dest = d.join("nieuw").join("media").join("bron.wav");
        copy_atomic(&src, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"data");
        assert!(!d.join("nieuw").join("media").join("bron.wav.part").exists());
        assert!(copy_atomic(&d.join("bestaat-niet"), &d.join("x.wav")).is_err());
        assert!(!d.join("x.wav.part").exists(), "mislukte kopie ruimt op");
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn files_travel_with_a_moved_show() {
        let old = tmp("carry-old");
        let new = tmp("carry-new");
        std::fs::create_dir_all(old.join("media")).unwrap();
        std::fs::write(old.join("media").join("a.mp3"), b"a").unwrap();
        std::fs::write(old.join("b.mp4"), b"b").unwrap();
        let media = vec![
            "a.mp3".to_string(),
            "b.mp4".to_string(),
            "a.mp3".to_string(),
            "C:\\abs\\c.mp3".to_string(),
            "av://lavfi:x".to_string(),
            "bestaat-niet.wav".to_string(),
            String::new(),
        ];
        let carry = files_to_carry(&old, &new, &media);
        assert_eq!(
            carry,
            vec![
                (
                    old.join("media").join("a.mp3"),
                    new.join("media").join("a.mp3")
                ),
                (old.join("b.mp4"), new.join("b.mp4")),
            ]
        );
        assert!(files_to_carry(&old, &old, &media).is_empty());
        std::fs::remove_dir_all(old).unwrap();
        std::fs::remove_dir_all(new).unwrap();
    }
}
