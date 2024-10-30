use std::str::from_utf8;
use log::{error};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ParserState::*;
use crate::commands::ClientCommand;
use crate::commands::ClientCommand::*;
use crate::parser::ParseError::{InvalidInput, NotAPositiveInt};

#[derive(Debug, PartialEq, Eq)]
enum ParserState {
    OpStart,

    OpC,
    OpCo,
    OpCon,
    OpConn,
    OpConne,
    OpConnec,
    OpConnect,
    ConnectArg,

    OpP,
    OpPi,
    OpPin,
    OpPing,
    OpPo,
    OpPon,
    OpPong,
    OpPu,
    OpPub,
    PubArg,
    PubMsg,

    OpS,
    OpSu,
    OpSub,
    SubArg,

    OpU,
    OpUn,
    OpUns,
    OpUnsu,
    OpUnsub,
    UnsubArg,
}

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("invalid input")]
    InvalidInput,
    #[error("not a positive int")]
    NotAPositiveInt,
}

fn split_arg(buf: &[char]) -> Vec<Vec<char>> {
    let mut result: Vec<Vec<char>> = Vec::new();
    let mut start = None;
    for (i, c) in buf.iter().enumerate() {
        match c {
            ' ' | '\t' => {
                if let Some(start_index) = start {
                    result.push(buf[start_index..i].into());
                    start = None;
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(i);
                }
            }
        }
    }
    if let Some(start_index) = start {
        result.push(buf[start_index..].into());
    }
    result
}

fn parse_uint(chars: &[char]) -> Result<u32, ParseError> {
    let mut number = 0;
    for c in chars {
        if let Some(digit) = c.to_digit(10) {
            number = number * 10 + digit;
        } else {
            return Err(NotAPositiveInt);
        }
    }
    Ok(number)
}

pub struct ClientRequest {
    parser_state: ParserState,
    arg_buffer: Vec<char>,
    msg_buffer: Vec<u8>,
    msg_size: usize,
    args: Vec<Vec<char>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ClientConnectOpts {
    #[serde(default)]
    pub verbose: bool,
}

impl Default for ClientConnectOpts {
    fn default() -> Self {
        ClientConnectOpts{
            verbose: false,
        }
    }
}

impl ClientRequest {
    fn reset_state(&mut self) {
        self.parser_state = OpStart;
        self.arg_buffer.clear();
        self.msg_buffer.clear();
        self.args.clear();
        self.msg_size = 0;
    }

    fn parse_error(&mut self) -> Result<ClientCommand, ParseError> {
        self.reset_state();
        Err(InvalidInput)
    }

    fn return_command(&mut self, command: ClientCommand) -> Result<ClientCommand, ParseError> {
        self.reset_state();
        Ok(command)
    }

    pub fn parse(&mut self, buf: &[u8]) -> (Result<ClientCommand, ParseError>, usize) {
        let mut msg_counter: usize = 0;
        for (i, b) in buf.iter().enumerate() {
            let c: char = (*b).into();
            match self.parser_state {
                OpStart => {
                    match c {
                        'C' | 'c' => self.parser_state = OpC,
                        'P' | 'p' => self.parser_state = OpP,
                        'S' | 's' => self.parser_state = OpS,
                        'U' | 'u' => self.parser_state = OpU,
                        '\r' | '\n' => return (self.return_command(Noop), i),
                        _ => return (self.parse_error(), i),
                    }
                }

                OpC => {
                    match c {
                        'O' | 'o' => self.parser_state = OpCo,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpCo => {
                    match c {
                        'N' | 'n' => self.parser_state = OpCon,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpCon => {
                    match c {
                        'N' | 'n' => self.parser_state = OpConn,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpConn => {
                    match c {
                        'E' | 'e' => self.parser_state = OpConne,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpConne => {
                    match c {
                        'C' | 'c' => self.parser_state = OpConnec,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpConnec => {
                    match c {
                        'T' | 't' => self.parser_state = OpConnect,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpConnect => {
                    match c {
                        ' ' | '\t' => self.parser_state = ConnectArg,
                        _ => return (self.parse_error(), i),
                    }
                }

                ConnectArg => {
                    match c {
                        '\n' => {
                            let arg: String = self.arg_buffer.iter().collect();

                            if let Err(e) = serde_json::from_str::<ClientConnectOpts>(&arg[..]) {
                                error!("error parsing! {}",e);
                            }

                            return if let Ok(opts) = serde_json::from_str::<ClientConnectOpts>(&arg[..]) {
                                (self.return_command(Connect(opts)), i)
                            } else {
                                (self.parse_error(), i)
                            }
                        }
                        '\r' => {} // ignore
                        _ => {
                            self.arg_buffer.push(c);
                        }
                    }
                }

                OpP => {
                    match c {
                        'I' | 'i' => self.parser_state = OpPi,
                        'O' | 'o' => self.parser_state = OpPo,
                        'U' | 'u' => self.parser_state = OpPu,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPi => {
                    match c {
                        'N' | 'n' => self.parser_state = OpPin,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPin => {
                    match c {
                        'G' | 'g' => self.parser_state = OpPing,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPing => {
                    match c {
                        '\n' => return (self.return_command(Ping), i),
                        '\r' => {}
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPo => {
                    match c {
                        'N' | 'n' => self.parser_state = OpPon,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPon => {
                    match c {
                        'G' | 'g' => self.parser_state = OpPong,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPong => {
                    match c {
                        '\n' => return (self.return_command(Pong), i),
                        '\r' => {}
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPu => {
                    match c {
                        'B' | 'b' => self.parser_state = OpPub,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpPub => {
                    match c {
                        ' ' | '\t' => self.parser_state = PubArg,
                        _ => return (self.parse_error(), i),
                    }
                }
                PubArg => {
                    match c {
                        '\n' => {
                            let args = split_arg(&self.arg_buffer);

                            if args.len() != 2 {
                                return (self.parse_error(), i);
                            }

                            match parse_uint(&args[1]) {
                                Ok(size) => {
                                    self.args = args;
                                    self.msg_size = size as usize;
                                    self.parser_state = PubMsg;
                                }
                                Err(e) => {
                                    error!("error parsing number: {}", e);
                                    return (self.parse_error(), i);
                                }
                            }
                        }
                        '\r' => {} // ignore
                        _ => {
                            self.arg_buffer.push(c);
                        }
                    }
                }
                PubMsg => {
                    match c {
                        '\r' => {
                            let arg: String = self.args[0].iter().collect();
                            return match from_utf8(&self.msg_buffer) {
                                Ok(msg) => {
                                    (self.return_command(Pub { subject: arg, msg: msg.to_string() }), i)
                                }
                                Err(e) => {
                                    error!("error parsing utf8 PUB message for subject {}: {}", arg, e);
                                    (self.parse_error(), i)
                                }
                            };
                        }
                        // somehow for PUB we can't use \n
                        _ => {
                            msg_counter += 1;
                            if msg_counter > self.msg_size {
                                error!("message size mismatch. counter = {}, msg size = {}", msg_counter, self.msg_size);
                                return (self.parse_error(), i);
                            }
                            self.msg_buffer.push(*b);
                        }
                    }
                }

                OpS => {
                    match c {
                        'U' | 'u' => self.parser_state = OpSu,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpSu => {
                    match c {
                        'B' | 'b' => self.parser_state = OpSub,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpSub => {
                    match c {
                        ' ' | '\t' => self.parser_state = SubArg,
                        _ => return (self.parse_error(), i),
                    }
                }
                SubArg => {
                    match c {
                        '\n' => {
                            let args = split_arg(&self.arg_buffer);
                            if args.len() != 2 {
                                return (self.parse_error(), i);
                            }

                            return (self.return_command(Sub {
                                subject: args[0].iter().collect(),
                                id: args[1].iter().collect(),
                            }), i);
                        }
                        '\r' => {} // ignore
                        _ => {
                            self.arg_buffer.push(c);
                        }
                    }
                }
                OpU => {
                    match c {
                        'N' | 'n' => self.parser_state = OpUn,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpUn => {
                    match c {
                        'S' | 's' => self.parser_state = OpUns,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpUns => {
                    match c {
                        'U' | 'u' => self.parser_state = OpUnsu,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpUnsu => {
                    match c {
                        'B' | 'b' => self.parser_state = OpUnsub,
                        _ => return (self.parse_error(), i),
                    }
                }
                OpUnsub => {
                    match c {
                        ' ' | '\t' => self.parser_state = UnsubArg,
                        _ => return (self.parse_error(), i),
                    }
                }
                UnsubArg => {
                    match c {
                        '\n' => {
                            return (self.return_command(Unsub {
                                id: self.arg_buffer.iter().collect(),
                            }), i);
                        }
                        '\r' => {} // ignore
                        _ => {
                            self.arg_buffer.push(c);
                        }
                    }
                }
            }
        }


        (Ok(Noop), buf.len())
    }

    pub fn new() -> Self {
        Self {
            parser_state: ParserState::OpStart,
            arg_buffer: vec![],
            msg_buffer: vec![],
            msg_size: 0,
            args: vec![vec![]],
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }


    #[test_case("PING", OpPing; "ping")]
    #[test_case("PONG", OpPong; "pong")]
    #[test_case("CONNECT {\"verbose\": true}", ConnectArg; "connect arg")]
    #[test_case("PUB subject", PubArg; "pub arg")]
    #[test_case("PUB subject 3", PubArg; "pub arg with msg len")]
    #[test_case("PUB subject 3\r\n", PubMsg; "pub arg with msg len before message")]
    #[test_case("PUB subject 3\r\nyes", PubMsg; "pub arg with msg len and message")]
    #[test_case("SUB subject", SubArg; "sub arg")]
    #[test_case("SUB subject id", SubArg; "sub arg with id")]
    #[test_case("UNSUB subject", UnsubArg; "unsub arg with id")]
    fn test_parse_state_ok(input: &str, expected: ParserState) {
        init();
        let mut client = ClientRequest::new();
        let _ = client.parse(input.as_bytes());
        assert_eq!(expected, client.parser_state);
    }


    #[test_case("PIN\r\n", InvalidInput; "pin")]
    #[test_case("PINGX\r\n", InvalidInput; "pingx")]
    #[test_case("CONNECT\r\n", InvalidInput; "connect without arg")]
    #[test_case("CONNECT {yeah}\r\n", InvalidInput; "connect invalid arg")]
    #[test_case("PUB\r\n", InvalidInput; "pub without arg")]
    #[test_case("PUB s\r\n", InvalidInput; "pub not enough arg")]
    #[test_case("PUB subj -3\r\nyes\r\n", InvalidInput; "pub message invalid negative size")]
    #[test_case("PUB subj x\r\nyes\r\n", InvalidInput; "pub message invalid size not a number")]
    #[test_case("PUB subj 3\r\ntoolong\r\n", InvalidInput; "pub message too long")]
    #[test_case("PUB subj 30\r\nyeah\r\n", InvalidInput; "pub message too short")]
    #[test_case("SUB\r\n", InvalidInput; "sub without arg")]
    #[test_case("SUB s\r\n", InvalidInput; "sub not enough arg")]
    #[test_case("UNSUB\r\n", InvalidInput; "unsub without arg")]
    fn test_parse_fail(input: &str, expected: ParseError) {
        init();
        let mut client = ClientRequest::new();
        let actual = client.parse(input.as_bytes()).0.unwrap_err();
        assert_eq!(expected, actual);
    }

    #[test_case("CONNECT {}\r\n", Connect(ClientConnectOpts{verbose: false}); "connect command")]
    #[test_case("CONNECT\t{\"verbose\": true}\r\n", Connect(ClientConnectOpts{verbose: true}); "connect with tab and argument")]
    #[test_case("PING\r\n", Ping; "ping command")]
    #[test_case("PONG\r\n", Pong; "pong command")]
    #[test_case("SUB subject id\r\n", Sub{subject: "subject".to_string(), id: "id".to_string()}; "sub command")]
    #[test_case("SUB\tsubject\tid\r\n", Sub{subject: "subject".to_string(), id: "id".to_string()}; "sub command with tab")]
    #[test_case("PUB subject 5\r\nhello\r\n", Pub{subject: "subject".to_string(), msg: "hello".to_string()}; "pub command")]
    #[test_case("PUB\tsubject\t5\r\nhello\r\n", Pub{subject: "subject".to_string(), msg: "hello".to_string()}; "pub command with tab")]
    #[test_case("PUB subject 0\r\n\r\n", Pub{subject: "subject".to_string(), msg: "".to_string()}; "pub command empty message")]
    #[test_case("UNSUB id\r\n", Unsub{id: "id".to_string()}; "unsub command")]
    fn test_parse_ok(input: &str, expected: ClientCommand) {
        let mut client = ClientRequest::new();
        let actual = client.parse(input.as_bytes()).0.unwrap();
        assert_eq!(expected, actual);
    }


    #[test_case(vec!['s','u','p'], vec![vec!['s','u','p']]; "one arg")]
    #[test_case(vec!['s','u','p',' ',' '], vec![vec!['s','u','p']]; "one arg extra space")]
    #[test_case(vec!['s','u','p',' ','1','2','3'], vec![vec!['s','u','p'], vec!['1','2','3']]; "two args"
    )]
    #[test_case(vec!['s','u','p','\t','1','2','3'], vec![vec!['s','u','p'], vec!['1','2','3']]; "two args with tab"
    )]
    fn test_split_args(input: Vec<char>, expected_output: Vec<Vec<char>>) {
        init();
        let actual = split_arg(&input);
        assert_eq!(expected_output, actual);
    }

    #[test_case(vec!['3','6','1'], Ok(361); "positive number")]
    #[test_case(vec!['-','3','6','1'], Err(NotAPositiveInt); "negative number")]
    #[test_case(vec!['3','.','1'], Err(NotAPositiveInt); "floating number")]
    #[test_case(vec!['a','3','1'], Err(NotAPositiveInt); "not a number")]
    fn test_parse_uint(input: Vec<char>, expected_output: Result<u32, ParseError>) {
        init();
        let actual = parse_uint(&input);
        assert_eq!(expected_output, actual);
    }

    #[test_case("PIN\r\nPING", 3; "invalid ping")]
    #[test_case("PING\r\n", 5; "correct ping")]
    #[test_case("PING\r\nPING\r\n", 5; "correct ping extra ignored")]
    fn test_parse_return_bytes_read(input: &str, expected: usize) {
        let mut client = ClientRequest::new();
        let actual = client.parse(input.as_bytes()).1;
        assert_eq!(expected, actual);
    }
}
