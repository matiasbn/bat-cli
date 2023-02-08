use std::fmt::Display;

use crate::batbelt::markdown::{MarkdownFile, MarkdownSectionLevel};
use inflector::Inflector;

pub enum MarkdownTemplate {
    CodeOverhaul,
    Finding,
    Informational,
    Result,
    FindingCandidates,
    Metadata,
    OpenQuestions,
    ThreatModeling,
    Robot,
}
