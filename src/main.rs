extern crate argparse;
extern crate mio;

use argparse::{ArgumentParser, Store, StoreTrue, StoreFalse};

use std::io::prelude::*;
use std::io::{stdin, stdout};
use std::os::unix::io::FromRawFd;
use std::net::{ToSocketAddrs, SocketAddr}; 
use std::process::exit;

use mio::{EventLoop, Token, EventSet, Handler, PollOpt};
use mio::unix::PipeReader;
use mio::tcp::{TcpListener, TcpStream}; // TODO implement listener

const SERVER: Token = Token(0); // TODO implement listener
const CLIENT: Token = Token(1);
const STDIN: Token = Token(2);

struct StdinToSockHandler {
    tcpstream : TcpStream,
    error_occurred : bool,
}

macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);

fn communicate(stream: TcpStream, use_stdin: bool) -> Result<(), String> {
    let mut event_loop = EventLoop::new().unwrap(); // TODO handle result
    let stdinreader = unsafe { PipeReader::from_raw_fd(0) };
    if use_stdin {
        event_loop.register(&stdinreader, STDIN, EventSet::readable(), PollOpt::edge())
            .unwrap(); // TODO handle result
    }
    event_loop.register(&stream, CLIENT, EventSet::readable(), PollOpt::edge()).unwrap(); // TODO handle result
    event_loop.run(&mut StdinToSockHandler::from_tcpstream(stream)).unwrap(); // TODO handle result
    Ok(())
}

impl StdinToSockHandler {
    fn from_tcpstream(tcpstream: TcpStream) -> StdinToSockHandler {
        StdinToSockHandler {
            tcpstream : tcpstream,
            error_occurred : false,
        }
    }
}


impl Handler for StdinToSockHandler {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self,
             event_loop: &mut EventLoop<StdinToSockHandler>,
             token: Token,
             events: EventSet) {
        if events.is_hup() {
            if token == CLIENT {
                self.error_occurred = true;
            }
            event_loop.shutdown();
            return;
        }

        match token {
            STDIN => {
                // read the bytes from stdin
                // and
                // write to socket
                let mut buf = [0; 8192];
                if let Ok(n) = stdin().read(&mut buf) {
                    if n == 0 {
                        event_loop.shutdown();
                    } else {
                        self.tcpstream.write(&buf[0..n]); // TODO handle result
                    }
                }
            }
            CLIENT => {
                let mut buf = [0; 8192];
                if let Ok(n) = self.tcpstream.read(&mut buf) {
                    if n == 0 {
                        event_loop.shutdown();
                    } else {
                        stdout().write(&buf[0..n]); // TODO handle result
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let mut use_ipv4 = true;
    let mut use_ipv6 = false;
    let mut use_listen = false;
    let mut use_stdin = true;
    let mut use_unix = false;
    let mut use_udp = false;
    let mut interval_secs = 0;
    let mut hostname = String::new();
    let mut port_spec = String::new();
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut use_ipv4)
            .add_option(&["-4"], StoreTrue, "Use IPv4");

        ap.refer(&mut use_ipv6)
            .add_option(&["-6"], StoreTrue, "Use IPv6");

        ap.refer(&mut use_stdin)
            .add_option(&["-d"], StoreFalse, "Detach from stdin");

        ap.refer(&mut interval_secs)
            .add_option(&["-i", "--interval"],
                        Store,
                        "Delay interval [seconds] for lines sent, ports scanned");

        ap.refer(&mut use_listen)
            .add_option(&["-l", "--listen"],
                        StoreTrue,
                        "Listen-mode, for inbound connects");
        ap.refer(&mut use_udp)
            .add_option(&["-u", "--udp"], StoreTrue, "UDP mode");

        ap.refer(&mut use_unix)
            .add_option(&["-U", "--unix"], StoreTrue, "Use UNIX domain socket");

        ap.refer(&mut hostname)
            .add_argument("hostname", Store, "Hostname");

        ap.refer(&mut port_spec)
            .add_argument("port(s)",
                          Store,
                          "Port (e.g. 20) or range of ports (eg 10-20)");

        ap.parse_args_or_exit();
    }

    if hostname != "" && port_spec == "" {
        port_spec = hostname.clone();
        hostname = "localhost".to_string();
    }

    println_stderr!("read from stdin : {}", use_stdin);
    println_stderr!("listen on socket: {}", use_listen);
    println_stderr!("use ipv4        : {}", use_ipv4);
    println_stderr!("use ipv6        : {}", use_ipv6);
    println_stderr!("use UDP         : {}", use_udp);
    println_stderr!("use UNIX domain : {}", use_unix);
    println_stderr!("hostname        : {}", hostname);
    println_stderr!("port(s)         : {}", port_spec);

    let addr = format!("{}:{}", hostname, port_spec);
    let mut exit_code = 0;

    if use_listen {

    } else {
        let sockaddr = (&addr)
            .to_socket_addrs()
            .unwrap() // TODO handle result
            .filter(|&a| {
                match a {
                    SocketAddr::V4(_) => use_ipv4,
                    SocketAddr::V6(_) => use_ipv6,
                }
            })
            .nth(0)
            .unwrap(); // TODO handle result
        println_stderr!("{:?}", &sockaddr);
        if let Ok(stream) = TcpStream::connect(&sockaddr) {
            communicate(stream, use_stdin); // TODO handle result
        } else {
            println_stderr!("Cannot connect to {}", addr);
            exit_code = 1;
        }
    }
    exit(exit_code);
}
