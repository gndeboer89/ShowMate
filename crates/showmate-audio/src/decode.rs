use crate::clip::AudioClip;
use crate::error::AudioError;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

fn dec(e: SymError) -> AudioError {
    AudioError::Decode(e.to_string())
}

/// Decodeert een bestand (mp3, wav, flac, ogg, aac/m4a) naar stereo f32 op de
/// oorspronkelijke samplerate van het bestand.
pub fn decode_file(path: &Path) -> Result<AudioClip, AudioError> {
    let file = File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(dec)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(AudioError::NoTrack)?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(dec)?;

    let mut rate = track.codec_params.sample_rate;
    let mut out: Vec<f32> = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(dec(e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue, // kapot pakketje overslaan
            Err(e) => return Err(dec(e)),
        };
        let spec = *decoded.spec();
        rate.get_or_insert(spec.rate);
        let channels = spec.channels.count();
        let needed = decoded.capacity() as u64;
        if buf
            .as_ref()
            .is_none_or(|b| (b.capacity() as u64) < needed * channels as u64)
        {
            buf = Some(SampleBuffer::<f32>::new(needed, spec));
        }
        let sb = buf.as_mut().expect("zojuist aangemaakt");
        sb.copy_interleaved_ref(decoded);
        match channels {
            0 => {}
            1 => out.extend(sb.samples().iter().flat_map(|s| [*s, *s])),
            n => out.extend(sb.samples().chunks_exact(n).flat_map(|f| [f[0], f[1]])),
        }
    }

    let rate = rate.ok_or_else(|| AudioError::Decode("onbekende samplerate".into()))?;
    if out.is_empty() {
        return Err(AudioError::Decode("bestand bevat geen audio".into()));
    }
    Ok(AudioClip::from_stereo(out, rate))
}
