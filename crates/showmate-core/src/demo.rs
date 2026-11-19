use crate::cue::{Cue, CueId, CueKind};
use crate::show::Show;

/// Voorbeeldshow naar het ontwerp-screenshot (7 cues).
pub fn demo_show() -> Show {
    let mut show = Show::new("Demo");
    let z = CueId(0);
    show.push(
        Cue::new(z, "Sfeer muziek", CueKind::Audio)
            .media("sfeer_opening.mp3")
            .duration_secs(192.0)
            .fade_in_secs(2.0)
            .output("Audio 1+2"),
    );
    show.push(
        Cue::new(z, "Videoprojectie - Intro", CueKind::Video)
            .media("intro.mp4")
            .duration_secs(45.0)
            .output("Display 1"),
    );
    show.push(
        Cue::new(z, "Spots - Opkomst", CueKind::Osc)
            .media("MIDI / OSC")
            .delay_secs(5.0)
            .output("OSC (Lighting)")
            .notes("Spots langzaam op. Intensity 0 → 70% (5 s)"),
    );
    show.push(
        Cue::new(z, "Projectie - Bos", CueKind::Video)
            .media("bos_loop.mp4")
            .auto()
            .delay_secs(5.0)
            .looping()
            .output("Display 2"),
    );
    show.push(
        Cue::new(z, "Sound FX - Wind", CueKind::Audio)
            .media("wind.wav")
            .auto()
            .delay_secs(5.0)
            .duration_secs(90.0)
            .volume_db(-6.0)
            .output("Audio 1+2"),
    );
    show.push(
        Cue::new(z, "Einde - Fade out", CueKind::Audio)
            .media("uitfade.mp3")
            .auto()
            .delay_secs(80.0)
            .duration_secs(10.0)
            .output("Audio 1+2"),
    );
    show.push(
        Cue::new(z, "Einde - Zwart", CueKind::Video)
            .media("zwart.mp4")
            .auto()
            .delay_secs(90.0)
            .duration_secs(2.0)
            .output("Display 1+2"),
    );
    show
}
