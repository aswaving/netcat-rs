extern crate argparse;
extern crate crossbeam;

use argparse::{ArgumentParser, Store, StoreTrue, StoreFalse};

use std::io::prelude::*;
use std::io::{stdin, stdout, BufReader};
use std::net::{TcpListener, TcpStream};

use std::sync::mpsc::{channel};

fn main() {
    //let mut use_ipv4 = true; TODO
    //let mut use_ipv6 = false; // TODO
    let mut use_listen = false;
    let mut use_stdin = true;
    //let mut use_unix = false; // TODO
    //let mut use_udp = false; // TODO
    // let mut interval_secs = 0; // TODO
    let mut hostname = "localhost".to_string();
    let mut port_spec = String::new();

    {
        let mut ap = ArgumentParser::new();
    /*    ap.refer(&mut use_ipv4)
            .add_option(&["-4"], StoreTrue, "Use IPv4");

        ap.refer(&mut use_ipv6)
            .add_option(&["-6"], StoreTrue, "Use IPv6");
            */

        ap.refer(&mut use_stdin)
            .add_option(&["-d"], StoreFalse, "Detach from stdin");

        /* ap.refer(&mut interval_secs)
            .add_option(&["-i", "--interval"], Store, "Delay interval [seconds] for lines sent, ports scanned");
            */

        ap.refer(&mut use_listen)
            .add_option(&["-l", "--listen"], StoreTrue, "Listen-mode, for inbound connects"); 
        /*ap.refer(&mut use_udp)
            .add_option(&["-u", "--udp"], StoreTrue, "UDP mode");

        ap.refer(&mut use_unix)
            .add_option(&["-U", "--unix"], StoreTrue, "Use UNIX domain socket");
            */

        ap.refer(&mut hostname)
            .add_argument("hostname", Store, "Hostname");

        ap.refer(&mut port_spec)
            .add_argument("port(s)", Store, "Port");

        ap.parse_args_or_exit();
    }
    if hostname != "" && port_spec == "" {
        port_spec = hostname.clone();
        hostname = "localhost".to_string();
    }

    let addr = format!("{}:{}", hostname, port_spec);


    if use_listen {
        let listener = TcpListener::bind(format!("{}:{}", hostname, port_spec).as_str()).unwrap();
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let mut rdr = BufReader::new(&stream);
                let mut wrt_stream = stream.try_clone().unwrap();
                let (tx, rx) = channel();

                crossbeam::scope(|scope| {
                    scope.spawn( move || {
                        let mut buf = [0;8192];
                        while let Ok(n) = rdr.read(&mut buf) {
                            if n == 0 {
                                tx.send(false).unwrap();
                                break;
                            }
                            stdout().write(&buf[0..n]).unwrap();
                        }
                    });
                    if use_stdin {
                    scope.spawn( move || {
                        let mut buf = [0;8192];
                        let mut socket_open = true;
                        while socket_open {
                            // TODO: to get this right asynch I/O is needed
                            // cannot cancel synchronous IO
                            // cannot cancel thread
                            // this thread stays open until read call has returned
                            // and client connection has closed
                            while let Ok(n) = stdin().read(&mut buf) {
                                socket_open = rx.recv().unwrap();
                                if n == 0 || !socket_open {
                                    break;
                                }
                                wrt_stream.write(&buf[0..n]).unwrap();
                            }
                        }
                    });
                    }
                });
            }
        }

    } else {
        let mut stream = TcpStream::connect(addr.as_str()).unwrap();
        let mut rdr = BufReader::new(stdin());
        let mut buf = [0;8192];
        while let Ok(n) = rdr.read(&mut buf) {
            if n == 0 { break; }
            stream.write(&buf[0..n as usize]).unwrap();
        }
    }
}
