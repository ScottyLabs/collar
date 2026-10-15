use clap::{CommandFactory, ValueEnum};

use super::Cli;

#[derive(Clone, Copy, ValueEnum)]
pub(super) enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    Zsh,
    Nushell,
}

impl CompletionShell {
    pub(super) fn generate(self) {
        let mut command = Cli::command();
        let mut output = std::io::stdout();
        let generator = match self {
            Self::Nushell => {
                clap_complete::generate(
                    clap_complete_nushell::Nushell,
                    &mut command,
                    "collar",
                    &mut output,
                );
                return;
            }
            Self::Bash => clap_complete::Shell::Bash,
            Self::Elvish => clap_complete::Shell::Elvish,
            Self::Fish => clap_complete::Shell::Fish,
            Self::PowerShell => clap_complete::Shell::PowerShell,
            Self::Zsh => clap_complete::Shell::Zsh,
        };
        clap_complete::generate(generator, &mut command, "collar", &mut output);
    }
}
