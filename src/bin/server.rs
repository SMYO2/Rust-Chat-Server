use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use serde::{Serialize, Deserialize};
use chrono::Local;
use std::{error::Error, net::SocketAddr};
use tokio::net::tcp::ReadHalf;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChatMessage {
    username: String,
    content: String,
    timestamp: String,
    message_type: MessageType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum MessageType {
    UserMessage,
    SystemNotification,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Start TCP listener
    let listener = TcpListener::bind("127.0.0.1:8082").await?;

    // Server banner
    println!("╔════════════════════════════════════════╗");
    println!("║        RETRO CHAT SERVER ACTIVE        ║");
    println!("║        Port: 8080  Host: 127.0.0.1     ║");
    println!("║        Press Ctrl+C to shutdown        ║");
    println!("╚════════════════════════════════════════╝");

    // Create a broadcast channel so all connected users can receive messages
    let (tx, _rx) = broadcast::channel::<String>(100);

    loop {
        // Accept a new user connection
        let (socket, addr): (TcpStream, SocketAddr) = listener.accept().await?;

        // Displays information about connection
        println!("┌─[{}] New connection", Local::now().format("%H:%M:%S"));
        println!("└─ Address: {}", addr);

        // Clone sender to send messages to all connected users
        let tx: broadcast::Sender<String> = tx.clone();
        let rx: broadcast::Receiver<String> = tx.subscribe();

        tokio::spawn(async move{
            handle_user_connection(socket, tx, rx).await;
        });

        // tokio::spawn(async move {
        //   // Handle new user connection
        // });
    }
}

// Function to handle the user connection
async fn handle_user_connection(
    mut socket: TcpStream,
    tx: broadcast::Sender<String>,
    mut rx: broadcast::Receiver<String>,
) {
    // Splitting the socket into reader and writer
    let (reader, mut writer) = socket.split();
    let mut reader: BufReader<ReadHalf<'_>> = BufReader::new(reader);
    let mut username: String = String::new();

    // Reads the username sent by the user
    reader.read_line(&mut username).await.unwrap();
    let username = username.trim().to_string();

    // Sends a welcome message to a new connected user
    let join_msg = ChatMessage {
        username: username.clone(),
        content: "joined the chat!".to_string(),
        timestamp: Local::now().format("%H:%M:%S").to_string(),
        message_type: MessageType::SystemNotification,
    };
    let join_json: String = serde_json::to_string(&join_msg).unwrap();
    tx.send(join_json).unwrap();

    // Initialize an incoming message from an user
    let mut line = String::new();
    loop {
        tokio::select! {
            result = reader.read_line(&mut line) => {
                if result.unwrap() == 0 {
                    break;
                }
                // Create and broadcast user message
                let msg = ChatMessage {
                    username: username.clone(),
                    content: line.trim().to_string(),
                    timestamp: Local::now().format("%H:%M:%S").to_string(),
                    message_type: MessageType::UserMessage,
                };

                let json_msg = serde_json::to_string(&msg).unwrap();
                tx.send(json_msg).unwrap();
                line.clear();
            }
            // Handles incoming messages from other connected users
            result = rx.recv() => {
                let msg = result.unwrap();
                writer.write_all(msg.as_bytes()).await.unwrap();
                writer.write_all(b"\n").await.unwrap();
            }    
        }   
    }
    //Sends a notification that the user has left the chat
    let leave_msg = ChatMessage {
        username: username.clone(),
        content: "left the chat!".to_string(),
        timestamp: Local::now().format("%H:%M:%S").to_string(),
        message_type: MessageType::SystemNotification,
    };
    let leave_json: String = serde_json::to_string(&leave_msg).unwrap();
    tx.send(leave_json).unwrap();

    // log disconnection message into the terminal
    println!("└─[{}] {} disconnected", Local::now().format("%H:%M:%S"), username);
}
