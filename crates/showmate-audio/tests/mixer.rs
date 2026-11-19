use showmate_audio::*;
use std::sync::Arc;
use std::time::Duration;

const RATE: u32 = 1000;

fn clip(frames: usize, value: f32) -> Arc<AudioClip> {
    Arc::new(AudioClip::from_stereo(vec![value; frames * 2], RATE))
}

fn params(id: u64, clip: Arc<AudioClip>) -> VoiceParams {
    VoiceParams {
        id,
        clip,
        gain_db: 0.0,
        fade_in: Duration::ZERO,
        fade_out: Duration::ZERO,
        looping: false,
        max_duration: None,
        out_left: 0,
        out_right: 1,
    }
}

fn play(m: &mut Mixer, p: VoiceParams) {
    m.apply(Command::Play(Voice::new(p, RATE)));
}

fn render(m: &mut Mixer, frames: usize, channels: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * channels];
    m.render(&mut out, channels);
    out
}

fn ended(m: &mut Mixer) -> Vec<u64> {
    std::iter::from_fn(|| m.pop_ended()).collect()
}

#[test]
fn plays_then_ends_and_reports() {
    let mut m = Mixer::new(RATE);
    play(&mut m, params(7, clip(100, 0.5)));
    let out = render(&mut m, 60, 2);
    assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-6));
    assert_eq!(m.voice_count(), 1);
    assert!(ended(&mut m).is_empty());
    let out = render(&mut m, 60, 2); // 40 frames geluid, daarna stilte
    assert!((out[2 * 39] - 0.5).abs() < 1e-6);
    assert_eq!(out[2 * 40], 0.0);
    assert_eq!(m.voice_count(), 0);
    assert_eq!(ended(&mut m), vec![7]);
}

#[test]
fn gain_in_db() {
    assert!((db_to_gain(0.0) - 1.0).abs() < 1e-6);
    assert!((db_to_gain(-6.0206) - 0.5).abs() < 1e-3);
    assert_eq!(db_to_gain(-60.0), 0.0);
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(10, 1.0));
    p.gain_db = -6.0206;
    play(&mut m, p);
    assert!((render(&mut m, 1, 2)[0] - 0.5).abs() < 1e-3);
}

#[test]
fn fade_in_ramps_linearly() {
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(1000, 1.0));
    p.fade_in = Duration::from_millis(100);
    play(&mut m, p);
    let out = render(&mut m, 200, 2);
    assert_eq!(out[0], 0.0);
    assert!((out[2 * 50] - 0.5).abs() < 0.02);
    assert!((out[2 * 150] - 1.0).abs() < 1e-6);
}

#[test]
fn fade_out_ramps_to_end() {
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(500, 1.0));
    p.fade_out = Duration::from_millis(100);
    play(&mut m, p);
    let out = render(&mut m, 500, 2);
    assert!((out[2 * 300] - 1.0).abs() < 1e-6);
    assert!((out[2 * 450] - 0.5).abs() < 0.02);
    assert!(out[2 * 499] < 0.02);
}

#[test]
fn max_duration_truncates() {
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(1000, 1.0));
    p.max_duration = Some(Duration::from_millis(100));
    play(&mut m, p);
    let out = render(&mut m, 200, 2);
    assert_eq!(out[2 * 99], 1.0);
    assert_eq!(out[2 * 100], 0.0);
    assert_eq!(ended(&mut m), vec![1]);
}

#[test]
fn looping_wraps_and_stop_fades_out() {
    let mut m = Mixer::new(RATE);
    let mut p = params(3, clip(10, 1.0));
    p.looping = true;
    play(&mut m, p);
    let out = render(&mut m, 95, 2);
    assert!(out.iter().all(|s| *s == 1.0), "loop mag niet stilvallen");
    assert_eq!(m.voice_count(), 1);
    m.apply(Command::Stop {
        id: 3,
        fade_frames: 20,
    });
    let out = render(&mut m, 40, 2);
    assert!(out[2 * 10] < 0.6 && out[2 * 10] > 0.4);
    assert_eq!(out[2 * 30], 0.0);
    assert_eq!(ended(&mut m), vec![3]);
}

#[test]
fn stop_all_stops_everything() {
    let mut m = Mixer::new(RATE);
    for id in 0..3 {
        let mut p = params(id, clip(10, 0.1));
        p.looping = true;
        play(&mut m, p);
    }
    m.apply(Command::StopAll { fade_frames: 5 });
    render(&mut m, 10, 2);
    assert_eq!(m.voice_count(), 0);
    let mut ids = ended(&mut m);
    ids.sort();
    assert_eq!(ids, vec![0, 1, 2]);
}

#[test]
fn pause_freezes_position_and_resumes() {
    let mut m = Mixer::new(RATE);
    // oplopende clip: frame i heeft waarde i/1000
    let samples: Vec<f32> = (0..1000).flat_map(|i| [i as f32 / 1000.0; 2]).collect();
    play(
        &mut m,
        params(1, Arc::new(AudioClip::from_stereo(samples, RATE))),
    );
    render(&mut m, 100, 2);
    m.apply(Command::Pause);
    render(&mut m, 20, 2); // pauze-ramp (10 ms = 10 frames)
    let silent = render(&mut m, 500, 2);
    assert!(silent.iter().all(|s| *s == 0.0));
    m.apply(Command::Resume);
    let out = render(&mut m, 100, 2);
    // hervat vlak na waar hij bevroor (≈ frame 110), niet 600 verder
    let last = out[2 * 99];
    assert!(last > 0.15 && last < 0.3, "positie sprong: {last}");
}

#[test]
fn output_pair_maps_to_channels() {
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(10, 0.5));
    p.out_left = 2;
    p.out_right = 3;
    play(&mut m, p);
    let out = render(&mut m, 1, 4);
    assert_eq!(out, vec![0.0, 0.0, 0.5, 0.5]);
}

#[test]
fn pair_beyond_device_is_silent_and_mono_sums() {
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(10, 0.5));
    p.out_left = 2;
    p.out_right = 3;
    play(&mut m, p);
    assert!(render(&mut m, 4, 2).iter().all(|s| *s == 0.0));
    let mut m = Mixer::new(RATE);
    play(&mut m, params(1, clip(10, 0.5)));
    assert_eq!(render(&mut m, 1, 1), vec![0.5]);
}

#[test]
fn voices_sum_clamp_and_peak() {
    let mut m = Mixer::new(RATE);
    play(&mut m, params(1, clip(10, 0.8)));
    play(&mut m, params(2, clip(10, 0.8)));
    let out = render(&mut m, 5, 2);
    assert!(out.iter().all(|s| *s == 1.0), "som wordt begrensd op 1.0");
    assert_eq!(m.take_peak(), (1.0, 1.0));
    assert_eq!(m.take_peak(), (0.0, 0.0));
}

#[test]
fn empty_clip_ends_immediately() {
    let mut m = Mixer::new(RATE);
    play(
        &mut m,
        params(9, Arc::new(AudioClip::from_stereo(vec![], RATE))),
    );
    render(&mut m, 4, 2);
    assert_eq!(ended(&mut m), vec![9]);
}

#[test]
fn full_mixer_rejects_extra_voices() {
    let mut m = Mixer::new(RATE);
    for id in 0..(MAX_VOICES as u64 + 1) {
        let mut p = params(id, clip(10, 0.0));
        p.looping = true;
        play(&mut m, p);
    }
    assert_eq!(m.voice_count(), MAX_VOICES);
    assert_eq!(ended(&mut m), vec![MAX_VOICES as u64]);
}

/// Een testtoon voor de "Test uitgang"-knop: `secs` seconden sinus met korte fades.
#[test]
fn test_tone_clip_is_stereo_with_fades() {
    let clip = AudioClip::test_tone(48_000, 0.5, 440.0);
    assert_eq!(clip.sample_rate(), 48_000);
    assert!((clip.duration().as_secs_f64() - 0.5).abs() < 0.01);
    let peak = clip.samples().iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(peak > 0.2 && peak < 0.5, "niet te hard: {peak}");
    assert!(
        clip.samples()[0].abs() < 0.01 && clip.samples()[1].abs() < 0.01,
        "begint stil"
    );
    let n = clip.samples().len();
    assert!(clip.samples()[n - 1].abs() < 0.01, "eindigt stil");
}

#[test]
fn routes_to_arbitrary_channel_pairs() {
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(4, 0.5));
    p.out_left = 4;
    p.out_right = 7; // 5 en 8 van een 8-kanaals kaart
    play(&mut m, p);
    let out = render(&mut m, 1, 8);
    assert_eq!(out, vec![0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.5]);
}

#[test]
fn left_and_right_may_swap_or_share_a_channel() {
    let mut m = Mixer::new(RATE);
    let samples: Vec<f32> = (0..8).flat_map(|_| [0.2f32, 0.6]).collect(); // L=0.2, R=0.6
    let mut p = params(1, Arc::new(AudioClip::from_stereo(samples, RATE)));
    p.out_left = 1;
    p.out_right = 0; // verwisseld
    play(&mut m, p);
    assert_eq!(render(&mut m, 1, 2), vec![0.6, 0.2]);
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(8, 0.25));
    p.out_left = 0;
    p.out_right = 0; // beide op kanaal 1: opgeteld
    play(&mut m, p);
    assert_eq!(render(&mut m, 1, 2), vec![0.5, 0.0]);
}

#[test]
fn peaks_follow_routed_channels() {
    let mut m = Mixer::new(RATE);
    let mut p = params(1, clip(10, 0.5));
    p.out_left = 2;
    p.out_right = 3;
    play(&mut m, p);
    render(&mut m, 4, 4);
    assert_eq!(
        m.take_peak(),
        (0.5, 0.5),
        "ook uitgang 3+4 geeft een uitslag"
    );
}
