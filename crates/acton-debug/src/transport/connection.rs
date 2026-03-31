use crate::transport::request_parser::{IncomingRequest, poll_request as poll_incoming_request};
use dap::base_message::{BaseMessage, Sendable};
use dap::events::Event;
use dap::prelude::Response;
use serde_json::Value;
use std::io::{BufRead, BufWriter, Write};

/// Low-level DAP wire connection with tolerant request parsing and exact framing.
pub(crate) struct DapConnection<R: BufRead, W: Write> {
    input_buffer: R,
    output_buffer: BufWriter<W>,
    sequence_number: i64,
}

impl<R: BufRead, W: Write> DapConnection<R, W> {
    pub(crate) fn new(input: R, output: W) -> Self {
        Self {
            input_buffer: input,
            output_buffer: BufWriter::new(output),
            sequence_number: 0,
        }
    }

    pub(crate) fn poll_request(&mut self) -> anyhow::Result<Option<IncomingRequest>> {
        poll_incoming_request(&mut self.input_buffer)
    }

    pub(crate) fn respond(&mut self, response: Response) -> anyhow::Result<()> {
        self.send(Sendable::Response(response))
    }

    pub(crate) fn send_event(&mut self, event: Event) -> anyhow::Result<()> {
        self.send(Sendable::Event(event))
    }

    pub(crate) fn respond_custom_success(
        &mut self,
        request_seq: i64,
        command: &str,
    ) -> anyhow::Result<()> {
        self.sequence_number += 1;
        self.send_json_value(&serde_json::json!({
            "seq": self.sequence_number,
            "type": "response",
            "request_seq": request_seq,
            "success": true,
            "command": command,
        }))
    }

    fn send(&mut self, body: Sendable) -> anyhow::Result<()> {
        self.sequence_number += 1;
        let message = BaseMessage {
            seq: self.sequence_number,
            message: body,
        };
        self.send_json_value(&serde_json::to_value(message)?)
    }

    fn send_json_value(&mut self, value: &Value) -> anyhow::Result<()> {
        let json = serde_json::to_string(value)?;
        write!(self.output_buffer, "Content-Length: {}\r\n\r\n", json.len())?;
        write!(self.output_buffer, "{json}")?;
        self.output_buffer.flush()?;
        Ok(())
    }
}
