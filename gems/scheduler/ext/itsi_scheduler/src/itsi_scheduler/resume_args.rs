use super::Readiness;

pub(crate) enum ResumeArgs {
    Readiness(Readiness),
    None,
}
