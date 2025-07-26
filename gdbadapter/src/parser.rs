/// GDB/MI output parser
/// 
/// This module handles parsing GDB/MI protocol output into structured data.

use crate::types::*;
use regex::Regex;
use std::collections::HashMap;

/// Parse a line of GDB/MI output
pub fn parse_gdb_output(line: &str) -> Result<GdbOutput, String> {
    let line = line.trim();
    
    if line.is_empty() || line == "(gdb)" {
        return Err("Empty or prompt line".into());
    }
    
    // Check for stream records first (single character prefix)
    if let Some(stream) = parse_stream_record(line) {
        return Ok(GdbOutput::Stream(stream));
    }
    
    // Check for async records (* or =)
    if line.starts_with('*') || line.starts_with('=') {
        return parse_async_record(line).map(GdbOutput::Async);
    }
    
    // Check for result records (^)
    if line.starts_with('^') || line.chars().any(|c| c == '^') {
        return parse_result_record(line).map(GdbOutput::Result);
    }
    
    Err(format!("Unknown GDB/MI output format: {}", line))
}

/// Parse a stream record (console, target, or log output)
fn parse_stream_record(line: &str) -> Option<StreamRecord> {
    if line.len() < 2 {
        return None;
    }
    
    let (stream_type, content) = match line.chars().next()? {
        '~' => (StreamType::Console, &line[1..]),
        '@' => (StreamType::Target, &line[1..]),
        '&' => (StreamType::Log, &line[1..]),
        _ => return None,
    };
    
    // Parse the C-string content
    let content = parse_c_string(content).unwrap_or_else(|| content.to_string());
    
    Some(StreamRecord {
        stream_type,
        content,
    })
}

/// Parse a result record
fn parse_result_record(line: &str) -> Result<GdbResult, String> {
    let re = Regex::new(r"^(?:(\d+))?\^(done|running|connected|error|exit)(?:,(.*))?$")
        .map_err(|e| format!("Regex error: {}", e))?;
    
    let caps = re.captures(line).ok_or_else(|| {
        format!("Invalid result record format: {}", line)
    })?;
    
    let token = caps.get(1)
        .and_then(|m| m.as_str().parse().ok());
    
    let class = match caps.get(2).unwrap().as_str() {
        "done" => ResultClass::Done,
        "running" => ResultClass::Running,
        "connected" => ResultClass::Connected,
        "error" => ResultClass::Error,
        "exit" => ResultClass::Exit,
        other => return Err(format!("Unknown result class: {}", other)),
    };
    
    let results = if let Some(results_str) = caps.get(3) {
        parse_results(results_str.as_str())?
    } else {
        HashMap::new()
    };
    
    Ok(GdbResult {
        token,
        class,
        results,
    })
}

/// Parse an async record
fn parse_async_record(line: &str) -> Result<AsyncRecord, String> {
    let (_prefix, rest) = if line.starts_with('*') {
        ('*', &line[1..])
    } else if line.starts_with('=') {
        ('=', &line[1..])
    } else {
        return Err("Invalid async record prefix".into());
    };
    
    // Find the first comma to separate class from results
    let (class_str, results_str) = if let Some(comma_pos) = rest.find(',') {
        (&rest[..comma_pos], Some(&rest[comma_pos + 1..]))
    } else {
        (rest, None)
    };
    
    let class = match class_str {
        "running" => AsyncClass::Running,
        "stopped" => AsyncClass::Stopped,
        "thread-group-added" => AsyncClass::ThreadGroupAdded,
        "thread-group-removed" => AsyncClass::ThreadGroupRemoved,
        "thread-group-started" => AsyncClass::ThreadGroupStarted,
        "thread-group-exited" => AsyncClass::ThreadGroupExited,
        "thread-created" => AsyncClass::ThreadCreated,
        "thread-exited" => AsyncClass::ThreadExited,
        "thread-selected" => AsyncClass::ThreadSelected,
        "library-loaded" => AsyncClass::LibraryLoaded,
        "library-unloaded" => AsyncClass::LibraryUnloaded,
        "traceframe-changed" => AsyncClass::TraceframeChanged,
        "tsv-created" => AsyncClass::TsvCreated,
        "tsv-deleted" => AsyncClass::TsvDeleted,
        "tsv-modified" => AsyncClass::TsvModified,
        "breakpoint-created" => AsyncClass::BreakpointCreated,
        "breakpoint-modified" => AsyncClass::BreakpointModified,
        "breakpoint-deleted" => AsyncClass::BreakpointDeleted,
        "record-started" => AsyncClass::RecordStarted,
        "record-stopped" => AsyncClass::RecordStopped,
        "cmd-param-changed" => AsyncClass::CmdParamChanged,
        "memory-changed" => AsyncClass::MemoryChanged,
        other => return Err(format!("Unknown async class: {}", other)),
    };
    
    let results = if let Some(results_str) = results_str {
        parse_results(results_str)?
    } else {
        HashMap::new()
    };
    
    Ok(AsyncRecord {
        token: None, // Async records don't typically have tokens
        class,
        results,
    })
}

/// Parse result key-value pairs
fn parse_results(input: &str) -> Result<HashMap<String, Value>, String> {
    let mut results = HashMap::new();
    let mut chars = input.chars().peekable();
    
    while chars.peek().is_some() {
        // Skip whitespace
        while chars.peek() == Some(&' ') {
            chars.next();
        }
        
        if chars.peek().is_none() {
            break;
        }
        
        // Parse key
        let key = parse_identifier(&mut chars)?;
        
        // Expect '='
        if chars.next() != Some('=') {
            return Err("Expected '=' after key".into());
        }
        
        // Parse value
        let value = parse_value(&mut chars)?;
        
        results.insert(key, value);
        
        // Skip optional comma
        if chars.peek() == Some(&',') {
            chars.next();
        }
    }
    
    Ok(results)
}

/// Parse an identifier (key name)
fn parse_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, String> {
    let mut identifier = String::new();
    
    while let Some(&ch) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            identifier.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    
    if identifier.is_empty() {
        return Err("Empty identifier".into());
    }
    
    Ok(identifier)
}

/// Parse a value (string, list, or tuple)
fn parse_value(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Value, String> {
    match chars.peek() {
        Some('"') => {
            chars.next(); // consume opening quote
            let mut string_val = String::new();
            let mut escaped = false;
            
            while let Some(ch) = chars.next() {
                if escaped {
                    match ch {
                        'n' => string_val.push('\n'),
                        't' => string_val.push('\t'),
                        'r' => string_val.push('\r'),
                        '\\' => string_val.push('\\'),
                        '"' => string_val.push('"'),
                        other => {
                            string_val.push('\\');
                            string_val.push(other);
                        }
                    }
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    break;
                } else {
                    string_val.push(ch);
                }
            }
            
            Ok(Value::String(string_val))
        }
        Some('[') => {
            chars.next(); // consume opening bracket
            let mut list = Vec::new();
            
            while chars.peek() != Some(&']') && chars.peek().is_some() {
                let value = parse_value(chars)?;
                list.push(value);
                
                if chars.peek() == Some(&',') {
                    chars.next();
                }
            }
            
            if chars.next() != Some(']') {
                return Err("Expected closing bracket".into());
            }
            
            Ok(Value::List(list))
        }
        Some('{') => {
            chars.next(); // consume opening brace
            let mut tuple = HashMap::new();
            
            while chars.peek() != Some(&'}') && chars.peek().is_some() {
                let key = parse_identifier(chars)?;
                
                if chars.next() != Some('=') {
                    return Err("Expected '=' in tuple".into());
                }
                
                let value = parse_value(chars)?;
                tuple.insert(key, value);
                
                if chars.peek() == Some(&',') {
                    chars.next();
                }
            }
            
            if chars.next() != Some('}') {
                return Err("Expected closing brace".into());
            }
            
            Ok(Value::Tuple(tuple))
        }
        _ => {
            // Try to parse as unquoted string until comma, space, or end
            let mut string_val = String::new();
            
            while let Some(&ch) = chars.peek() {
                if ch == ',' || ch == ']' || ch == '}' || ch == ' ' {
                    break;
                }
                string_val.push(ch);
                chars.next();
            }
            
            if string_val.is_empty() {
                return Err("Empty value".into());
            }
            
            Ok(Value::String(string_val))
        }
    }
}

/// Parse a C-style string (removes quotes and handles escape sequences)
fn parse_c_string(input: &str) -> Option<String> {
    if input.len() < 2 || !input.starts_with('"') || !input.ends_with('"') {
        return None;
    }
    
    let content = &input[1..input.len() - 1];
    let mut result = String::new();
    let mut chars = content.chars();
    
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_c_string() {
        assert_eq!(parse_c_string("\"Hello\""), Some("Hello".to_string()));
        assert_eq!(parse_c_string("\"Hello\\nWorld\""), Some("Hello\nWorld".to_string()));
        assert_eq!(parse_c_string("\"Hello\\\\World\""), Some("Hello\\World".to_string()));
        assert_eq!(parse_c_string("\"Hello\\\"World\""), Some("Hello\"World".to_string()));
        assert_eq!(parse_c_string("Hello"), None);
    }
    
    #[test]
    fn test_parse_simple_results() {
        let input = "msg=\"test message\"";
        let results = parse_results(input).unwrap();
        
        assert_eq!(results.len(), 1);
        assert_eq!(results.get("msg").unwrap().as_string(), Some("test message"));
    }
    
    #[test]
    fn test_parse_multiple_results() {
        let input = "reason=\"breakpoint-hit\",thread-id=\"1\"";
        let results = parse_results(input).unwrap();
        
        assert_eq!(results.len(), 2);
        assert_eq!(results.get("reason").unwrap().as_string(), Some("breakpoint-hit"));
        assert_eq!(results.get("thread-id").unwrap().as_string(), Some("1"));
    }
    
    #[test]
    fn test_parse_tuple_value() {
        let input = "bkpt={number=\"1\",type=\"breakpoint\"}";
        let results = parse_results(input).unwrap();
        
        assert_eq!(results.len(), 1);
        let bkpt = results.get("bkpt").unwrap().as_tuple().unwrap();
        assert_eq!(bkpt.get("number").unwrap().as_string(), Some("1"));
        assert_eq!(bkpt.get("type").unwrap().as_string(), Some("breakpoint"));
    }
    
    #[test]
    fn test_parse_list_value() {
        let input = "thread-groups=[\"i1\"]";
        let results = parse_results(input).unwrap();
        
        assert_eq!(results.len(), 1);
        let groups = results.get("thread-groups").unwrap().as_list().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].as_string(), Some("i1"));
    }
}
