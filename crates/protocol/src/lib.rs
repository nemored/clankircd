use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub prefix: Option<String>,
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

        let mut rest = line;
        let prefix = if let Some(stripped) = rest.strip_prefix(':') {
            if let Some((raw_prefix, remainder)) = stripped.split_once(' ') {
                rest = remainder.trim_start();
                Some(raw_prefix.to_string())
            } else {
                return Err(ParseError::EmptyLine);
            }
        } else {
            None
        };

        let mut params = Vec::new();
        let command;

        if let Some((raw_command, remainder)) = rest.split_once(' ') {
            command = raw_command.to_ascii_uppercase();
            let mut trailing = remainder.trim_start();
            while !trailing.is_empty() {
                if let Some(stripped) = trailing.strip_prefix(':') {
                    params.push(stripped.to_string());
                    break;
                }

                if let Some((param, remainder)) = trailing.split_once(' ') {
                    params.push(param.to_string());
                    trailing = remainder.trim_start();
                } else {
                    params.push(trailing.to_string());
                    break;
                }
            }
        } else {
            command = rest.to_ascii_uppercase();
        }

        Ok(Self {
            prefix,
            command,
            params,
        })
    }

    pub fn serialize(&self) -> String {
        let mut message = String::new();
        if let Some(prefix) = &self.prefix {
            message.push(':');
            message.push_str(prefix);
            message.push(' ');
        }

        message.push_str(&self.command);
        for (index, param) in self.params.iter().enumerate() {
            message.push(' ');
            let is_last = index == self.params.len() - 1;
            let requires_trailing =
                is_last && (param.contains(' ') || param.starts_with(':') || param.is_empty());
            if requires_trailing {
                message.push(':');
            }
            message.push_str(param);
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
        assert_eq!(parsed.params, vec!["#chan", "hello world"]);
    }

    #[test]
    fn parse_prefix() {
        let parsed = Message::parse(":nick!u@h NOTICE #chan :hi there").expect("valid");
        assert_eq!(parsed.prefix.as_deref(), Some("nick!u@h"));
        assert_eq!(parsed.command, "NOTICE");
    }

    #[test]
    fn serializes_with_crlf() {
        let msg = Message {
            prefix: None,
            command: "PING".to_string(),
            params: vec!["123".to_string()],
        };
        assert_eq!(msg.serialize(), "PING 123\r\n");
    }

    #[test]
    fn serializes_trailing_param() {
        let msg = Message {
            prefix: Some("server.test".to_string()),
            command: "001".to_string(),
            params: vec!["nick".to_string(), "Welcome to clankircd".to_string()],
        };
        assert_eq!(
            msg.serialize(),
            ":server.test 001 nick :Welcome to clankircd\r\n"
        );
    }
}
