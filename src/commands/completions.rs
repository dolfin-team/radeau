use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::Cli;

pub fn run(shell: Shell) {
    let mut cmd = Cli::command();
    let bin_name = "raft";

    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
}
