use std::path::Path;

use anyhow::Result;
use q_core::PromptSource;
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceOutput<'a> {
    Inline,
    MarkdownFile { path: &'a Path },
}

pub struct ResolvedSource<'a> {
    pub text: Option<String>,
    pub source: SourceOutput<'a>,
    pub available: bool,
}

pub fn read(source: &PromptSource) -> Result<String> {
    match source {
        PromptSource::Inline { text } => Ok(text.clone()),
        PromptSource::ExternalMarkdown { path } => {
            Ok(q_platform::external_document::read_utf8(path)?)
        }
    }
}

pub fn resolve(source: &PromptSource) -> ResolvedSource<'_> {
    match source {
        PromptSource::Inline { text } => ResolvedSource {
            text: Some(text.clone()),
            source: SourceOutput::Inline,
            available: true,
        },
        PromptSource::ExternalMarkdown { path } => {
            match q_platform::external_document::read_utf8(path) {
                Ok(text) => ResolvedSource {
                    text: Some(text),
                    source: SourceOutput::MarkdownFile { path },
                    available: true,
                },
                Err(_) => ResolvedSource {
                    text: None,
                    source: SourceOutput::MarkdownFile { path },
                    available: false,
                },
            }
        }
    }
}

pub fn searchable_text(source: &PromptSource) -> String {
    match source {
        PromptSource::Inline { text } => text.clone(),
        PromptSource::ExternalMarkdown { path } => {
            let mut searchable = path.to_string_lossy().into_owned();
            if let Ok(text) = q_platform::external_document::read_utf8(path) {
                searchable.push('\n');
                searchable.push_str(&text);
            }
            searchable
        }
    }
}

pub fn forget_matching(workspace: &mut q_core::Workspace, query: &q_core::search::Query) -> usize {
    let matches: Vec<_> = workspace
        .history()
        .iter()
        .filter(|entry| query.is_match(&searchable_text(entry.source())))
        .map(|entry| entry.source().clone())
        .collect();
    let count = matches.len();
    for source in matches {
        workspace.forget_history(&source);
    }
    count
}
