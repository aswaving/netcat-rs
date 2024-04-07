#[macro_use]
mod util;
mod iopoll;

use crate::iopoll::{EventHandler, EventLoop, EventSet, Token};
use clap::Parser;
use std::io::prelude::*;
use std::io::{stdin, stdout};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::process::exit;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version)]
struct ProgramOptions {
    #[arg(short='4', help="Use IPv4 only")]
    ipv4_only: bool,
    #[arg(short='6', help="Use IPv6 only")]
    ipv6_only: bool,
    #[arg(short='l', long="listen", help="Listen on specified port")]
    use_listen: bool,
    #[arg(short, help="Detach stdin")]
    detach_stdin: bool,
    #[arg(short='U', help="Use UNIX domain sockets")]
    use_unix: bool,
    #[arg(short, help="Use UDP")]
    use_udp: bool,
    #[arg(short, help="Don't lookup address using DNS")]
    no_dns: bool,
    #[arg(short, help="Delay (in seconds) between lines of text sent and received.")]
    interval_secs: Option<i32>,
    hostname: String,
    #[arg(short='p')]
    source_port: Option<u16>,
    target_port: u16,
    #[arg(short, long="verbose", action=clap::ArgAction::Count)]
    verbosity: u8,
    #[arg(short='w', help="If a connection or stdin is idle for more than TIMEOUT seconds, the connection is silently closed. The default is no timeout.")]
    timeout: Option<u32>,
}

enum NetworkConnection {
    TcpClient(TcpStream),
    UdpClient(UdpSocket),
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
    fn ready_for_io(
        &mut self,
        event_loop: &mut EventLoop,
        token: Token,
        eventset: EventSet,
    ) -> std::io::Result<()> {
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
                                    tcpstream.write_all(&buf[0..n])?;
                                }
                                NetworkConnection::UdpClient(ref mut udpsocket) => {
                                    match udpsocket.send(&buf[0..n]) {
                                        Ok(sent) => {
                                            if sent != n {
                                                eprintln!(
                                                    "Shutting down loop after udp send, number of bytes sent {sent} != num bytes in buf {n}");
                                                shutdown_loop = true;
                                            }
                                        }
                                        Err(err) => {
                                            eprintln!(
                                                "Shutting down loop, error when sending to udpsocket {err}" 
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
                                    tcpstream.shutdown(std::net::Shutdown::Write)?;
                                    // TODO
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
                } {
                    trace!("Read/recv done, writing to stdout");

                    stdout.lock().write_all(&buf[0..n])?;

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
        Ok(())
    }

    fn error(&mut self, _eventloop: &mut EventLoop, _stream_id: Token) {}

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

fn main() -> std::io::Result<()> {
    let options = ProgramOptions::parse();
    let mut exit_code = 0;
    let stdin = stdin();
    let connection;
    let mut eventloop = EventLoop::new_with_timeout(options.timeout);
    if !options.detach_stdin {
        eventloop.register_stdin(&stdin);
    }
    if options.use_unix {
        println!("UNIX domain sockets are currently unsupported");
        exit(1);
    }

    if options.use_listen {
        // TODO
        let tcplistener = if options.ipv4_only {
            TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 1234))?
        } else {
            TcpListener::bind(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 1234))?
        };
        eventloop.register_read(&tcplistener);
    } else {
        if options.use_udp {
            let target_addr: IpAddr = options.hostname.parse().expect("Invalid hostname");
            match target_addr {
                IpAddr::V4(_) => {
                    if options.ipv6_only {
                        println!("IPV6 only specified and {target_addr:?} is not an IPv6Addr.");
                        exit(1);
                    }
                }
                IpAddr::V6(_) => {
                    if options.ipv4_only {
                        println!("IPV4 only specified and {target_addr:?} is not an I4v6Addr.");
                        exit(1);
                    }
                }
            }
            let target_sock: SocketAddr = if target_addr.is_ipv6() {
                format!("[{}]:{}", options.hostname.as_str(), options.target_port)
                    .parse()
                    .expect("Invalid target")
            } else {
                format!("{}:{}", options.hostname.as_str(), options.target_port)
                    .parse()
                    .expect("Invalid target")
            };

            trace!("target={:?}", target_sock);

            let bind_addr = if target_addr.is_ipv4() {
                IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            } else {
                IpAddr::V6(Ipv6Addr::UNSPECIFIED)
            };
            let sock = UdpSocket::bind(SocketAddr::new(bind_addr, options.source_port.unwrap_or(0)))?;
            sock.connect(target_sock)?;

            trace!("localsock={:?}", sock);

            if let Some(timeout) = options.timeout {
                sock.set_read_timeout(Some(Duration::new(u64::from(timeout), 0)))?;
            }
            eventloop.register_read(&sock);
            connection = NetworkConnection::UdpClient(sock);
        } else {
            let tcpstream = TcpStream::connect((options.hostname.as_str(), options.target_port))?;
            if let Some(timeout) = options.timeout {
                tcpstream.set_read_timeout(Some(Duration::new(u64::from(timeout), 0)))?;
            }
            eventloop.register_read(&tcpstream);
            connection = NetworkConnection::TcpClient(tcpstream);
        }

        let mut eh = NetcatClientEventHandler::new(connection);
        if let Err(err) = eventloop.run(&mut eh) {
            exit_code = 1;
            if options.verbosity > 0  {
                eprintln!("{err}");
            }
        }
    }
    exit(exit_code);
}
