use std::{io::{ErrorKind, Read, Write}, net::{SocketAddr, SocketAddrV4, TcpListener, TcpStream}, os::fd::{AsFd, AsRawFd}, time::{Duration, Instant}};
use clap::Parser;
use nix::{errno::Errno, poll::{poll, PollFd, PollFlags, PollTimeout}, sys::socket::{connect, socket, AddressFamily, SockFlag, SockType, SockaddrIn}};

#[derive(Parser)]
struct Args {
    /// If specified, enables client mode and connects to the specified server endpoint
    #[clap(short)]
    client: Option<SocketAddrV4>,
    /// Enable server mode
    #[clap(short, default_value_t = false)]
    server: bool,
    /// For server mode, request a specific binding
    #[clap(short, default_value = "0.0.0.0:12345")]
    bind: SocketAddrV4,
    /// Number of iterations of the test to run
    #[clap(short, default_value_t = 100_000)]
    num: u32,
    /// Warmup iterations
    #[clap(short, default_value_t = 1_000)]
    warmup_num: u32,
    /// Open a new TCP connection for every iteration (default: reuse one long-lived connection)
    #[clap(long)]
    per_conn: bool
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.server == args.client.is_some() {
        anyhow::bail!("Must select either client or server");
    }
    if args.num == 0 {
        anyhow::bail!("Must use num > 0");
    }

    if args.server {
        if args.per_conn {
            run_server(args.bind.into(), args.num, args.warmup_num)?;
        } else {
            run_server_no_conn(args.bind.into(), args.num, args.warmup_num)?;
        }
    } else {
        let Some(connect_addr) = args.client else {
            anyhow::bail!("Must use a vaild server endpoint for connection establishment");
        };

        if args.per_conn {
            run_client(connect_addr, args.num, args.warmup_num)?;
        } else {
            run_client_no_conn(connect_addr, args.num, args.warmup_num)?;
        }
    }

    Ok(())
}

fn run_server(bind: SocketAddr, num: u32, warmup_num: u32) -> anyhow::Result<()> {
    let mut buf = [0u8];
    let listener = TcpListener::bind(bind)?;
    listener.set_nonblocking(true)?;

    for _ in 0..(num + warmup_num) {
        let mut stream = loop {
            // println!("Looping accept");
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        };
        stream.set_nonblocking(true)?;
        // println!("Accept DONE");

        loop {
            // println!("Looping read");
            match stream.read(&mut buf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }
        // println!("Read DONE");
        loop {
            // println!("Looping write");
            match stream.write(&buf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }
        // println!("Write DONE");
    }

    Ok(())
}

fn run_server_no_conn(bind: SocketAddr, num: u32, warmup_num: u32) -> anyhow::Result<()> {
    let mut buf = [0u8];
    let listener = TcpListener::bind(bind)?;
    listener.set_nonblocking(true)?;
    let mut stream = loop {
        // println!("Looping accept");
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => continue,
                _ => return Err(e.into())
            }
        }
    };
    stream.set_nonblocking(true)?;

    for _ in 0..(num + warmup_num) {
        loop {
            // println!("Looping read");
            match stream.read(&mut buf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }
        loop {
            // println!("Looping write");
            match stream.write(&buf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }
    }

    Ok(())
}

/// TODO: TCP Fast Open implementation
fn run_server_tfo(bind: SocketAddr, num: u32, warmup_num: u32) -> anyhow::Result<()> {
    let mut buf = [0u8];

    // TODO

    Ok(())
}

fn run_client(connect_addr: SocketAddrV4, num: u32, warmup_num: u32) -> anyhow::Result<()> {
    let mut samples = Vec::with_capacity(num as _);
    let sndbuf = [42u8];
    let mut rcvbuf = [0u8];
    let sockaddr = SockaddrIn::from(connect_addr);

    for it in 0..(num + warmup_num) {
        let socket_fd = socket(
            AddressFamily::Inet,
            SockType::Stream,
            SockFlag::SOCK_NONBLOCK,
            None
        )?;

        let prev = Instant::now();

        // println!("About to call connect");
        match connect(socket_fd.as_raw_fd(), &sockaddr) {
            Ok(()) => { /* Nothing to do */ },
            Err(Errno::EINPROGRESS) => {
                let mut pollfd = [PollFd::new(socket_fd.as_fd(), PollFlags::POLLOUT)];
                loop {
                    // println!("Looping connect");
                    match poll(&mut pollfd, PollTimeout::ZERO) {
                        Ok(1) => if pollfd[0].revents().unwrap().contains(PollFlags::POLLOUT) {
                            break
                        } else {
                            continue
                        },
                        Ok(_) => continue,
                        Err(e) => return Err(e.into())
                    }
                }
            },
            Err(e) => return Err(e.into())
        }
        // println!("Connect DONE");

        let mut stream = TcpStream::from(socket_fd);
        loop {
            // println!("Looping write");
            match stream.write(&sndbuf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }
        // println!("Write DONE");
        loop {
            // println!("Looping read");
            match stream.read(&mut rcvbuf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }
        // println!("Read DONE");

        let ping_duration = prev.elapsed();
        if it >= warmup_num {
            samples.push(ping_duration);
        }

        assert_eq!(sndbuf, rcvbuf);
    }

    produce_output(samples);

    Ok(())
}

fn run_client_no_conn(connect_addr: SocketAddrV4, num: u32, warmup_num: u32) -> anyhow::Result<()> {
    let mut samples = Vec::with_capacity(num as _);
    let sndbuf = [42u8];
    let mut rcvbuf = [0u8];
    let sockaddr = SockaddrIn::from(connect_addr);

    let socket_fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::SOCK_NONBLOCK,
        None
    )?;

    match connect(socket_fd.as_raw_fd(), &sockaddr) {
        Ok(()) => { /* Nothing to do */ },
        Err(Errno::EINPROGRESS) => {
            let mut pollfd = [PollFd::new(socket_fd.as_fd(), PollFlags::POLLOUT)];
            loop {
                // println!("Looping connect");
                match poll(&mut pollfd, PollTimeout::ZERO) {
                    Ok(1) => if pollfd[0].revents().unwrap().contains(PollFlags::POLLOUT) {
                        break
                    } else {
                        continue
                    },
                    Ok(_) => continue,
                    Err(e) => return Err(e.into())
                }
            }
        },
        Err(e) => return Err(e.into())
    }

    let mut stream = TcpStream::from(socket_fd);

    for it in 0..(num + warmup_num) {
        let prev = Instant::now();
        loop {
            // println!("Looping write");
            match stream.write(&sndbuf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }
        loop {
            // println!("Looping read");
            match stream.read(&mut rcvbuf) {
                Ok(0) => continue,
                Ok(_) => break,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => continue,
                    _ => return Err(e.into())
                }
            }
        }

        let ping_duration = prev.elapsed();
        if it >= warmup_num {
            samples.push(ping_duration);
        }

        assert_eq!(sndbuf, rcvbuf);
    }

    produce_output(samples);

    Ok(())
}

fn produce_output(samples: Vec<Duration>) {
    for s in samples {
        println!("{}", s.as_nanos());
    }
}
