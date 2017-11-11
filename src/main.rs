extern crate libc;
extern crate clap;

#[macro_use]
mod util;
mod iopoll;

use clap::{Arg, App};
use iopoll::{Token, EventLoop, EventSet, EventHandler};

use std::io::prelude::*;
use std::io::{stdin, stdout};
use std::net::{TcpStream, TcpListener, UdpSocket, SocketAddr};
use std::process::exit;
use std::time::Duration;
use std::str::FromStr;

#[derive(Default, Debug)]
struct ProgramOptions {
    ipv4_only: bool,
    ipv6_only: bool,
    use_listen: bool,
    detach_stdin: bool,
    use_unix: bool,
    use_udp: bool,
    interval_secs: u32,
    zero_io_mode: bool,
    hostname: String,
    source_port: u16,
    target_port: u16,
    verbosity: u8,
    wait_time_ms: Option<u32>,
}

enum NetworkConnection {
    TcpClient(TcpStream),
    UdpClient(UdpSocket),
}

fn parse_commandline() -> ProgramOptions {
    let matches = App::new("netcat")
        .version("0.1.0")
        .arg(Arg::with_name("ipv4").short("4").help(
            "Forces use of IPv4 addresses only.",
        ))
        .arg(Arg::with_name("ipv6").short("6").help(
            "Forces use of IPv6 addresses only.",
        ))
        .arg(Arg::with_name("udp").short("u").long("udp").help(
            "UDP mode",
        ))
        .arg(Arg::with_name("hostname").required(false))
        .arg(Arg::with_name("target-port").required(false))
        .arg(Arg::with_name("verbose").short("v").multiple(true).help(
            "Set verbosity level (can be used several times)",
        ))
        .arg(Arg::with_name("no-dns").short("n").long("nodns").help(
            "Suppress name/port resolutions",
        ))
        .arg(Arg::with_name("listen").short("l").long("listen").help(
            "Listen mode, for inbound connects",
        ))
        .arg(Arg::with_name("zero-io").short("z").help(
            "Zero-I/O mode [used for scanning]",
        ))
        .arg(
            Arg::with_name("wait-time")
                .short("w")
                .value_name("secs")
                .help("Timeout for connects and final net reads"),
        )
        .arg(Arg::with_name("detach-stdin").short("d").help(
            "Detach from stdin",
        ))
        .arg(
            Arg::with_name("source-port")
                .short("p")
                .long("source-port")
                .value_name("port"),
        )
        .get_matches();

    let mut options: ProgramOptions = Default::default();
    options.hostname = matches
        .value_of("hostname")
        .unwrap_or("localhost")
        .to_string();
    options.source_port = matches
        .value_of("source-port")
        .map_or(Ok(0), |v| u16::from_str(v))
        .unwrap_or(0);
    options.target_port = matches
        .value_of("target-port")
        .map_or(Ok(0), |v| u16::from_str(v))
        .unwrap_or(0);
    options.use_udp = matches.is_present("udp");
    options.ipv4_only = matches.is_present("ipv4");
    options.ipv6_only = matches.is_present("ipv6");
    options.use_listen = matches.is_present("listen");
    options.verbosity = matches.occurrences_of("verbose") as u8;
    if let Some(wait_time) = matches.value_of("wait-time") {
        options.wait_time_ms = Some(u32::from_str(wait_time).expect("Invalid wait time") * 1000);
    }
    options
}

pub struct NetcatClientEventHandler {
    stdin_open: bool,
    network_open: bool,
    network_client: NetworkConnection,
}

impl NetcatClientEventHandler {
    fn new(network_connection: NetworkConnection) -> NetcatClientEventHandler {
        NetcatClientEventHandler {
            stdin_open: true,
            network_open: true,
            network_client: network_connection,
        }
    }
}

impl EventHandler for NetcatClientEventHandler {
    fn ready_for_io(&mut self, event_loop: &mut EventLoop, token: Token, eventset: EventSet) {
        trace!("ready for io token={:?},eventset={}", token, eventset);

        let mut shutdown_loop = false;
        if eventset.is_readable() {
            if token == Token(0) {
                let stdin = stdin();
                if self.stdin_open {
                    let mut buf = [0; 1024];

                    trace!("start read from stdin");

                    match stdin.lock().read(&mut buf) {
                        Ok(n) => {
                            trace!("stdin: {} bytes read", n);

                            match self.network_client {
                                NetworkConnection::TcpClient(ref mut tcpstream) => {
                                    tcpstream.write_all(&buf[0..n]).unwrap();
                                }
                                NetworkConnection::UdpClient(ref mut udpsocket) => {
                                    match udpsocket.send(&buf[0..n]) {
                                        Ok(sent) => {
                                            shutdown_loop = sent != n;
                                            if shutdown_loop {
                                                eprintln!(
                                                    "Shutting down loop after udp send, number of bytes sent {} != num bytes in buf {}",
                                                    sent,
                                                    n
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            eprintln!(
                                                "Shutting down loop, error when sending to udpsocket {}",
                                                err
                                            );
                                            shutdown_loop = true;
                                        }
                                    }
                                }
                            }
                            if n == 0 {
                                self.stdin_open = false;
                                event_loop.unregister_stdin();
                                if let NetworkConnection::TcpClient(ref mut tcpstream) =
                                    self.network_client
                                {
                                    tcpstream.shutdown(std::net::Shutdown::Write).unwrap(); // TODO
                                }
                            }
                        }
                        Err(_) => {
                            shutdown_loop = true;
                        }
                    }
                }
            } else {
                let stdout = stdout();
                let mut buf = [0; 1024];
                if let Ok(n) = match self.network_client {
                    NetworkConnection::TcpClient(ref mut tcpstream) => {
                        trace!("Start read from tcpstream");

                        tcpstream.read(&mut buf)
                    }
                    NetworkConnection::UdpClient(ref mut udpsocket) => {
                        trace!("Start recv from UDP socket");

                        udpsocket.recv(&mut buf)
                    }
                }
                {
                    trace!("Read/recv done, writing to stdout");

                    stdout.lock().write_all(&buf[0..n]).unwrap();

                    trace!("Write to stdout done");

                    if n == 0 {
                        shutdown_loop = true;
                        self.network_open = false;
                    }
                }
            }
        }

        trace!("end ready for io");

        if shutdown_loop {
            trace!("Event loop is shut down");

            event_loop.shutdown();
        }
    }

    fn timeout(&mut self, eventloop: &mut EventLoop) {
        trace!("timeout");

        eventloop.shutdown();
    }

    fn hangup(&mut self, eventloop: &mut EventLoop, connection_id: Token) {
        trace!("hangup token={:?}", connection_id);

        if connection_id == Token(0) {
            eventloop.shutdown();
        }
    }
}

fn main() {
    let options = parse_commandline();
    if options.verbosity > 1 {
        eprintln!("options: {:?}", options);
    }

    let mut exit_code = 0;

    let stdin = stdin();
    let connection;
    let mut eventloop = EventLoop::new(options.wait_time_ms);
    if !options.detach_stdin {
        eventloop.register_stdin(&stdin);
    }

    if options.use_listen {
        let tcplistener = TcpListener::bind("0.0.0.0").unwrap();
        eventloop.register_read(&tcplistener);
    } else {
        if options.use_udp {
            let sock = UdpSocket::bind(("0.0.0.0", options.source_port)).unwrap();
            sock.connect((options.hostname.as_str(), options.target_port))
                .unwrap();

            trace!("{:?}", sock);

            if let Some(timeout) = options.wait_time_ms {
                sock.set_read_timeout(Some(Duration::new(u64::from(timeout), 0)))
                    .unwrap(); // TODO
            }
            eventloop.register_read(&sock);
            connection = NetworkConnection::UdpClient(sock);
        } else {
            let tcpstream = TcpStream::connect((options.hostname.as_str(), options.target_port))
                .unwrap();
            if let Some(timeout) = options.wait_time_ms {
                tcpstream
                    .set_read_timeout(Some(Duration::new(u64::from(timeout), 0)))
                    .unwrap(); // TODO
            }
            eventloop.register_read(&tcpstream);
            connection = NetworkConnection::TcpClient(tcpstream);
        }

        let mut eh = NetcatClientEventHandler::new(connection);
        if let Err(err) = eventloop.run(&mut eh) {
            exit_code = 1;
            if options.verbosity > 0 {
                eprintln!("{}", err);
            }
        }
    }
    exit(exit_code);
}
