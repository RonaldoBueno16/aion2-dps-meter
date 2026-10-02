// Raw socket (SIO_RCVALL) só funciona elevado: o Windows pede UAC ao abrir o executável de
// release. O build de debug abre sem elevação, para testar a janela com --replay.
use embed_manifest::manifest::ExecutionLevel;
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    let nivel = if std::env::var("PROFILE").as_deref() == Ok("release") {
        ExecutionLevel::RequireAdministrator
    } else {
        ExecutionLevel::AsInvoker
    };
    embed_manifest(new_manifest("Aion2Meter").requested_execution_level(nivel)).expect("manifesto do Windows");
    println!("cargo:rerun-if-changed=build.rs");
}
