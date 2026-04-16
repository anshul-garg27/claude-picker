//! `--pipe` command — run the picker, then print the chosen session id to
//! stdout so shell pipelines can consume it.
//!
//! Everything else goes to stderr so a `$(claude-picker pipe)` substitution
//! captures exactly the id.

pub fn run() -> anyhow::Result<()> {
    match crate::commands::pick::run()? {
        Some((id, _)) => {
            println!("{id}");
            Ok(())
        }
        None => {
            // No selection — exit with a non-zero code so the shell can tell.
            std::process::exit(2);
        }
    }
}
