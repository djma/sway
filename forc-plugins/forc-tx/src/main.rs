use clap::Parser;

fn main() {
    let cmd = forc_tx::Command::parse();
    dbg!(cmd);
}
