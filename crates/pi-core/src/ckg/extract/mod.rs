use std::path::Path;

use crate::ckg::graph::Symbol;

pub trait LanguageExtractor {
    fn extract(&self, text: &str, path: &Path) -> anyhow::Result<Vec<Symbol>>;
}
