use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use prism_api::{CommentDraft, Diagnostic, FileRange};
use prism_plugin_api::{
    AgentPlugin, PluginCapabilities, PluginError, PluginResult, PluginSession, ReviewPayload,
    RevisionProgress, RevisionState, SubmissionResult, ThreadRef,
};
use wait_timeout::ChildExt;

const DEFAULT_TIMEOUT_SECS: u64 = 60;
const SESSION_PREFIX: &str = "amp-session";
const MOCK_ENV_PREFIX: &str = "PRISM_AMP_";

#[derive(Debug, Clone)]
struct AmpCli {
    binary: OsString,
    timeout: Duration,
    path: Option<OsString>,
    home: Option<OsString>,
    amp_api_key: Option<OsString>,
    amp_url: Option<OsString>,
    amp_settings_file: Option<OsString>,
    passthrough: Vec<(OsString, OsString)>,
}

impl AmpCli {
    fn new() -> Self {
        let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let mut passthrough = Vec::new();
        for (key, value) in env::vars_os() {
            if key.to_string_lossy().starts_with(MOCK_ENV_PREFIX) {
                passthrough.push((key, value));
            }
        }
        let binary = env::var_os("PRISM_AMP_CLI_BIN").unwrap_or_else(|| OsString::from("amp"));
        Self {
            binary,
            timeout,
            path: env::var_os("PATH"),
            home: env::var_os("HOME"),
            amp_api_key: env::var_os("AMP_API_KEY"),
            amp_url: env::var_os("AMP_URL"),
            amp_settings_file: env::var_os("AMP_SETTINGS_FILE"),
            passthrough,
        }
    }

    fn list_threads(&self) -> PluginResult<Vec<ThreadRef>> {
        let output = self.run(&["threads", "list"], None)?;
        Ok(parse_thread_table(&output.stdout))
    }

    fn create_thread(&self) -> PluginResult<String> {
        let output = self.run(&["threads", "new"], None)?;
        let id = output.stdout.trim();
        if id.is_empty() {
            return Err(PluginError::message("Amp CLI did not return a thread id"));
        }
        Ok(id.to_string())
    }

    fn continue_thread(&self, thread_id: &str, message: &str) -> PluginResult<ProcessOutput> {
        let args = ["threads", "continue", thread_id, "--execute"];
        self.run(&args, Some(message))
    }

    fn run(&self, args: &[&str], stdin_payload: Option<&str>) -> PluginResult<ProcessOutput> {
        let mut command = Command::new(&self.binary);
        command.args(args);
        command.stdin(if stdin_payload.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        self.configure_environment(&mut command);

        let mut child = command
            .spawn()
            .map_err(|err| PluginError::message(format!("failed to spawn amp CLI: {err}")))?;

        if let Some(body) = stdin_payload {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(body.as_bytes()).map_err(|err| {
                    PluginError::message(format!("failed to write to amp stdin: {err}"))
                })?;
            }
        }

        let stdout_handle = child.stdout.take().map(|mut stdout| {
            thread::spawn(move || -> io::Result<Vec<u8>> {
                let mut buffer = Vec::new();
                stdout.read_to_end(&mut buffer)?;
                Ok(buffer)
            })
        });

        let stderr_handle = child.stderr.take().map(|mut stderr| {
            thread::spawn(move || -> io::Result<Vec<u8>> {
                let mut buffer = Vec::new();
                stderr.read_to_end(&mut buffer)?;
                Ok(buffer)
            })
        });

        match child.wait_timeout(self.timeout) {
            Ok(Some(_)) => (),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PluginError::message("Amp CLI timed out after 60s"));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PluginError::message(format!(
                    "failed waiting on amp CLI: {err}"
                )));
            }
        }

        let status = child
            .wait()
            .map_err(|err| PluginError::message(format!("failed to reap amp CLI: {err}")))?;

        let stdout = join_reader(stdout_handle, "stdout")?;
        let stderr = join_reader(stderr_handle, "stderr")?;

        if !status.success() {
            let code = status
                .code()
                .map_or_else(|| "terminated".to_string(), |c| c.to_string());
            return Err(PluginError::message(format!(
                "Amp CLI failed with status {}: {}",
                code,
                stderr.trim()
            )));
        }

        Ok(ProcessOutput { stdout, stderr })
    }

    fn configure_environment(&self, command: &mut Command) {
        command.env_clear();
        if let Some(path) = &self.path {
            command.env("PATH", path);
        }
        if let Some(home) = &self.home {
            command.env("HOME", home);
        }
        if let Some(api_key) = &self.amp_api_key {
            command.env("AMP_API_KEY", api_key);
        }
        if let Some(url) = &self.amp_url {
            command.env("AMP_URL", url);
        }
        if let Some(settings) = &self.amp_settings_file {
            command.env("AMP_SETTINGS_FILE", settings);
        }
        for (key, value) in &self.passthrough {
            command.env(key, value);
        }
    }
}

fn join_reader(
    handle: Option<std::thread::JoinHandle<io::Result<Vec<u8>>>>,
    stream: &str,
) -> PluginResult<String> {
    match handle {
        Some(handle) => {
            let bytes = handle
                .join()
                .map_err(|_| PluginError::message(format!("failed to join amp {stream} reader")))?
                .map_err(|err| {
                    PluginError::message(format!("failed to read amp {stream}: {err}"))
                })?;
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }
        None => Ok(String::new()),
    }
}

/// Sourcegraph Amp plugin backed by the local Amp CLI.
#[derive(Debug)]
pub struct AmpPlugin {
    cli: AmpCli,
    sessions: Mutex<HashMap<String, Arc<AmpSession>>>,
    counter: AtomicU64,
}

impl AmpPlugin {
    /// Construct a plugin that shells out to the Amp CLI with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cli: AmpCli::new(),
            sessions: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
        }
    }

    fn session(&self, id: &str) -> PluginResult<Arc<AmpSession>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| PluginError::message("Amp session registry poisoned"))?;
        sessions
            .get(id)
            .cloned()
            .ok_or_else(|| PluginError::message("Unknown Amp session"))
    }

    fn next_session_id(&self) -> String {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("{SESSION_PREFIX}-{counter}")
    }
}

impl Default for AmpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPlugin for AmpPlugin {
    fn id(&self) -> &'static str {
        "amp"
    }

    fn label(&self) -> &'static str {
        "Sourcegraph Amp"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::new(true, true, true)
    }

    fn list_threads(&self) -> PluginResult<Vec<ThreadRef>> {
        self.cli.list_threads()
    }

    fn attach(&self, thread_id: Option<&str>) -> PluginResult<PluginSession> {
        let thread = if let Some(id) = thread_id {
            ThreadRef::new(id, None::<String>)
        } else {
            let id = self.cli.create_thread()?;
            ThreadRef::new(id, None::<String>)
        };

        let session_id = self.next_session_id();
        let handle = Arc::new(AmpSession::new(thread.clone()));
        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| PluginError::message("Amp session registry poisoned"))?;
            sessions.insert(session_id.clone(), handle);
        }

        Ok(PluginSession::new(self.id(), session_id, Some(thread)))
    }

    fn post_review(
        &self,
        session: &PluginSession,
        payload: ReviewPayload,
    ) -> PluginResult<SubmissionResult> {
        let state = self.session(&session.session_id)?;
        let message = render_payload(&payload);
        let progress = Arc::new(Mutex::new(RevisionProgress {
            state: RevisionState::InProgress,
            detail: Some("Amp processing started".into()),
        }));

        let thread_id = state.thread.id.clone();
        let thread_id_for_task = thread_id.clone();
        let cli = self.cli.clone();
        let progress_clone = Arc::clone(&progress);

        let handle = thread::spawn(move || {
            let result = cli.continue_thread(&thread_id_for_task, &message);
            let mut guard = progress_clone
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match result {
                Ok(output) => {
                    guard.state = RevisionState::Completed;
                    guard.detail = Some(output.stdout.trim().to_string());
                }
                Err(err) => {
                    guard.state = RevisionState::Failed;
                    guard.detail = Some(err.to_string());
                }
            }
        });

        {
            let mut revision_slot = state
                .revision
                .lock()
                .map_err(|_| PluginError::message("Amp revision state poisoned"))?;
            *revision_slot = Some(AmpRevision { progress, handle });
        }

        Ok(SubmissionResult {
            revision_started: true,
            reference: Some(thread_id),
            message: Some("Amp review submitted".into()),
        })
    }

    fn poll_revision(&self, session: &PluginSession) -> PluginResult<RevisionProgress> {
        let state = self.session(&session.session_id)?;
        let revision_slot = state
            .revision
            .lock()
            .map_err(|_| PluginError::message("Amp revision state poisoned"))?;
        if let Some(revision) = revision_slot.as_ref() {
            let guard = revision
                .progress
                .lock()
                .map_err(|_| PluginError::message("Amp revision state poisoned"))?;
            Ok(guard.clone())
        } else {
            Ok(RevisionProgress {
                state: RevisionState::Pending,
                detail: Some("No revision in progress".into()),
            })
        }
    }
}

#[derive(Debug)]
struct AmpSession {
    thread: ThreadRef,
    revision: Mutex<Option<AmpRevision>>,
}

impl AmpSession {
    const fn new(thread: ThreadRef) -> Self {
        Self {
            thread,
            revision: Mutex::new(None),
        }
    }
}

#[derive(Debug)]
struct AmpRevision {
    progress: Arc<Mutex<RevisionProgress>>,
    #[allow(dead_code)]
    handle: thread::JoinHandle<()>,
}

#[derive(Debug)]
struct ProcessOutput {
    stdout: String,
    #[allow(dead_code)]
    stderr: String,
}

fn parse_thread_table(output: &str) -> Vec<ThreadRef> {
    output
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('─') && !trimmed.starts_with("Title ")
        })
        .filter_map(parse_thread_line)
        .collect()
}

fn parse_thread_line(line: &str) -> Option<ThreadRef> {
    let columns = split_columns(line);
    if columns.len() < 4 {
        return None;
    }

    let title_split = columns.len().saturating_sub(4);
    let title_split = if title_split == 0 { 1 } else { title_split };
    let (title_cols, tail_cols) = columns.split_at(title_split);
    if tail_cols.len() < 3 {
        return None;
    }

    let info_cols = if tail_cols.len() >= 4 {
        &tail_cols[1..]
    } else {
        tail_cols
    };

    if info_cols.len() < 3 {
        return None;
    }

    let thread_id = info_cols[info_cols.len() - 1];
    let title = title_cols.join(" ").trim().to_string();

    Some(ThreadRef::new(
        thread_id,
        (!title.is_empty()).then_some(title),
    ))
}

fn split_columns(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut columns = Vec::new();
    let mut start = 0;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b' ' {
            let mut j = i;
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if j - i >= 2 {
                if start < i {
                    let segment = line[start..i].trim();
                    if !segment.is_empty() {
                        columns.push(segment);
                    }
                }
                start = j;
            }
            i = j;
        } else {
            i += 1;
        }
    }

    if start < bytes.len() {
        let segment = line[start..].trim();
        if !segment.is_empty() {
            columns.push(segment);
        }
    }

    columns
}

fn render_payload(payload: &ReviewPayload) -> String {
    let mut sections = Vec::new();

    if let Some(summary) = &payload.summary {
        sections.push(format!("Summary:\n{}", summary.trim()));
    }

    if !payload.actions.is_empty() {
        let body = payload
            .actions
            .iter()
            .map(|action| format!("- {action}"))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Requested Actions:\n{body}"));
    }

    if !payload.comments.is_empty() {
        let body = payload
            .comments
            .iter()
            .map(format_comment)
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Inline Comments:\n{body}"));
    }

    if !payload.diagnostics.is_empty() {
        let body = payload
            .diagnostics
            .iter()
            .map(format_diagnostic)
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Diagnostics:\n{body}"));
    }

    sections.join("\n\n")
}

fn format_comment(comment: &CommentDraft) -> String {
    let (start, end) = format_line_range(&comment.location);
    format!(
        "- {path} [{side:?}] lines {start}-{end}: {body}",
        path = comment.location.path,
        side = comment.location.side,
        start = start,
        end = end,
        body = comment.body.trim()
    )
}

fn format_diagnostic(diag: &Diagnostic) -> String {
    let detail = diag
        .detail
        .as_ref()
        .map(|d| format!(" -- {}", d.trim()))
        .unwrap_or_default();
    let (start, end) = format_line_range(&diag.location);
    format!(
        "- {title} [{severity:?}] at {path} [{side:?}] lines {start}-{end}{detail}",
        title = diag.title,
        severity = diag.severity,
        path = diag.location.path,
        side = diag.location.side,
        start = start,
        end = end,
    )
}

fn format_line_range(range: &FileRange) -> (u32, u32) {
    let start = range.range.start.line;
    let raw_end = range.range.end.line;
    let end = raw_end.saturating_sub(1).max(start);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_api::{DiffSide, FileRange, Position, Range, Severity};

    #[test]
    fn render_payload_formats_all_sections() {
        let diagnostic = {
            let mut diag = Diagnostic::new(
                "Diag title",
                Severity::Warning,
                FileRange::new(
                    "src/lib.rs",
                    DiffSide::Base,
                    Range::new(Position::new(3, None), Position::new(4, None)),
                ),
            );
            diag.detail = Some(" Detail info ".into());
            diag
        };
        let payload = ReviewPayload {
            summary: Some(" Summary text ".into()),
            actions: vec!["Action one".into(), "Action two".into()],
            comments: vec![CommentDraft {
                body: " Comment body ".into(),
                location: FileRange::new(
                    "src/lib.rs",
                    DiffSide::Head,
                    Range::new(Position::new(10, Some(1)), Position::new(12, Some(1))),
                ),
            }],
            diagnostics: vec![diagnostic],
        };

        let rendered = render_payload(&payload);

        let expected = "Summary:\nSummary text\n\nRequested Actions:\n- Action one\n- Action two\n\nInline Comments:\n- src/lib.rs [Head] lines 10-11: Comment body\n\nDiagnostics:\n- Diag title [Warning] at src/lib.rs [Base] lines 3-3 -- Detail info";
        assert_eq!(rendered, expected);
    }

    #[test]
    fn parse_thread_line_keeps_single_word_title() {
        let line = format!(
            "{:<45}{:<15}{:<12}{:<10}{}",
            "Docs", "an hour ago", "Private", "12", "T-123",
        );

        let thread = parse_thread_line(&line).expect("parse");
        assert_eq!(thread.id, "T-123");
        assert_eq!(thread.title.as_deref(), Some("Docs"));
    }

    #[test]
    fn parse_thread_line_drops_last_updated_phrase() {
        let line = format!(
            "{:<45}{:<15}{:<12}{:<10}{}",
            "My Fix", "5 minutes ago", "Private", "7", "T-456",
        );

        let thread = parse_thread_line(&line).expect("parse");
        assert_eq!(thread.id, "T-456");
        assert_eq!(thread.title.as_deref(), Some("My Fix"));
    }
}
