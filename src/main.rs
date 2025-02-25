use crate::messages::{BookDepthUpdate, FullBook, Subscription};
use crate::order_book::OrderBook;
use clap::Parser;
use console_arguments::Config;
use futures_util::future::try_join_all;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

mod console_arguments;
mod messages;
mod order_book;

#[tokio::main]
async fn main() {
    println!("Binance order book scraper started!");

    // init ctrl-c hook to finish program gracefully
    let is_app_running = Arc::new(AtomicBool::new(true));
    ctrl_c_hook_init(is_app_running.clone());

    // args parsing
    let config = Config::parse();
    print!("{}", config);

    // sockets/handlers vector of futures to join at the end of the program
    let mut handlers = vec![];

    // run a bunch of symbols per socket
    for chunk_of_instruments in config
        .instruments
        .chunks(config.instruments_per_connection())
    {
        // spawn a new connection/handler, if there is a bunch of instruments to allocate
        let (read, write) = connect_to_binance(config.ws_api_url.clone()).await;

        // create handler
        let handle = tokio::spawn(handle_updates(
            is_app_running.clone(),
            chunk_of_instruments.to_vec(),
            config.levels,
            config.api_url.clone(),
            write,
            read,
        ));

        handlers.push(handle)
    }
    println!("Connections to binance opened: {}", handlers.len());

    // wait for handler/socket closure
    try_join_all(handlers)
        .await
        .expect("Failed to join all handlers");

    println!("Binance order book scraper finished!");
}

async fn handle_updates(
    is_app_running: Arc<AtomicBool>,
    symbols: Vec<String>,
    levels: u32,
    binance_api_url: String,
    mut read: SplitStream<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>>,
    mut write: SplitSink<WebSocketStream<impl AsyncRead + AsyncWrite + Unpin>, Message>,
) {
    // init books map
    let mut order_books: HashMap<String, OrderBook> = symbols
        .iter()
        .map(|symbol| (symbol.clone(), OrderBook::new(levels, symbol.clone())))
        .collect();

    // topic subscription
    for symbol in &symbols {
        // create all necessary topics to watch
        let topic_md = format!("{}@{}", symbol, "depth");
        let avg_price = format!("{}@{}", symbol, "aggTrade");
        let book_ticker = format!("{}@{}", symbol, "bookTicker");
        let text = serde_json::to_string(&Subscription {
            method: "SUBSCRIBE".to_string(),
            params: vec![topic_md, avg_price, book_ticker],
            id: format!("{}_{}", symbol, get_epoch_ms()),
        })
        .unwrap();

        // subscribe to a topic
        println!("Subscribe to topic: {text}");
        write
            .send(Message::Text(text.into()))
            .await
            .expect("Failed to send message");
    }

    // todo: consider to place it in a separate method?
    loop {
        // stop on ctrl-c
        if !is_app_running.load(Ordering::SeqCst) {
            print!("Connection closing!");
            break;
        }

        // read full books
        for symbol in &symbols {
            let url = format!(
                "{}/depth?symbol={}&limit={}",
                binance_api_url,
                symbol.to_uppercase(),
                levels
            );
            let body = reqwest::get(url.clone())
                .await
                .expect("Failed to get full book")
                .text()
                .await
                .expect("Failed to get text body");
            let book: FullBook = read_str(&body);
            order_books
                .get_mut(symbol)
                .unwrap()
                .apply_full_book_from_http_api(&book);
        }

        // incoming messages handling
        while let Some(message) = read.next().await {
            // stop on ctrl-c
            if !is_app_running.load(Ordering::SeqCst) {
                print!("Connection closing!");
                break;
            }
            match message {
                Ok(msg) => match msg {
                    Message::Ping(vec) => {
                        // send PONG (todo improve with fire and forget)
                        let fire_and_forget = write.send(Message::Pong(vec));
                        fire_and_forget.await.expect("Failed to send PING message");
                    }
                    _ => {
                        // all other messages
                        match message_type(&msg) {
                            TypeOfUpdate::AggTrade => {
                                // tbd: is it really useful?
                            }
                            TypeOfUpdate::MD => {
                                let book_update: BookDepthUpdate = read_message(&msg);
                                let book =
                                    order_books.get_mut(&book_update.s.to_lowercase()).unwrap();

                                match book.apply_depth_book_update_from_websocket(&book_update) {
                                    Ok(_) => {
                                        println!("{}", book)
                                    }
                                    Err(e) => {
                                        // eprintln!("Failed to apply depth book update");
                                        break;
                                    }
                                }
                            }
                            TypeOfUpdate::Ticker => {
                                // tbd: calculated from book
                            }
                            TypeOfUpdate::Other => {
                                // subscriptions acks
                            }
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                    break;
                }
            }
        }
    }
}

enum TypeOfUpdate {
    AggTrade,
    MD,
    Ticker,
    Other,
}

async fn connect_to_binance(
    url: String,
) -> (
    SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
) {
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect!");
    ws_stream.split()
}

fn message_type(msg: &Message) -> TypeOfUpdate {
    let text = msg.to_text().expect("Failed to parse message");
    if text.contains("id") {
        return TypeOfUpdate::Other;
    }
    if text.contains("depthUpdate") {
        return TypeOfUpdate::MD;
    }
    if text.contains("bookTicker") {
        return TypeOfUpdate::Ticker;
    }
    if text.contains("aggTrade") {
        return TypeOfUpdate::AggTrade;
    }
    TypeOfUpdate::Other
}

fn ctrl_c_hook_init(is_app_running: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        is_app_running.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
}

// utils

fn read_message<'a, T>(msg: &'a Message) -> T
where
    T: Deserialize<'a>,
{
    let text = msg.to_text().expect("Failed to parse message");
    serde_json::from_str::<'a, T>(text).expect("Cannot parse message")
}

fn read_str<'a, T>(msg: &'a String) -> T
where
    T: Deserialize<'a>,
{
    serde_json::from_str::<'a, T>(msg).expect("Cannot parse message")
}

fn get_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
