use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub command: String,
    pub params: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("empty IRC line")]
    EmptyLine,
}

impl Message {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let line = input.trim_matches(['\r', '\n']);
        if line.trim().is_empty() {
            return Err(ParseError::EmptyLine);
        }

        let mut parts = line.split_whitespace();
        let command = parts
            .next()
            .ok_or(ParseError::EmptyLine)?
            .to_ascii_uppercase();
        let params = parts.map(ToOwned::to_owned).collect();

        Ok(Self { command, params })
    }

    pub fn serialize(&self) -> String {
        let mut message = self.command.clone();
        if !self.params.is_empty() {
            message.push(' ');
            message.push_str(&self.params.join(" "));
        }
        message.push_str("\r\n");
        message
    }
}

#[cfg(test)]
mod tests {
    use super::Message;

    #[test]
    fn parse_command_and_params() {
        let parsed = Message::parse("privmsg #chan :hello world\r\n").expect("valid message");
        assert_eq!(parsed.command, "PRIVMSG");
        assert_eq!(parsed.params, vec!["#chan", ":hello", "world"]);
    }

    #[test]
    fn serializes_with_crlf() {
        let msg = Message {
            command: "PING".to_string(),
            params: vec!["123".to_string()],
        };
        assert_eq!(msg.serialize(), "PING 123\r\n");
    }
}
