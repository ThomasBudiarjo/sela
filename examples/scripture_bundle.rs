//! Builds a shipped scripture bundle (`assets/scripture/*.selabible.z`) from
//! a public-domain OSIS, USFX or Zefania file, through the same importer the
//! app uses for user files. Provenance of the shipped inputs is recorded in
//! docs/scripture.md.
//!
//! cargo run --locked --example scripture_bundle -- INPUT ID NAME LANGUAGE RIGHTS SOURCE OUTPUT
use sela::scripture::{TranslationInfo, bundled, import};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, id, name, language, rights, source, output] = &args[..] else {
        eprintln!("usage: scripture_bundle INPUT ID NAME LANGUAGE RIGHTS SOURCE OUTPUT");
        std::process::exit(2);
    };
    let bytes = std::fs::read(input).expect("read input");
    let parsed = import::parse(&bytes).unwrap_or_else(|why| {
        eprintln!("rejected: {why:?}");
        std::process::exit(1);
    });
    for warning in &parsed.warnings {
        eprintln!("warning: {warning:?}");
    }
    let info = TranslationInfo {
        id: id.clone(),
        name: name.clone(),
        language: language.clone(),
        rights: rights.clone(),
        source: source.clone(),
        redistributable: true,
        bundled: true,
    };
    let bundle = bundled::encode(&info, &parsed.verses);
    std::fs::write(output, &bundle).expect("write output");
    println!(
        "{id}: {} verses, {} bytes, sha256 {}",
        parsed.verses.len(),
        bundle.len(),
        bundled::sha256(&bundle)
    );
}
