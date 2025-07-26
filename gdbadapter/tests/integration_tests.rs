/// Integration tests for the GDB adapter
/// 
/// These tests demonstrate how to use the GDB adapter and test parsing functionality

use gdbadapter::*;

#[test]
fn test_gdb_adapter_creation() {
    let adapter = GdbAdapter::new();
    assert!(!adapter.is_running());
}

#[test]
fn test_parse_complex_breakpoint_result() {
    let input = r#"^done,bkpt={number="1",type="breakpoint",disp="keep",enabled="y",addr="0x08048564",func="main",file="myprog.c",fullname="/home/user/myprog.c",line="68",thread-groups=["i1"],times="0"}"#;
    
    let result = parse_gdb_output(input).unwrap();
    
    match result {
        GdbOutput::Result(result) => {
            assert_eq!(result.class, ResultClass::Done);
            assert_eq!(result.token, None);
            
            // Check that we have the breakpoint data
            assert!(result.results.contains_key("bkpt"));
            let bkpt = result.results.get("bkpt").unwrap();
            
            if let Value::Tuple(bkpt_data) = bkpt {
                assert_eq!(bkpt_data.get("number").unwrap().as_string(), Some("1"));
                assert_eq!(bkpt_data.get("type").unwrap().as_string(), Some("breakpoint"));
                assert_eq!(bkpt_data.get("func").unwrap().as_string(), Some("main"));
                assert_eq!(bkpt_data.get("file").unwrap().as_string(), Some("myprog.c"));
                assert_eq!(bkpt_data.get("line").unwrap().as_string(), Some("68"));
            } else {
                panic!("Expected tuple for bkpt field");
            }
        }
        _ => panic!("Expected result record"),
    }
}

#[test]
fn test_parse_stopped_with_frame_info() {
    let input = r#"*stopped,reason="breakpoint-hit",disp="keep",bkptno="1",thread-id="0",frame={addr="0x08048564",func="main",args=[{name="argc",value="1"},{name="argv",value="0xbfc4d4d4"}],file="myprog.c",fullname="/home/user/myprog.c",line="68",arch="i386:x86_64"}"#;
    
    let result = parse_gdb_output(input).unwrap();
    
    match result {
        GdbOutput::Async(async_record) => {
            assert_eq!(async_record.class, AsyncClass::Stopped);
            
            // Check reason
            assert_eq!(
                async_record.results.get("reason").unwrap().as_string(), 
                Some("breakpoint-hit")
            );
            
            // Check frame info
            assert!(async_record.results.contains_key("frame"));
            let frame = async_record.results.get("frame").unwrap();
            
            if let Value::Tuple(frame_data) = frame {
                assert_eq!(frame_data.get("func").unwrap().as_string(), Some("main"));
                assert_eq!(frame_data.get("file").unwrap().as_string(), Some("myprog.c"));
                assert_eq!(frame_data.get("line").unwrap().as_string(), Some("68"));
                
                // Check args
                if let Some(Value::List(args)) = frame_data.get("args") {
                    assert_eq!(args.len(), 2);
                    
                    if let Value::Tuple(arg1) = &args[0] {
                        assert_eq!(arg1.get("name").unwrap().as_string(), Some("argc"));
                        assert_eq!(arg1.get("value").unwrap().as_string(), Some("1"));
                    }
                }
            }
        }
        _ => panic!("Expected async record"),
    }
}

#[test]
fn test_parse_error_with_message() {
    let input = r#"^error,msg="No symbol table is loaded.  Use the \"file\" command.",code="undefined-command""#;
    
    let result = parse_gdb_output(input).unwrap();
    
    match result {
        GdbOutput::Result(result) => {
            assert_eq!(result.class, ResultClass::Error);
            
            assert_eq!(
                result.results.get("msg").unwrap().as_string(),
                Some("No symbol table is loaded.  Use the \"file\" command.")
            );
            
            assert_eq!(
                result.results.get("code").unwrap().as_string(),
                Some("undefined-command")
            );
        }
        _ => panic!("Expected result record"),
    }
}

#[test]
fn test_parse_thread_group_notifications() {
    let inputs = [
        r#"=thread-group-added,id="i1""#,
        r#"=thread-group-started,id="i1",pid="28655""#,
        r#"=thread-created,id="1",group-id="i1""#,
        r#"=thread-selected,id="1""#,
    ];
    
    for input in &inputs {
        let result = parse_gdb_output(input).unwrap();
        
        match result {
            GdbOutput::Async(async_record) => {
                match async_record.class {
                    AsyncClass::ThreadGroupAdded |
                    AsyncClass::ThreadGroupStarted |
                    AsyncClass::ThreadCreated |
                    AsyncClass::ThreadSelected => {
                        // Valid async classes
                        assert!(!async_record.results.is_empty());
                    }
                    _ => panic!("Unexpected async class for input: {}", input),
                }
            }
            _ => panic!("Expected async record for input: {}", input),
        }
    }
}

#[test]
fn test_stop_reason_parsing() {
    let reasons = [
        ("breakpoint-hit", StopReason::BreakpointHit),
        ("end-stepping-range", StopReason::EndSteppingRange),
        ("exited-normally", StopReason::ExitedNormally),
        ("signal-received", StopReason::SignalReceived),
    ];
    
    for (reason_str, expected_reason) in &reasons {
        let parsed = StopReason::from_str(reason_str).unwrap();
        assert_eq!(parsed, *expected_reason);
        assert_eq!(parsed.to_string(), *reason_str);
    }
}
