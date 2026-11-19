//! Speelt een bestand af en toont de pieken: `cargo run -p showmate-audio --example play -- bestand.mp3 [dB]`
use showmate_audio::{AudioOutput, VoiceParams, decode_file};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("gebruik: play <bestand> [dB]");
    let gain_db: f32 = args.next().map_or(0.0, |s| s.parse().expect("dB"));
    let mut out = AudioOutput::open_default().expect("audio-apparaat");
    println!(
        "{} — {} Hz, {} kanalen",
        out.device_name(),
        out.sample_rate(),
        out.channels()
    );
    let clip = Arc::new(
        decode_file(path.as_ref())
            .expect("decoderen")
            .resampled(out.sample_rate()),
    );
    println!("duur {:.1} s", clip.duration().as_secs_f64());
    out.play(VoiceParams {
        id: 1,
        clip: clip.clone(),
        gain_db,
        fade_in: Duration::from_millis(500),
        fade_out: Duration::from_millis(500),
        looping: false,
        max_duration: None,
        out_pair: 0,
    })
    .expect("play");
    let start = Instant::now();
    while out.drain_ended().is_empty() && start.elapsed() < clip.duration() + Duration::from_secs(2)
    {
        std::thread::sleep(Duration::from_millis(250));
        let (l, r) = out.take_peaks();
        println!(
            "{:5.2} s  L {l:.3}  R {r:.3}",
            start.elapsed().as_secs_f64()
        );
    }
    println!("klaar na {:.2} s", start.elapsed().as_secs_f64());
}
