// Import the libraries
use cursive::align::HAlign;
use cursive::event::{Key, Event};
use cursive::theme::{BorderStyle, Palette, Theme, Color, PaletteColor, BaseColor};
use cursive::{traits::*, CursiveExt};
use cursive::views::{Dialog, DummyView, EditView, ScrollView, TextView, LinearLayout, Panel};
use cursive::Cursive;

use serde::{Serialize, Deserialize};
use chrono::Local;
use tokio::{
    net::TcpStream,
    sync::Mutex,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use std::{env, error::Error, sync::Arc};

// Structure for chat messages
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChatMessage {
    username: String,
    content: String,
    timestamp: String,
    message_type: MessageType,
}

// Enum for message types
#[derive(Serialize, Deserialize, Debug, Clone)]
enum MessageType {
    UserMessage,
    SystemNotification,
}

// Standard synchronous entry point
fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the Tokio runtime manually (needed because Cursive is not async)
    let rt = tokio::runtime::Runtime::new()?;

    // Run the async chat logic inside the Tokio runtime
    rt.block_on(async {
        run_chat().await
    })
}

// Async part of the program – networking + UI setup happens here
async fn run_chat() -> Result<(), Box<dyn Error>> {
    // Fetch username from command line
    let username: String = env::args()
        .nth(1)
        .expect("Please provide a username");

    // Initialize Cursive
    let mut siv = Cursive::default();
    siv.set_theme(create_retro_style());

    // Create header (username + current time)
    let header = TextView::new(format!(
        r#"╔═ RETRO CHAT ═╗ User: {} ╔═ {} ═╗"#,
        username,
        Local::now().format("%H:%M:%S")
    ))
    .style(Color::Light(BaseColor::White))
    .h_align(HAlign::Center);

    // Create the chat message area
    let messages = TextView::new("")
        .with_name("messages")
        .scrollable()
        .min_height(20);
    let messages = ScrollView::new(messages)
        .scroll_strategy(cursive::view::ScrollStrategy::StickToBottom)
        .min_width(60)
        .full_width();

    // Create input box for typing messages
    let input = EditView::new()
        .on_submit(move |s, text| send_message(s, text.to_string()))
        .with_name("input")
        .max_height(3)
        .max_width(50)
        .full_width();

    // Create help text at bottom
    let help_text = TextView::new("ESC: quit | Enter: send | Commands: /help, /clear, /quit")
        .style(Color::Dark(BaseColor::White));

    // Combine everything into one main layout
    let layout = LinearLayout::vertical()
        .child(Panel::new(header))
        .child(
            Dialog::around(messages)
                .title("Messages")
                .title_position(HAlign::Center)
                .full_width(),
        )
        .child(
            Dialog::around(input)
                .title("Message")
                .title_position(HAlign::Center)
                .full_width(),
        )
        .child(Panel::new(help_text).full_width());

    // Center layout vertically
    let centered_layout = LinearLayout::vertical()
        .child(DummyView.fixed_height(1))
        .child(layout)
        .child(DummyView.fixed_height(1));

    // Add layout to the screen
    siv.add_fullscreen_layer(centered_layout);

    // Add global key bindings
    siv.add_global_callback(Key::Esc, |s| s.quit());
    siv.add_global_callback(Event::Char('/'), |s| {
        s.call_on_name("input", |view: &mut EditView| {
            view.set_content("/");
        });
    });

    // Connect to TCP chat server
    let stream = TcpStream::connect("127.0.0.1:8082").await?;
    let (reader, mut writer) = stream.into_split();

    // Send username to the server
    writer.write_all(format!("{}\n", username).as_bytes()).await?;

    // Wrap writer in Arc<Mutex> for shared ownership
    let writer = Arc::new(Mutex::new(writer));
    let writer_clone = Arc::clone(&writer);
    siv.set_user_data(writer);

    // Prepare async reader
    let reader = BufReader::new(reader);
    let mut lines = reader.lines();

    // Clone Cursive sink for thread-safe UI updates
    let sink = siv.cb_sink().clone();

    // Spawn async task to listen for incoming messages
    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(msg) = serde_json::from_str::<ChatMessage>(&line) {
                // Format messages depending on type
                let formatted_msg = match msg.message_type {
                    MessageType::UserMessage => format!(
                        "┌─[{}]\n└─ {} ▶ {}\n",
                        msg.timestamp, msg.username, msg.content
                    ),
                    MessageType::SystemNotification => {
                        format!("\n[{} {}]\n", msg.username, msg.content)
                    }
                };

                // Send message safely to UI thread
                if sink
                    .send(Box::new(move |siv: &mut Cursive| {
                        siv.call_on_name("messages", |view: &mut TextView| {
                            view.append(formatted_msg);
                        });
                    }))
                    .is_err()
                {
                    break; // Exit loop on error
                }
            }
        }
    });

    // Start the UI event loop (blocks until user exits)
    siv.run();

    // After exiting, close the connection cleanly
    let _ = writer_clone.lock().await.shutdown().await;

    Ok(())
}

// Function to send messages typed by the user
fn send_message(siv: &mut Cursive, msg: String) {
    if msg.trim().is_empty() {
        return;
    }

    // Handle special commands
    match msg.as_str() {
        "/help" => {
            siv.call_on_name("messages", |view: &mut TextView| {
                view.append("\n=== Commands ===\n/help - Show this help\n/clear - Clear messages\n/quit - Exit chat\n\n");
            });
            siv.call_on_name("input", |view: &mut EditView| {
                view.set_content("");
            });
            return;
        }
        "/clear" => {
            siv.call_on_name("messages", |view: &mut TextView| {
                view.set_content("");
            });
            siv.call_on_name("input", |view: &mut EditView| {
                view.set_content("");
            });
            return;
        }
        "/quit" => {
            siv.quit();
            return;
        }
        _ => {}
    }

    // Send regular message to server
    let writer = siv
        .user_data::<Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>>()
        .unwrap()
        .clone();

    tokio::spawn(async move {
        let _ = writer.lock().await.write_all(format!("{}\n", msg).as_bytes()).await;
    });

    // Clear input box
    siv.call_on_name("input", |view: &mut EditView| {
        view.set_content("");
    });
}


// Create the retro style theme
fn create_retro_style() -> Theme {
    let mut theme = Theme::default();
    theme.shadow = true;
    theme.borders = BorderStyle::Simple;

     let mut palette = Palette::default();
    palette[PaletteColor::Background] = Color::Rgb(0, 0, 20); // Deep blue background
    palette[PaletteColor::View] = Color::Rgb(0, 0, 20); // Deep blue for views
    palette[PaletteColor::Primary] = Color::Rgb(0, 255, 0); // Bright green text
    palette[PaletteColor::TitlePrimary] = Color::Rgb(255, 255, 255); // Green for titles
    palette[PaletteColor::Secondary] = Color::Rgb(255, 191, 0); // Amber secondary elements
    palette[PaletteColor::Highlight] = Color::Rgb(0, 255, 255); // Cyan highlights
    palette[PaletteColor::HighlightInactive] = Color::Rgb(0, 128, 128); // Dark cyan for inactive
    palette[PaletteColor::Shadow] = Color::Rgb(0, 0, 40); // Subtle shadow
    theme.palette = palette; // Apply the palette
    theme
}
