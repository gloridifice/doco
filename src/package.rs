use crate::markdown;
use anyhow::{Result, bail};

pub const PROPOSAL_ONLY_MARKER: &str = "<!-- doco:change mode=proposal-only -->";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageMode {
    Full,
    ProposalOnly,
}

pub fn package_mode(proposal: &str) -> Result<PackageMode> {
    let declarations: Vec<_> = markdown::body_lines(proposal)
        .into_iter()
        .map(|(_, line)| line.trim())
        .filter(|line| line.starts_with("<!-- doco:change mode="))
        .collect();
    match declarations.as_slice() {
        [] => Ok(PackageMode::Full),
        [marker] if *marker == PROPOSAL_ONLY_MARKER => Ok(PackageMode::ProposalOnly),
        [marker] => bail!("proposal.md: unsupported change mode marker: {marker}"),
        _ => bail!("proposal.md: expected at most one doco change mode marker"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_is_explicit_unique_and_ignores_fenced_examples() {
        assert_eq!(package_mode("# Full").unwrap(), PackageMode::Full);
        assert_eq!(
            package_mode(PROPOSAL_ONLY_MARKER).unwrap(),
            PackageMode::ProposalOnly
        );
        assert_eq!(
            package_mode(&format!("```md\n{PROPOSAL_ONLY_MARKER}\n```")).unwrap(),
            PackageMode::Full
        );
        assert!(package_mode("<!-- doco:change mode=unknown -->").is_err());
        assert!(package_mode(&format!("{PROPOSAL_ONLY_MARKER}\n{PROPOSAL_ONLY_MARKER}")).is_err());
    }
}
