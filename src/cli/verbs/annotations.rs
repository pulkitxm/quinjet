#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(in crate::cli) enum AnnotationSeverityArg {
    Failure,
    Warning,
    Notice,
}

impl From<AnnotationSeverityArg> for AnnotationSeverity {
    fn from(severity: AnnotationSeverityArg) -> Self {
        match severity {
            AnnotationSeverityArg::Failure => Self::Failure,
            AnnotationSeverityArg::Warning => Self::Warning,
            AnnotationSeverityArg::Notice => Self::Notice,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(in crate::cli) enum AnnotationGroupingArg {
    File,
    Check,
    Severity,
}

impl From<AnnotationGroupingArg> for AnnotationGrouping {
    fn from(grouping: AnnotationGroupingArg) -> Self {
        match grouping {
            AnnotationGroupingArg::File => Self::File,
            AnnotationGroupingArg::Check => Self::Check,
            AnnotationGroupingArg::Severity => Self::Severity,
        }
    }
}

#[derive(Debug, Args)]
pub(in crate::cli) struct PrAnnotationsArgs {
    #[command(flatten)]
    pub(in crate::cli) pull_request: PrArgs,
    #[doc = " Only annotations at this level"]
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub(in crate::cli) severity: Option<AnnotationSeverityArg>,
    #[doc = " Only annotations from check runs whose name contains this"]
    #[arg(long, value_name = "NAME", value_hint = ValueHint::Other)]
    pub(in crate::cli) check: Option<String>,
    #[doc = " Only annotations under this path"]
    #[arg(long = "file", value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(in crate::cli) path: Option<PathBuf>,
    #[doc = " Only annotations on lines the pull request's patch shows"]
    #[arg(long)]
    pub(in crate::cli) in_diff: bool,
    #[doc = " How to group the listing"]
    #[arg(long, value_enum, value_name = "BY", default_value = "file")]
    pub(in crate::cli) group: AnnotationGroupingArg,
    #[doc = " Print each annotation's full message rather than one line"]
    #[arg(long)]
    pub(in crate::cli) full: bool,
    #[doc = " Exit 1 when any listed annotation is a failure"]
    #[arg(long)]
    pub(in crate::cli) exit_code: bool,
}
