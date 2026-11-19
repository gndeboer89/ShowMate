use showmate_audio::*;
use std::path::PathBuf;

fn wav_bytes(channels: u16, rate: u32, frames: &[Vec<i16>]) -> Vec<u8> {
    let data_len = (frames.len() * channels as usize * 2) as u32;
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + data_len).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(channels.to_le_bytes());
    b.extend(rate.to_le_bytes());
    b.extend((rate * u32::from(channels) * 2).to_le_bytes());
    b.extend((channels * 2).to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(data_len.to_le_bytes());
    for f in frames {
        for s in f {
            b.extend(s.to_le_bytes());
        }
    }
    b
}

fn write_tmp(name: &str, bytes: &[u8]) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("showmate-audio-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.wav"));
    std::fs::write(&path, bytes).unwrap();
    (dir, path)
}

#[test]
fn decodes_stereo_wav() {
    let frames: Vec<Vec<i16>> = (0..4410).map(|_| vec![16384, -16384]).collect();
    let (dir, path) = write_tmp("stereo", &wav_bytes(2, 44100, &frames));
    let clip = decode_file(&path).unwrap();
    assert_eq!(clip.sample_rate(), 44100);
    assert_eq!(clip.frames(), 4410);
    assert!((clip.duration().as_secs_f64() - 0.1).abs() < 1e-6);
    assert!((clip.samples()[0] - 0.5).abs() < 1e-3);
    assert!((clip.samples()[1] + 0.5).abs() < 1e-3);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mono_is_upmixed_to_stereo() {
    let frames: Vec<Vec<i16>> = (0..100).map(|_| vec![8192]).collect();
    let (dir, path) = write_tmp("mono", &wav_bytes(1, 8000, &frames));
    let clip = decode_file(&path).unwrap();
    assert_eq!(clip.frames(), 100);
    assert_eq!(clip.samples()[0], clip.samples()[1]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn resample_changes_length_not_duration() {
    let clip = AudioClip::from_stereo(vec![0.25; 44100 * 2], 44100);
    let r = clip.resampled(48000);
    assert_eq!(r.sample_rate(), 48000);
    assert!((r.frames() as i64 - 48000).abs() <= 1);
    assert!((r.samples()[1000] - 0.25).abs() < 1e-6);
    assert!((r.duration().as_secs_f64() - 1.0).abs() < 1e-3);
}

#[test]
fn peaks_cover_the_clip() {
    let mut s = vec![0.0f32; 2000];
    s[1000] = 0.8; // frame 500 van 1000 → halverwege
    s[1001] = 0.8;
    let clip = AudioClip::from_stereo(s, 1000);
    let peaks = clip.peaks();
    assert_eq!(peaks.len(), 512);
    let max_at = peaks.iter().position(|p| *p > 0.5).unwrap();
    assert!((250..262).contains(&max_at), "{max_at}");
}

#[test]
fn garbage_and_missing_files_fail_cleanly() {
    let (dir, path) = write_tmp("garbage", b"dit is geen audio");
    assert!(decode_file(&path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
    assert!(matches!(
        decode_file(std::path::Path::new("/bestaat/niet.mp3")),
        Err(AudioError::Io(_))
    ));
}

#[test]
fn cache_loads_in_background_and_reports_failure() {
    let frames: Vec<Vec<i16>> = (0..4410).map(|_| vec![1000, 1000]).collect();
    let (dir, path) = write_tmp("cache", &wav_bytes(2, 44100, &frames));
    let cache = ClipCache::new();
    assert!(matches!(cache.status(&path), ClipStatus::Unknown));
    cache.request(&path, &[48000]);
    cache.request(&path, &[48000]); // idempotent
    let bad = dir.join("niet-aanwezig.wav");
    cache.request(&bad, &[48000]);
    let start = std::time::Instant::now();
    loop {
        match (cache.status(&path), cache.status(&bad)) {
            (ClipStatus::Ready(c), ClipStatus::Failed(_)) => {
                assert_eq!(c.sample_rate(), 44100, "status() geeft het origineel");
                let ClipStatus::Ready(c48) = cache.status_at(&path, 48000) else {
                    panic!()
                };
                assert_eq!(c48.sample_rate(), 48000);
                let ClipStatus::Ready(c96) = cache.status_at(&path, 96000) else {
                    panic!()
                };
                assert_eq!(
                    c96.sample_rate(),
                    96000,
                    "ook een rate die niet vooraf is gevraagd"
                );
                break;
            }
            _ if start.elapsed().as_secs() > 10 => panic!("cache te traag"),
            _ => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    cache.forget(&path);
    assert!(matches!(cache.status(&path), ClipStatus::Unknown));
    std::fs::remove_dir_all(dir).unwrap();
}
