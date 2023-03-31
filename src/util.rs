/// Insert workspace before or after pivot
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InsertionDestination {
    After { pivot: String },
    Before { pivot: String },
}

impl InsertionDestination {
    pub const fn new(pivot: String, before: bool) -> Self {
        if before {
            Self::Before { pivot }
        } else {
            Self::After { pivot }
        }
    }
    pub fn pivot(&self) -> &str {
        match &self {
            Self::After { pivot } | Self::Before { pivot } => pivot,
        }
    }
}
