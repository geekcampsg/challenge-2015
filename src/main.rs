extern crate bufstream;
extern crate rand;
extern crate time;

use std::cmp::Ordering;
use std::env;
use std::fs::File;
use std::io;
use std::io::{Write, BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use bufstream::BufStream;
use rand::Rng;

/// Attempt to write to a `TcpStream` and flush it
///
/// # Failures
///
/// Returns an Err if any errors occur during writing and flushing
fn send_line(stream: &mut BufStream<TcpStream>, message: &str) -> Result<(), io::Error> {
    try!(stream.write(format!("{}\n", message).as_bytes()));
    try!(stream.flush());
    Ok(())
}

/// Handles a single connection
///
/// # Panics
///
/// Any errors writing to or flushing the stream result in a panic.
/// Additionally, a panic also occurs if we can't get the address of our peer.
/// Lots of things can throw panics here, since it only penalises that particular connection.
fn handle_client(stream: TcpStream, words: Arc<Vec<String>>) {

    let remote = stream.peer_addr().unwrap().to_string();
    let mut stream = BufStream::new(stream);

    // We want a trivial way to identify winners
    send_line(&mut stream, "TWITTER HANDLE PLZ").unwrap();

    let mut handle = String::new();
    stream.read_line(&mut handle).unwrap();
    let handle = handle.trim();

    // Get ourselves a random word for this connection
    let idx = rand::thread_rng().gen_range(0, words.len());
    let ref my_word = words[idx];
    println!("[{} | {}] WORD {}", remote, handle, my_word);

    let begin_msg = format!("HELLO {}; BEGIN GUESSING THE WORD", handle);
    send_line(&mut stream, &begin_msg[..]).unwrap();

    let start_time = time::precise_time_s();

    let mut success = false;
    loop {

        let mut input = String::new();
        let len = stream.read_line(&mut input).unwrap();
        // Ghetto EOF detection
        if len == 0 { break; }

        input = input.trim().to_string();

        let output = match input.cmp(&my_word) {
            Ordering::Less    => "<",
            Ordering::Greater => ">",
            Ordering::Equal   => {
                success = true;
                "="
            }
        };
        send_line(&mut stream, output).unwrap();

        if success {
            break;
        } else {
            // 100ms delay between guesses
            thread::sleep_ms(100);
        }

    }

    let end_time = time::precise_time_s();
    let status: &str;
    if success {
        status = "SUCCESS";
    } else {
        status = "FAIL";
    }

    // Milliseconds FTW
    let diff = (end_time - start_time) * 1000f64;
    println!("[{} | {}] {} TIME {:.3}", remote, handle, status, diff);
}

fn main() {

    let port = match env::args().nth(1) {
        Some(port) => port,
        None => "5000".to_owned(),
    };

    let dict_file = match env::args().nth(2) {
        Some(dict_file) => dict_file,
        None => "./dictionary".to_owned(),
    };

    let listen_addr: &str = &format!("0.0.0.0:{}", port);
    let dict_file = Path::new(&dict_file[..]);

    let dict = File::open(&dict_file).unwrap();
    let dict = BufReader::new(dict);
    let mut words : Vec<String> = vec![];
    for word in dict.lines() {
        words.push(word.unwrap());
    }
    println!("[sys] Loaded {} words.", words.len());

    let shared_words = Arc::new(words);

    let listener = TcpListener::bind(listen_addr).unwrap();
    println!("[sys] Listening on {}.", listen_addr);

    for stream in listener.incoming() {

        match stream {
            Err(why) => {
                println!("[ERR] {:?}", why);
                continue;
            },
            Ok(stream) => {

                let remote = match stream.peer_addr() {
                    Err(why) => {
                        println!("[ERR] {:?}", why);
                        continue;
                    },
                    Ok(remote) => remote.to_string(),
                };
                println!("[sys] NEW {}", remote);

                let my_words = shared_words.clone();
                match thread::Builder::new().name(remote).spawn(move || {
                    handle_client(stream, my_words); 
                }) {
                    Err(why) => {
                        println!("[ERR] {:?}", why);
                        continue;
                    },
                    Ok(_) => {},
                }

            }
        }

    }

}
