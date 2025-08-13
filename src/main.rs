use anyhow::{Context, Result};
use dotenv::dotenv;
use imap::Session;
use native_tls::TlsStream;
use serenity::all::{CreateMessage, Http, UserId};
use std::collections::HashSet;
use std::env;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;
use tracing::{error, info, warn};

type ImapSession = Session<TlsStream<TcpStream>>;

struct MailNotifier {
    discord_http: Http,
    user_id: UserId,
    imap_session: ImapSession,
    seen_uids: HashSet<u32>,
}

impl MailNotifier {
    async fn new() -> Result<Self> {
        let discord_token =
            env::var("DISCORD_TOKEN").context("DISCORD_TOKEN environment variable not set")?;
        let user_id: u64 = env::var("DISCORD_USER_ID")
            .context("DISCORD_USER_ID environment variable not set")?
            .parse()
            .context("Invalid DISCORD_USER_ID format")?;
        let gmail_email =
            env::var("GMAIL_EMAIL").context("GMAIL_EMAIL environment variable not set")?;
        let gmail_password = env::var("GMAIL_APP_PASSWORD")
            .context("GMAIL_APP_PASSWORD environment variable not set")?;

        let discord_http = Http::new(&discord_token);
        let user_id = UserId::new(user_id);

        let domain = "imap.gmail.com";
        let port = 993;
        let socket =
            TcpStream::connect((domain, port)).context("Failed to connect to Gmail IMAP server")?;

        let tls = native_tls::TlsConnector::builder()
            .build()
            .context("Failed to create TLS connector")?;
        let tls_stream = tls
            .connect(domain, socket)
            .context("Failed to establish TLS connection")?;

        let client = imap::Client::new(tls_stream);
        let mut imap_session = client
            .login(&gmail_email, &gmail_password)
            .map_err(|e| anyhow::anyhow!("IMAP login failed: {:?}", e.0))?;

        imap_session
            .select("INBOX")
            .context("Failed to select INBOX")?;

        let mut seen_uids = HashSet::new();

        let messages = imap_session
            .search("ALL")
            .context("Failed to search existing messages")?;
        for uid in messages {
            seen_uids.insert(uid);
        }

        info!("Initialized with {} existing messages", seen_uids.len());

        Ok(Self {
            discord_http,
            user_id,
            imap_session,
            seen_uids,
        })
    }

    async fn check_new_emails(&mut self) -> Result<()> {
        self.imap_session
            .noop()
            .context("Failed to send NOOP command")?;

        let messages = self
            .imap_session
            .search("UNSEEN")
            .context("Failed to search for unseen messages")?;

        for uid in messages {
            if !self.seen_uids.contains(&uid) {
                self.seen_uids.insert(uid);

                if let Err(e) = self.process_new_email(uid).await {
                    error!("Failed to process email {}: {}", uid, e);
                }
            }
        }

        Ok(())
    }

    async fn process_new_email(&mut self, uid: u32) -> Result<()> {
        let messages = self
            .imap_session
            .fetch(format!("{uid}"), "(ENVELOPE BODY[HEADER.FIELDS (DATE)])")
            .context("Failed to fetch email")?;

        if let Some(message) = messages.iter().next() {
            let envelope = message.envelope().context("Failed to get email envelope")?;

            let from = envelope
                .from
                .as_ref()
                .and_then(|addrs| addrs.first())
                .map(|addr| {
                    let name = addr
                        .name
                        .as_ref()
                        .map(|n| std::str::from_utf8(n).unwrap_or("Unknown"))
                        .unwrap_or("Unknown");
                    let email = addr
                        .mailbox
                        .as_ref()
                        .map(|m| std::str::from_utf8(m).unwrap_or("unknown"))
                        .unwrap_or("unknown");
                    let host = addr
                        .host
                        .as_ref()
                        .map(|h| std::str::from_utf8(h).unwrap_or("unknown"))
                        .unwrap_or("unknown");
                    format!("{name} <{email}@{host}>")
                })
                .unwrap_or_else(|| "Unknown Sender".to_string());

            let subject = envelope
                .subject
                .as_ref()
                .map(|s| std::str::from_utf8(s).unwrap_or("No Subject"))
                .unwrap_or("No Subject");

            let date = envelope
                .date
                .as_ref()
                .map(|d| std::str::from_utf8(d).unwrap_or("Unknown Date"))
                .unwrap_or("Unknown Date");

            let notification = format!(
                "📧 **New Email Received!**\n\n\
                **From:** {from}\n\
                **Subject:** {subject}\n\
                **Date:** {date}\n\
                **UID:** {uid}"
            );

            info!("New email from: {} - Subject: {}", from, subject);

            self.send_discord_dm(&notification)
                .await
                .context("Failed to send Discord DM")?;
        }

        Ok(())
    }

    async fn send_discord_dm(&self, message: &str) -> Result<()> {
        let dm_channel = self
            .user_id
            .create_dm_channel(&self.discord_http)
            .await
            .context("Failed to create DM channel")?;

        let builder = CreateMessage::new().content(message);

        dm_channel
            .send_message(&self.discord_http, builder)
            .await
            .context("Failed to send Discord message")?;

        info!("Discord DM sent successfully");
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        info!("Mail notifier started. Checking for new emails every 30 seconds...");

        let mut interval = time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            if let Err(e) = self.check_new_emails().await {
                error!("Error checking emails: {}", e);

                if e.to_string().contains("connection") || e.to_string().contains("timeout") {
                    warn!("Connection issue detected, attempting to reconnect...");
                    if let Err(reconnect_err) = self.reconnect().await {
                        error!("Failed to reconnect: {}", reconnect_err);
                        time::sleep(Duration::from_secs(60)).await;
                    }
                }
            }
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        let gmail_email = env::var("GMAIL_EMAIL")?;
        let gmail_password = env::var("GMAIL_APP_PASSWORD")?;

        let domain = "imap.gmail.com";
        let port = 993;
        let socket = TcpStream::connect((domain, port))?;

        let tls = native_tls::TlsConnector::builder().build()?;
        let tls_stream = tls.connect(domain, socket)?;

        let client = imap::Client::new(tls_stream);
        self.imap_session = client
            .login(&gmail_email, &gmail_password)
            .map_err(|e| anyhow::anyhow!("IMAP reconnection failed: {:?}", e.0))?;

        self.imap_session.select("INBOX")?;

        info!("Successfully reconnected to IMAP server");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let exe_path = env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let project_root = exe_dir.parent().unwrap().parent().unwrap();
    let env_path = project_root.join(".env");
    
    if env_path.exists() {
        dotenv::from_path(&env_path).ok();
    } else {
        dotenv().ok();
    }
    
    tracing_subscriber::fmt::init();

    info!("Starting Gmail to Discord notifier...");

    let required_vars = [
        "DISCORD_TOKEN",
        "DISCORD_USER_ID",
        "GMAIL_EMAIL",
        "GMAIL_APP_PASSWORD",
    ];
    for var in &required_vars {
        if env::var(var).is_err() {
            error!("Missing required environment variable: {}", var);
            eprintln!("\nRequired environment variables:");
            eprintln!("DISCORD_TOKEN=your_discord_bot_token");
            eprintln!("DISCORD_USER_ID=your_discord_user_id");
            eprintln!("GMAIL_EMAIL=your_gmail_address");
            eprintln!("GMAIL_APP_PASSWORD=your_gmail_app_password");
            eprintln!("\nNote: Use Gmail App Password, not your regular password!");
            std::process::exit(1);
        }
    }

    let mut notifier = MailNotifier::new()
        .await
        .context("Failed to initialize mail notifier")?;

    notifier
        .run()
        .await
        .context("Mail notifier encountered an error")?;

    Ok(())
}
