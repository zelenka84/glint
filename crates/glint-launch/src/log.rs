//! Turning the game's output into readable log entries.
//!
//! With Mojang's logging config the game prints log4j XML events, one element
//! spread over several lines. Anything else (early JVM output, crashes) is passed
//! through as plain text.

use crate::{LogLine, LogStream};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// `INFO`, `WARN`, `ERROR`, … as reported by the game.
    pub level: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct LogParser {
    event: Option<String>,
}

impl LogParser {
    /// Feeds one output line; returns an entry once a full one is available.
    pub fn push(&mut self, line: &LogLine) -> Option<LogEntry> {
        let text = line.text.as_str();
        if let Some(event) = self.event.as_mut() {
            event.push('\n');
            event.push_str(text);
            if text.trim_start().starts_with("</log4j:Event>") {
                let event = self.event.take().expect("event in progress");
                return Some(parse_event(&event));
            }
            return None;
        }
        if text.trim_start().starts_with("<log4j:Event") {
            self.event = Some(text.to_owned());
            return None;
        }
        if text.trim().is_empty() {
            return None;
        }
        let level = if line.stream == LogStream::Stderr { "ERROR" } else { "INFO" };
        Some(LogEntry { level: level.into(), message: text.to_owned() })
    }
}

fn parse_event(xml: &str) -> LogEntry {
    let level = attribute(xml, "level").unwrap_or("INFO").to_owned();
    let mut message = element_cdata(xml, "log4j:Message").unwrap_or_default().to_owned();
    if let Some(throwable) = element_cdata(xml, "log4j:Throwable") {
        message.push('\n');
        message.push_str(throwable.trim_end());
    }
    LogEntry { level, message }
}

fn attribute<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let start = xml.find(&format!(" {name}=\""))? + name.len() + 3;
    let len = xml[start..].find('"')?;
    Some(&xml[start..start + len])
}

fn element_cdata<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = xml.find(&format!("<{tag}>"))?;
    let rest = &xml[open..];
    let start = rest.find("<![CDATA[")? + "<![CDATA[".len();
    let len = rest[start..].find("]]>")?;
    Some(&rest[start..start + len])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(parser: &mut LogParser, lines: &[&str]) -> Vec<LogEntry> {
        lines
            .iter()
            .filter_map(|l| parser.push(&LogLine { stream: LogStream::Stdout, text: (*l).to_owned() }))
            .collect()
    }

    #[test]
    fn parses_xml_events_and_plain_lines() {
        let mut parser = LogParser::default();
        let entries = feed(
            &mut parser,
            &[
                "Picked up JAVA_TOOL_OPTIONS",
                r#"  <log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1" level="INFO" thread="Render thread">"#,
                "    <log4j:Message><![CDATA[Setting user: Player]]></log4j:Message>",
                "  </log4j:Event>",
                r#"  <log4j:Event logger="x" timestamp="2" level="ERROR" thread="main">"#,
                "    <log4j:Message><![CDATA[Failed to fetch user properties]]></log4j:Message>",
                "    <log4j:Throwable><![CDATA[java.lang.Exception: boom",
                "\tat a.b(C.java:1)",
                "]]></log4j:Throwable>",
                "  </log4j:Event>",
                "",
            ],
        );
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], LogEntry { level: "INFO".into(), message: "Picked up JAVA_TOOL_OPTIONS".into() });
        assert_eq!(entries[1], LogEntry { level: "INFO".into(), message: "Setting user: Player".into() });
        assert_eq!(entries[2].level, "ERROR");
        assert_eq!(entries[2].message, "Failed to fetch user properties\njava.lang.Exception: boom\n\tat a.b(C.java:1)");
    }

    #[test]
    fn stderr_lines_are_errors() {
        let mut parser = LogParser::default();
        let entry = parser.push(&LogLine { stream: LogStream::Stderr, text: "Exception in thread".into() }).unwrap();
        assert_eq!(entry.level, "ERROR");
    }
}
