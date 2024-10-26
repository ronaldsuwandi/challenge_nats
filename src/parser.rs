use std::str::from_utf8;
use log::{error};
use thiserror::Error;
use ParserState::*;
use Command::*;
use crate::parser::ParseError::{InvalidInput, NotAPositiveInt};

#[derive(Debug, PartialEq, Eq)]
enum ParserState {
    OP_START,

    OP_C,
    OP_CO,
    OP_CON,
    OP_CONN,
    OP_CONNE,
    OP_CONNEC,
    OP_CONNECT,
    CONNECT_ARG,

    OP_P,
    OP_PI,
    OP_PIN,
    OP_PING,
    OP_PO,
    OP_PON,
    OP_PONG,
    OP_PU,
    OP_PUB,
    PUB_ARG,
    PUB_MSG,

    OP_S,
    OP_SU,
    OP_SUB,

    SUB_ARG,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Noop,
    Connect(String),
    Pub{subject: String, msg: String},
    Sub{subject: String, id: String},
    Ping,
    Pong,
}

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("invalid input")]
    InvalidInput,
    #[error("not a positive int")]
    NotAPositiveInt
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

impl ClientRequest {
    fn reset_state(&mut self) {
        self.parser_state = OP_START;
        self.arg_buffer.clear();
        self.msg_buffer.clear();
        self.args.clear();
        self.msg_size = 0;
    }

    fn parse_error(&mut self) -> Result<Command, ParseError> {
        self.reset_state();
        Err(InvalidInput)
    }

    fn return_command(&mut self, command: Command) -> Result<Command, ParseError> {
        self.reset_state();
        Ok(command)
    }

    pub fn parse(&mut self, buf: &[u8]) -> Result<Command, ParseError> {
        let mut msg_counter: usize = 0;

        for b in buf {
            let c: char = (*b).into();

            match self.parser_state {
                OP_START => {
                    match c {
                        'C' | 'c' => {
                            self.parser_state = OP_C;
                        }
                        'P' | 'p' => {
                            self.parser_state = OP_P;
                        }
                        'S' | 's' => {
                            self.parser_state = OP_S;
                        }
                        _ => return self.parse_error()
                    }
                }

                OP_C => {
                    match c {
                        'O' | 'o' => self.parser_state = OP_CO,
                        _ => return self.parse_error(),
                    }
                }
                OP_CO => {
                    match c {
                        'N' | 'n' => self.parser_state = OP_CON,
                        _ => return self.parse_error(),
                    }
                }
                OP_CON => {
                    match c {
                        'N' | 'n' => self.parser_state = OP_CONN,
                        _ => return self.parse_error(),
                    }
                }
                OP_CONN => {
                    match c {
                        'E' | 'e' => self.parser_state = OP_CONNE,
                        _ => return self.parse_error(),
                    }
                }
                OP_CONNE => {
                    match c {
                        'C' | 'c' => self.parser_state = OP_CONNEC,
                        _ => return self.parse_error(),
                    }
                }
                OP_CONNEC => {
                    match c {
                        'T' | 't' => self.parser_state = OP_CONNECT,
                        _ => return self.parse_error(),
                    }
                }
                OP_CONNECT => {
                    match c {
                        ' '|'\t' => self.parser_state = CONNECT_ARG,
                        _ => return self.parse_error(),
                    }
                }

                CONNECT_ARG => {
                    match c {
                        '\n' => {
                            let arg: String = self.arg_buffer.iter().collect();
                            if !arg.eq("{}") {
                                return self.parse_error();
                            }
                            return self.return_command(Connect(arg));
                        }
                        '\r' => {} // ignore
                        _ => {
                            self.arg_buffer.push(c);
                        }
                    }
                }

                OP_P => {
                    match c {
                        'I' | 'i' => self.parser_state = OP_PI,
                        'O' | 'o' => self.parser_state = OP_PO,
                        'U' | 'u' => self.parser_state = OP_PU,
                        _ => return self.parse_error(),
                    }
                }
                OP_PI => {
                    match c {
                        'N' | 'n' => self.parser_state = OP_PIN,
                        _ => return self.parse_error(),
                    }
                }
                OP_PIN => {
                    match c {
                        'G' | 'g' => self.parser_state = OP_PING,
                        _ => return self.parse_error(),
                    }
                }
                OP_PING => {
                    match c {
                        '\n' => {
                            return self.return_command(Ping);
                        }
                        '\r' => {}
                        _ => return self.parse_error(),
                    }
                }
                OP_PO => {
                    match c {
                        'N' | 'n' => self.parser_state = OP_PON,
                        _ => return self.parse_error(),
                    }
                }
                OP_PON => {
                    match c {
                        'G' | 'g' => self.parser_state = OP_PONG,
                        _ => return self.parse_error(),
                    }
                }
                OP_PONG => {
                    match c {
                        '\n' => {
                            return self.return_command(Pong);
                        }
                        '\r' => {}
                        _ => return self.parse_error(),
                    }
                }
                OP_PU => {
                    match c {
                        'B' | 'b' => self.parser_state = OP_PUB,
                        _ => return self.parse_error(),
                    }
                }
                OP_PUB => {
                    match c {
                        ' '|'\t' => self.parser_state = PUB_ARG,
                        _ => return self.parse_error(),
                    }
                }
                PUB_ARG => {
                    match c {
                        '\n' => {
                            let args = split_arg(&self.arg_buffer);

                            if args.len() != 2 {
                                return self.parse_error();
                            }

                            match parse_uint(&args[1]) {
                                Ok(size) => {
                                    self.args = args;
                                    self.msg_size = size as usize;
                                    self.parser_state = PUB_MSG;
                                }
                                Err(e) => {
                                    error!("error parsing number: {}", e);
                                    return self.parse_error();
                                }
                            }
                        }
                        '\r' => {} // ignore
                        _ => {
                            self.arg_buffer.push(c);
                        }
                    }
                }
                PUB_MSG => {
                    match c {
                        '\n' => {
                            if msg_counter != self.msg_size {
                                error!("message size mismatch");
                                return self.parse_error();
                            }

                            let arg: String = self.args[0].iter().collect();
                            return match from_utf8(&self.msg_buffer) {
                                Ok(msg) => {
                                    self.return_command(Pub{subject: arg, msg: msg.to_string()})
                                }
                                Err(e) => {
                                    error!("error parsing utf8 PUB message for subject {}", arg);
                                    self.parse_error()
                                }
                            };
                        }
                        '\r' => {} // ignore
                        _ => {
                            msg_counter += 1;
                            if msg_counter > self.msg_size {
                                error!("message size mismatch");
                                return self.parse_error();
                            }
                            self.msg_buffer.push(*b);
                        }
                    }
                }

                OP_S => {
                    match c {
                        'U' | 'u' => self.parser_state = OP_SU,
                        _ => return self.parse_error(),
                    }
                }
                OP_SU => {
                    match c {
                        'B' | 'b' => self.parser_state = OP_SUB,
                        _ => return self.parse_error(),
                    }
                }
                OP_SUB => {
                    match c {
                        ' '|'\t' => self.parser_state = SUB_ARG,
                        _ => return self.parse_error(),

                    }
                }
                SUB_ARG => {
                    match c {
                        '\n' => {
                            let args = split_arg(&self.arg_buffer);
                            if args.len() != 2 {
                                return self.parse_error();
                            }

                            return self.return_command(Sub{
                                subject: args[0].iter().collect(),
                                id: args[1].iter().collect(),
                            });
                        }
                        '\r' => {} // ignore
                        _ => {
                            self.arg_buffer.push(c);
                        }
                    }
                }
                _ => {
                    return self.parse_error();
                }
            }
        }


        Ok(Noop)
    }

    pub fn new() -> ClientRequest {
        ClientRequest {
            parser_state: ParserState::OP_START,
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


    #[test_case("PING", OP_PING; "ping")]
    #[test_case("PONG", OP_PONG; "pong")]
    #[test_case("CONNECT {}", CONNECT_ARG; "connect arg")]
    #[test_case("PUB subject", PUB_ARG; "pub arg")]
    #[test_case("PUB subject 3", PUB_ARG; "pub arg with msg len")]
    #[test_case("PUB subject 3\r\n", PUB_MSG; "pub arg with msg len before message")]
    #[test_case("PUB subject 3\r\nyes", PUB_MSG; "pub arg with msg len and message")]
    #[test_case("SUB subject", SUB_ARG; "sub arg")]
    #[test_case("SUB subject id", SUB_ARG; "sub arg with id")]
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
    fn test_parse_fail(input: &str, expected: ParseError) {
        init();
        let mut client = ClientRequest::new();
        let actual = client.parse(input.as_bytes()).unwrap_err();
        assert_eq!(expected, actual);
    }

    #[test_case("CONNECT {}\r\n", Connect("{}".to_string()); "connect command")]
    #[test_case("CONNECT\t{}\r\n", Connect("{}".to_string()); "connect with tab")]
    #[test_case("PING\r\n", Ping; "ping command")]
    #[test_case("PONG\r\n", Pong; "pong command")]
    #[test_case("SUB subject id\r\n", Sub{subject: "subject".to_string(), id: "id".to_string()}; "sub command")]
    #[test_case("SUB\tsubject\tid\r\n", Sub{subject: "subject".to_string(), id: "id".to_string()}; "sub command with tab")]
    #[test_case("PUB subject 5\r\nhello\r\n", Pub{subject: "subject".to_string(), msg: "hello".to_string()}; "pub command")]
    #[test_case("PUB\tsubject\t5\r\nhello\r\n", Pub{subject: "subject".to_string(), msg: "hello".to_string()}; "pub command with tab")]
    fn test_parse_ok(input: &str, expected: Command) {
        let mut client = ClientRequest::new();
        let actual = client.parse(input.as_bytes()).unwrap();
        assert_eq!(expected, actual);
    }


    #[test_case(vec!['s','u','p'], vec![vec!['s','u','p']]; "one arg")]
    #[test_case(vec!['s','u','p',' ',' '], vec![vec!['s','u','p']]; "one arg extra space")]
    #[test_case(vec!['s','u','p',' ','1','2','3'], vec![vec!['s','u','p'], vec!['1','2','3']]; "two args")]
    #[test_case(vec!['s','u','p','\t','1','2','3'], vec![vec!['s','u','p'], vec!['1','2','3']]; "two args with tab")]
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
}
