/// GDB command management and execution
/// 
/// This module provides a high-level interface for executing GDB commands
/// and managing their responses, with support for common GDB operations.

use std::collections::HashMap;
use crate::communication::{GdbCommunication, CommunicationError};
use crate::types::Value as GdbValue;

pub type Result<T> = std::result::Result<T, CommandError>;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Communication error: {0}")]
    Communication(#[from] CommunicationError),
    #[error("GDB command failed: {0}")]
    GdbError(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Type conversion error: {0}")]
    TypeError(String),
}

/// High-level interface for GDB commands
pub struct GdbCommands {
    comm: GdbCommunication,
}

impl GdbCommands {
    pub fn new(comm: GdbCommunication) -> Self {
        Self { comm }
    }
    
    pub fn communication_mut(&mut self) -> &mut GdbCommunication {
        &mut self.comm
    }
    
    pub fn communication(&self) -> &GdbCommunication {
        &self.comm
    }
    
    /// Load executable file
    pub async fn file_exec_and_symbols(&mut self, path: &str) -> Result<()> {
        let cmd = format!("file-exec-and-symbols \"{}\"", path);
        let _result = self.comm.send_command(&cmd).await?;
        Ok(())
    }
    
    /// Set a breakpoint
    pub async fn break_insert(&mut self, location: &str) -> Result<u32> {
        let cmd = format!("break-insert {}", location);
        let result = self.comm.send_command(&cmd).await?;
        
        let bkpt = result.results.get("bkpt")
            .ok_or_else(|| CommandError::MissingField("bkpt".to_string()))?;
            
        if let GdbValue::Tuple(ref tuple) = bkpt {
            if let Some(GdbValue::String(ref number)) = tuple.get("number") {
                number.parse().map_err(|_| CommandError::TypeError("Invalid breakpoint number".to_string()))
            } else {
                Err(CommandError::MissingField("number".to_string()))
            }
        } else {
            Err(CommandError::TypeError("Expected tuple for bkpt".to_string()))
        }
    }
    
    /// Delete a breakpoint
    pub async fn break_delete(&mut self, number: u32) -> Result<()> {
        let cmd = format!("break-delete {}", number);
        let _result = self.comm.send_command(&cmd).await?;
        Ok(())
    }
    
    /// List all breakpoints
    pub async fn break_list(&mut self) -> Result<Vec<Breakpoint>> {
        let result = self.comm.send_command("break-list").await?;
        
        let body = result.results.get("BreakpointTable")
            .and_then(|v| match v {
                GdbValue::Tuple(tuple) => tuple.get("body"),
                _ => None,
            })
            .ok_or_else(|| CommandError::MissingField("BreakpointTable.body".to_string()))?;
        
        if let GdbValue::List(ref breakpoints) = body {
            let mut result = Vec::new();
            for bp in breakpoints {
                if let GdbValue::Tuple(ref tuple) = bp {
                    result.push(Breakpoint::from_tuple(tuple)?);
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }
    
    /// Start execution
    pub async fn exec_run(&mut self, args: Option<&str>) -> Result<()> {
        let cmd = if let Some(args) = args {
            format!("exec-run {}", args)
        } else {
            "exec-run".to_string()
        };
        let _result = self.comm.send_command(&cmd).await?;
        Ok(())
    }
    
    /// Continue execution
    pub async fn exec_continue(&mut self) -> Result<()> {
        let _result = self.comm.send_command("exec-continue").await?;
        Ok(())
    }
    
    /// Step one instruction
    pub async fn exec_step(&mut self) -> Result<()> {
        let _result = self.comm.send_command("exec-step").await?;
        Ok(())
    }
    
    /// Step over (next instruction)
    pub async fn exec_next(&mut self) -> Result<()> {
        let _result = self.comm.send_command("exec-next").await?;
        Ok(())
    }
    
    /// Step into instruction
    pub async fn exec_step_instruction(&mut self) -> Result<()> {
        let _result = self.comm.send_command("exec-step-instruction").await?;
        Ok(())
    }
    
    /// Step over instruction
    pub async fn exec_next_instruction(&mut self) -> Result<()> {
        let _result = self.comm.send_command("exec-next-instruction").await?;
        Ok(())
    }
    
    /// Interrupt execution
    pub async fn exec_interrupt(&mut self) -> Result<()> {
        let _result = self.comm.send_command("exec-interrupt").await?;
        Ok(())
    }
    
    /// Get stack frames
    pub async fn stack_list_frames(&mut self, low_frame: Option<u32>, high_frame: Option<u32>) -> Result<Vec<StackFrame>> {
        let cmd = match (low_frame, high_frame) {
            (Some(low), Some(high)) => format!("stack-list-frames {} {}", low, high),
            (Some(low), None) => format!("stack-list-frames {}", low),
            _ => "stack-list-frames".to_string(),
        };
        
        let result = self.comm.send_command(&cmd).await?;
        
        let stack = result.results.get("stack")
            .ok_or_else(|| CommandError::MissingField("stack".to_string()))?;
        
        if let GdbValue::List(ref frames) = stack {
            let mut result = Vec::new();
            for frame in frames {
                if let GdbValue::Tuple(ref tuple) = frame {
                    result.push(StackFrame::from_tuple(tuple)?);
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }
    
    /// List local variables
    pub async fn stack_list_variables(&mut self, print_values: bool) -> Result<Vec<Variable>> {
        let cmd = if print_values {
            "stack-list-variables --all-values"
        } else {
            "stack-list-variables --no-values"
        };
        
        let result = self.comm.send_command(cmd).await?;
        
        let variables = result.results.get("variables")
            .ok_or_else(|| CommandError::MissingField("variables".to_string()))?;
        
        if let GdbValue::List(ref vars) = variables {
            let mut result = Vec::new();
            for var in vars {
                if let GdbValue::Tuple(ref tuple) = var {
                    result.push(Variable::from_tuple(tuple)?);
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }
    
    /// Evaluate expression
    pub async fn data_evaluate_expression(&mut self, expression: &str) -> Result<String> {
        let cmd = format!("data-evaluate-expression \"{}\"", expression);
        let result = self.comm.send_command(&cmd).await?;
        
        let value = result.results.get("value")
            .and_then(|v| v.as_string())
            .ok_or_else(|| CommandError::MissingField("value".to_string()))?;
        
        Ok(value.to_string())
    }
    
    /// Get target information
    pub async fn target_select(&mut self, target_type: &str, params: &str) -> Result<()> {
        let cmd = format!("target-select {} {}", target_type, params);
        let _result = self.comm.send_command(&cmd).await?;
        Ok(())
    }
    
    /// Check if GDB is running
    pub fn is_running(&self) -> bool {
        self.comm.is_running()
    }
    
    /// Stop GDB communication
    pub fn stop(&mut self) {
        self.comm.stop();
    }
}

/// Represents a breakpoint
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub number: u32,
    pub enabled: bool,
    pub addr: Option<String>,
    pub func: Option<String>,
    pub file: Option<String>,
    pub fullname: Option<String>,
    pub line: Option<u32>,
    pub times: u32,
}

impl Breakpoint {
    pub fn from_tuple(tuple: &HashMap<String, GdbValue>) -> Result<Self> {
        let number = tuple.get("number")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| CommandError::MissingField("number".to_string()))?;
        
        let enabled = tuple.get("enabled")
            .and_then(|v| v.as_string())
            .map(|s| s == "y")
            .unwrap_or(false);
        
        let line = tuple.get("line")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse().ok());
        
        let times = tuple.get("times")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        
        Ok(Breakpoint {
            number,
            enabled,
            addr: tuple.get("addr").and_then(|v| v.as_string()).map(|s| s.to_string()),
            func: tuple.get("func").and_then(|v| v.as_string()).map(|s| s.to_string()),
            file: tuple.get("file").and_then(|v| v.as_string()).map(|s| s.to_string()),
            fullname: tuple.get("fullname").and_then(|v| v.as_string()).map(|s| s.to_string()),
            line,
            times,
        })
    }
}

/// Represents a stack frame
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub level: u32,
    pub addr: String,
    pub func: Option<String>,
    pub file: Option<String>,
    pub fullname: Option<String>,
    pub line: Option<u32>,
}

impl StackFrame {
    pub fn from_tuple(tuple: &HashMap<String, GdbValue>) -> Result<Self> {
        let level = tuple.get("level")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| CommandError::MissingField("level".to_string()))?;
        
        let addr = tuple.get("addr")
            .and_then(|v| v.as_string())
            .ok_or_else(|| CommandError::MissingField("addr".to_string()))?
            .to_string();
        
        let line = tuple.get("line")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse().ok());
        
        Ok(StackFrame {
            level,
            addr,
            func: tuple.get("func").and_then(|v| v.as_string()).map(|s| s.to_string()),
            file: tuple.get("file").and_then(|v| v.as_string()).map(|s| s.to_string()),
            fullname: tuple.get("fullname").and_then(|v| v.as_string()).map(|s| s.to_string()),
            line,
        })
    }
}

/// Represents a variable
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: Option<String>,
    pub var_type: Option<String>,
}

impl Variable {
    pub fn from_tuple(tuple: &HashMap<String, GdbValue>) -> Result<Self> {
        let name = tuple.get("name")
            .and_then(|v| v.as_string())
            .ok_or_else(|| CommandError::MissingField("name".to_string()))?
            .to_string();
        
        Ok(Variable {
            name,
            value: tuple.get("value").and_then(|v| v.as_string()).map(|s| s.to_string()),
            var_type: tuple.get("type").and_then(|v| v.as_string()).map(|s| s.to_string()),
        })
    }
}
