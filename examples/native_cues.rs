//! Opt-in audience diagnostic child for `scripts/native-cues.py`; stdin/stdout
//! are binary protocol, never logs. Shares the application's audience renderer
//! with a fixed 641x360 window, top-left text and a hard 24s/25s lifetime.
#[allow(dead_code)] // Fullscreen placement is used only by the application.
#[path = "../src/audience.rs"]
mod audience;
use sela::delivery::Epoch;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: native_cues <fresh-nonzero-epoch-hex> <gl|vulkan|dx12|metal>".into());
    }
    let epoch = Epoch(u128::from_str_radix(&args[1], 16)?);
    let backend = audience::parse_backend(&args[2]).ok_or("unsupported backend; no fallback")?;
    audience::run(audience::Config {
        epoch,
        backend,
        title: "Sela M0-06b native cue diagnostic",
        placement: audience::Placement::Window {
            width: 641,
            height: 360,
        },
        lifetime: Some(Duration::from_secs(24)),
        report_surface: false,
        centered_text: false,
        retain_on_disconnect: false,
    })
}
