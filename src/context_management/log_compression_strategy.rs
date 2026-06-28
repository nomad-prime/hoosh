use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;

use crate::agent::{Conversation, ConversationMessage};
use crate::context_management::{ContextManagementStrategy, LogCompressionConfig, StrategyResult};
use crate::tools::ToolRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pytest,
    Npm,
    Cargo,
    Jest,
    Make,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogLevel {
    Error,
    Fail,
    Warn,
    Info,
    Debug,
    Trace,
    Unknown,
}

#[derive(Debug, Clone)]
struct LogLine {
    line_number: usize,
    content: String,
    level: LogLevel,
    is_stack_trace: bool,
    is_summary: bool,
    score: f32,
}

impl LogLine {
    fn new(line_number: usize, content: &str) -> Self {
        Self {
            line_number,
            content: content.to_string(),
            level: LogLevel::Unknown,
            is_stack_trace: false,
            is_summary: false,
            score: 0.0,
        }
    }
}

struct FormatDetector {
    matchers: Vec<(LogFormat, AhoCorasick)>,
}

impl FormatDetector {
    fn new() -> Self {
        let table: &[(LogFormat, &[&str])] = &[
            (
                LogFormat::Pytest,
                &[
                    "=== FAILURES",
                    "=== ERRORS",
                    "=== test session",
                    "=== short test summary",
                    "PASSED [",
                    "FAILED [",
                    "ERROR [",
                    "SKIPPED [",
                    "collected ",
                ],
            ),
            (
                LogFormat::Npm,
                &["npm ERR!", "npm WARN", "npm info", "npm http"],
            ),
            (
                LogFormat::Cargo,
                &[
                    "Compiling ",
                    "Finished ",
                    "Running ",
                    "warning: ",
                    "error[E",
                ],
            ),
            (LogFormat::Jest, &["PASS ", "FAIL ", "Test Suites:"]),
            (
                LogFormat::Make,
                &["make[", "make:", "gcc ", "g++ ", "clang "],
            ),
        ];

        let matchers = table
            .iter()
            .map(|(fmt, patterns)| {
                let ac = AhoCorasickBuilder::new()
                    .match_kind(MatchKind::LeftmostFirst)
                    .build(*patterns)
                    .expect("format-detector automaton builds from static input");
                (*fmt, ac)
            })
            .collect();
        Self { matchers }
    }

    fn detect(&self, lines: &[&str]) -> LogFormat {
        let sample: Vec<&str> = lines.iter().take(100).copied().collect();
        let mut best: Option<(LogFormat, usize)> = None;
        for (fmt, ac) in &self.matchers {
            let score = sample.iter().filter(|line| ac.is_match(**line)).count();
            if score > 0 && best.map(|(_, s)| score > s).unwrap_or(true) {
                best = Some((*fmt, score));
            }
        }
        best.map(|(f, _)| f).unwrap_or(LogFormat::Generic)
    }
}

struct LevelClassifier {
    automaton: AhoCorasick,
    levels: Vec<LogLevel>,
}

impl LevelClassifier {
    fn new() -> Self {
        let entries: &[(LogLevel, &[&str])] = &[
            (
                LogLevel::Error,
                &[
                    "ERROR", "error", "Error", "FATAL", "fatal", "Fatal", "CRITICAL", "critical",
                ],
            ),
            (
                LogLevel::Fail,
                &["FAIL", "FAILED", "fail", "failed", "Fail", "Failed"],
            ),
            (
                LogLevel::Warn,
                &["WARN", "WARNING", "warn", "warning", "Warn", "Warning"],
            ),
            (LogLevel::Info, &["INFO", "info", "Info"]),
            (LogLevel::Debug, &["DEBUG", "debug", "Debug"]),
            (LogLevel::Trace, &["TRACE", "trace", "Trace"]),
        ];
        let mut patterns = Vec::new();
        let mut levels = Vec::new();
        for (level, words) in entries {
            for w in *words {
                patterns.push(*w);
                levels.push(*level);
            }
        }
        let automaton = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(&patterns)
            .expect("level-classifier automaton builds from static input");
        Self { automaton, levels }
    }

    fn classify(&self, line: &str) -> LogLevel {
        let bytes = line.as_bytes();
        for m in self.automaton.find_iter(line) {
            if is_word_boundary(bytes, m.start(), m.end()) {
                return self.levels[m.pattern().as_usize()];
            }
        }
        LogLevel::Unknown
    }
}

fn is_word_boundary(bytes: &[u8], start: usize, end: usize) -> bool {
    let left_ok = start == 0 || !is_word_byte(bytes[start - 1]);
    let right_ok = end == bytes.len() || !is_word_byte(bytes[end]);
    left_ok && right_ok
}

#[inline]
fn is_word_byte(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TraceFlavor {
    PythonTraceback,
    Js,
    Java,
    RustError,
    Go,
}

fn trace_flavor_for(line: &str) -> Option<TraceFlavor> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("Traceback (most recent call last)") || is_python_file_frame(trimmed) {
        Some(TraceFlavor::PythonTraceback)
    } else if is_js_at_frame(trimmed) {
        Some(TraceFlavor::Js)
    } else if is_java_at_frame(trimmed) {
        Some(TraceFlavor::Java)
    } else if trimmed.starts_with("--> ") && has_line_col_suffix(trimmed) {
        Some(TraceFlavor::RustError)
    } else if is_go_frame(line) {
        Some(TraceFlavor::Go)
    } else {
        None
    }
}

fn is_python_file_frame(s: &str) -> bool {
    s.starts_with("File \"")
        && s.contains("\", line ")
        && s.bytes().next_back().is_some_and(|b| b.is_ascii_digit())
}

fn is_js_at_frame(s: &str) -> bool {
    s.starts_with("at ") && s.contains('(') && s.contains(')') && has_line_col_suffix(s)
}

fn is_java_at_frame(s: &str) -> bool {
    if !s.starts_with("at ") || !s.contains('(') {
        return false;
    }
    let body = &s[3..s.find('(').unwrap_or(s.len())];
    !body.is_empty()
        && body
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '$'))
}

fn has_line_col_suffix(s: &str) -> bool {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i] == b':' && bytes[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len()
                && bytes[j] == b':'
                && bytes.get(j + 1).is_some_and(|b| b.is_ascii_digit())
            {
                return true;
            }
        }
    }
    false
}

fn is_go_frame(s: &str) -> bool {
    let trimmed = s.trim_start();
    let mut chars = trimmed.chars().peekable();
    let mut saw_digit = false;
    while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
        saw_digit = true;
        chars.next();
    }
    if !saw_digit || chars.next() != Some(':') {
        return false;
    }
    while chars.peek() == Some(&' ') {
        chars.next();
    }
    let rest: String = chars.collect();
    rest.starts_with("0x")
        && rest[2..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .count()
            > 0
}

fn trace_terminates(flavor: TraceFlavor, line: &str) -> bool {
    let trimmed = line.trim_start();
    match flavor {
        TraceFlavor::PythonTraceback => {
            let is_indented_or_blank = line.starts_with([' ', '\t']) || line.is_empty();
            let is_continuation = trimmed.starts_with("Traceback")
                || trimmed.starts_with("File ")
                || trimmed.starts_with("During handling")
                || trimmed.starts_with("The above exception");
            if is_indented_or_blank || is_continuation {
                false
            } else {
                !trimmed.starts_with(char::is_uppercase)
            }
        }
        TraceFlavor::Js | TraceFlavor::Java => !trimmed.starts_with("at ") && !line.is_empty(),
        TraceFlavor::RustError => !trimmed.starts_with("--> ") && !line.is_empty(),
        TraceFlavor::Go => {
            !trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) && !line.is_empty()
        }
    }
}

fn is_summary_line(line: &str) -> bool {
    if line.starts_with("===") || line.starts_with("---") {
        return true;
    }
    let leading_digits = line.bytes().take_while(|b| b.is_ascii_digit()).count();
    if leading_digits > 0 && line[leading_digits..].starts_with(' ') {
        let rest = &line[leading_digits + 1..];
        for kw in &["passed", "failed", "skipped", "error", "warning"] {
            if rest.starts_with(kw) {
                return true;
            }
        }
    }
    for prefix in &[
        "Test ", "Tests ", "Tests:", "Test:", "Suite ", "Suites ", "Suites:", "Suite:",
    ] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return rest
                .chars()
                .find(|c| !c.is_whitespace())
                .is_some_and(|c| c.is_ascii_digit());
        }
    }
    for prefix in &["TOTAL", "Total", "Summary"] {
        if line.starts_with(prefix) {
            return true;
        }
    }
    for prefix in &["Build", "Compile", "Test"] {
        if line.starts_with(prefix)
            && ["succeeded", "failed", "complete"]
                .iter()
                .any(|o| line.contains(o))
        {
            return true;
        }
    }
    false
}

fn score_log_line(level: LogLevel, is_stack_trace: bool, is_summary: bool) -> f32 {
    let level_score: f32 = match level {
        LogLevel::Error | LogLevel::Fail => 1.0,
        LogLevel::Warn => 0.5,
        LogLevel::Info | LogLevel::Unknown => 0.1,
        LogLevel::Debug => 0.05,
        LogLevel::Trace => 0.02,
    };
    let stack_boost = if is_stack_trace { 0.3 } else { 0.0 };
    let summary_boost = if is_summary { 0.4 } else { 0.0 };
    (level_score + stack_boost + summary_boost).min(1.0_f32)
}

fn normalize_for_dedupe(content: &str) -> String {
    let split_at = content.find([':', '=']).unwrap_or(content.len());
    let prefix = &content[..split_at];
    let suffix = &content[split_at..];

    let stage1 = digit_regex().replace_all(suffix, "N");
    let stage2 = hex_regex().replace_all(&stage1, "ADDR");
    let stage3 = path_regex().replace_all(&stage2, "/PATH/");
    format!("{}{}", prefix, stage3)
}

fn digit_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\d+").expect("static regex compiles"))
}

fn hex_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"0x[0-9a-fA-F]+").expect("static regex compiles"))
}

fn path_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"/[\w/]+/").expect("static regex compiles"))
}

pub struct LogCompressor {
    config: LogCompressionConfig,
    formats: FormatDetector,
    levels: LevelClassifier,
}

impl LogCompressor {
    pub fn new(config: LogCompressionConfig) -> Self {
        Self {
            config,
            formats: FormatDetector::new(),
            levels: LevelClassifier::new(),
        }
    }

    /// Returns the compressed text, or `None` when the input is left untouched
    /// (too short, or not log-shaped — deferred to the truncation backstop).
    pub fn compress(&self, content: &str) -> Option<String> {
        let lines: Vec<&str> = content.split('\n').collect();
        if lines.len() < self.config.min_lines {
            return None;
        }

        let format = self.formats.detect(&lines);
        let log_lines = self.parse_lines(&lines);

        if format == LogFormat::Generic && !self.has_signal(&log_lines) {
            return None;
        }

        let selected = self.select_lines(&log_lines);
        if selected.len() >= log_lines.len() {
            return None;
        }
        Some(self.format_output(&selected, &log_lines))
    }

    fn has_signal(&self, lines: &[LogLine]) -> bool {
        lines.iter().any(|l| {
            l.is_stack_trace || matches!(l.level, LogLevel::Error | LogLevel::Fail | LogLevel::Warn)
        })
    }

    fn parse_lines(&self, lines: &[&str]) -> Vec<LogLine> {
        let mut out: Vec<LogLine> = Vec::with_capacity(lines.len());
        let mut active: Option<TraceFlavor> = None;
        let mut trace_lines = 0usize;

        for (i, line) in lines.iter().enumerate() {
            let mut entry = LogLine::new(i, line);
            entry.level = self.levels.classify(line);
            entry.is_summary = is_summary_line(line);

            if let Some(flavor) = active {
                if trace_lines >= self.config.stack_trace_max_lines
                    || trace_terminates(flavor, line)
                {
                    active = None;
                    trace_lines = 0;
                    if let Some(new_flavor) = trace_flavor_for(line) {
                        active = Some(new_flavor);
                        trace_lines = 1;
                        entry.is_stack_trace = true;
                    }
                } else {
                    entry.is_stack_trace = true;
                    trace_lines += 1;
                }
            } else if let Some(flavor) = trace_flavor_for(line) {
                active = Some(flavor);
                trace_lines = 1;
                entry.is_stack_trace = true;
            }

            entry.score = score_log_line(entry.level, entry.is_stack_trace, entry.is_summary);
            out.push(entry);
        }
        out
    }

    fn select_lines(&self, log_lines: &[LogLine]) -> Vec<LogLine> {
        let mut errors: Vec<&LogLine> = Vec::new();
        let mut fails: Vec<&LogLine> = Vec::new();
        let mut warnings: Vec<&LogLine> = Vec::new();
        let mut summaries: Vec<&LogLine> = Vec::new();
        let mut stack_traces: Vec<Vec<&LogLine>> = Vec::new();
        let mut current_stack: Vec<&LogLine> = Vec::new();

        for line in log_lines {
            match line.level {
                LogLevel::Error => errors.push(line),
                LogLevel::Fail => fails.push(line),
                LogLevel::Warn => warnings.push(line),
                _ => {}
            }
            if line.is_stack_trace {
                current_stack.push(line);
            } else if !current_stack.is_empty() {
                stack_traces.push(std::mem::take(&mut current_stack));
            }
            if line.is_summary {
                summaries.push(line);
            }
        }
        if !current_stack.is_empty() {
            stack_traces.push(current_stack);
        }

        let mut selected: BTreeSet<usize> = BTreeSet::new();

        for line in self.select_with_first_last(&errors) {
            selected.insert(line.line_number);
        }
        for line in self.select_with_first_last(&fails) {
            selected.insert(line.line_number);
        }

        let warnings = if self.config.dedupe_warnings {
            dedupe_similar(warnings)
        } else {
            warnings
        };
        for line in warnings.into_iter().take(self.config.max_warnings) {
            selected.insert(line.line_number);
        }

        for stack in stack_traces.iter().take(self.config.max_stack_traces) {
            for line in stack.iter().take(self.config.stack_trace_max_lines) {
                selected.insert(line.line_number);
            }
        }

        if self.config.keep_summary_lines {
            for line in summaries {
                selected.insert(line.line_number);
            }
        }

        let anchors: Vec<usize> = selected.iter().copied().collect();
        for idx in anchors {
            let lo = idx.saturating_sub(self.config.error_context_lines);
            let hi = (idx + self.config.error_context_lines + 1).min(log_lines.len());
            for i in lo..hi {
                selected.insert(i);
            }
        }

        let mut ordered: Vec<LogLine> =
            selected.into_iter().map(|i| log_lines[i].clone()).collect();
        if ordered.len() > self.config.max_total_lines {
            ordered.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.line_number.cmp(&b.line_number))
            });
            ordered.truncate(self.config.max_total_lines);
            ordered.sort_by_key(|l| l.line_number);
        }
        ordered
    }

    fn select_with_first_last<'a>(&self, lines: &[&'a LogLine]) -> Vec<&'a LogLine> {
        if lines.len() <= self.config.max_errors {
            return lines.to_vec();
        }
        let mut out: Vec<&LogLine> = Vec::with_capacity(self.config.max_errors);
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut push = |line: &'a LogLine, out: &mut Vec<&'a LogLine>| {
            if seen.insert(line.line_number) {
                out.push(line);
            }
        };
        push(lines[0], &mut out);
        push(lines[lines.len() - 1], &mut out);

        let mut by_score: Vec<&LogLine> = lines.to_vec();
        by_score.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.line_number.cmp(&b.line_number))
        });
        for line in by_score {
            if out.len() >= self.config.max_errors {
                break;
            }
            push(line, &mut out);
        }
        out
    }

    fn format_output(&self, selected: &[LogLine], all_lines: &[LogLine]) -> String {
        let mut output: Vec<String> = selected.iter().map(|l| l.content.clone()).collect();

        let omitted = all_lines.len().saturating_sub(selected.len());
        if omitted > 0 {
            let counts = level_counts(all_lines);
            let mut parts: Vec<String> = Vec::new();
            for (label, count) in [
                ("ERROR", counts.error),
                ("FAIL", counts.fail),
                ("WARN", counts.warn),
                ("INFO", counts.info),
            ] {
                if count > 0 {
                    parts.push(format!("{} {}", count, label));
                }
            }
            if !parts.is_empty() {
                output.push(format!("[{} lines omitted: {}]", omitted, parts.join(", ")));
            } else {
                output.push(format!("[{} lines omitted]", omitted));
            }
        }
        output.join("\n")
    }
}

#[derive(Default)]
struct LevelCounts {
    error: u64,
    fail: u64,
    warn: u64,
    info: u64,
}

fn level_counts(lines: &[LogLine]) -> LevelCounts {
    let mut c = LevelCounts::default();
    for l in lines {
        match l.level {
            LogLevel::Error => c.error += 1,
            LogLevel::Fail => c.fail += 1,
            LogLevel::Warn => c.warn += 1,
            LogLevel::Info => c.info += 1,
            _ => {}
        }
    }
    c
}

fn dedupe_similar(lines: Vec<&LogLine>) -> Vec<&LogLine> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<&LogLine> = Vec::with_capacity(lines.len());
    for line in lines {
        if seen.insert(normalize_for_dedupe(&line.content)) {
            out.push(line);
        }
    }
    out
}

pub struct LogCompressionStrategy {
    compressor: LogCompressor,
    tool_registry: Arc<ToolRegistry>,
}

impl LogCompressionStrategy {
    pub fn new(config: LogCompressionConfig, tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            compressor: LogCompressor::new(config),
            tool_registry,
        }
    }

    fn is_tool_result(message: &ConversationMessage) -> bool {
        message.role == "tool" && message.tool_call_id.is_some()
    }

    fn produces_log_output(&self, name: Option<&str>) -> bool {
        name.and_then(|n| self.tool_registry.get_tool(n))
            .is_some_and(|tool| tool.output_is_log())
    }
}

#[async_trait]
impl ContextManagementStrategy for LogCompressionStrategy {
    async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
        let mut any_compressed = false;
        for message in conversation.messages.iter_mut() {
            if !Self::is_tool_result(message) {
                continue;
            }
            if !self.produces_log_output(message.name.as_deref()) {
                continue;
            }
            let Some(content) = &message.content else {
                continue;
            };
            if let Some(compressed) = self.compressor.compress(content) {
                message.content = Some(compressed);
                any_compressed = true;
            }
        }

        if any_compressed {
            Ok(StrategyResult::Applied)
        } else {
            Ok(StrategyResult::NoChange)
        }
    }
}

#[cfg(test)]
#[path = "log_compression_strategy_tests.rs"]
mod tests;
