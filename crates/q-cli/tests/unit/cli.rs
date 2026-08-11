use super::*;

#[test]
fn no_subcommand_selects_default_tui() {
    let cli = Cli::try_parse_from(["q"]).unwrap();
    assert!(cli.command.is_none());
}

#[test]
fn explicit_tui_remains_available() {
    let cli = Cli::try_parse_from(["q", "tui"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Tui)));
}
