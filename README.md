# Gmail to Discord Notifier

A Rust application that monitors your Gmail inbox via IMAP and sends Discord DMs when new emails arrive.

## Features

- Real-time email monitoring with Gmail IMAP connection
- Discord DM notifications for new emails
- Automatic reconnection feature
- Secure TLS connection
- Check every 30 seconds
- Detailed logging

## Installation

### 1. Requirements

- Rust (1.70+)
- Gmail account
- Discord account and bot token

### 2. Creating Gmail App Password

1. Enable 2FA on your Gmail account
2. Google Account Settings > Security > 2-Step Verification > App passwords
3. Create a new app password for "Mail"
4. Save this password (not your regular Gmail password!)

### 3. Getting Discord Bot Token

1. Go to https://discord.com/developers/applications
2. Create a new application with "New Application"
3. Go to "Bot" tab and click "Add Bot"
4. Copy the token
5. To get your Discord User ID, enable Developer Mode and right-click on your profile

### 4. Environment Variables

Set the following environment variables:

```bash
export DISCORD_TOKEN="your_discord_bot_token"
export DISCORD_USER_ID="your_discord_user_id"
export GMAIL_EMAIL="your_gmail_address"
export GMAIL_APP_PASSWORD="your_gmail_app_password"
```

Or create a `.env` file:

```
DISCORD_TOKEN=your_discord_bot_token
DISCORD_USER_ID=your_discord_user_id
GMAIL_EMAIL=your_gmail_address
GMAIL_APP_PASSWORD=your_gmail_app_password
```

## Usage

### Build and Run

```bash
# Install dependencies and build
cargo build --release

# Run the application
cargo run
```

### Running in Background

```bash
# Run in background
nohup cargo run --release > mail-notifier.log 2>&1 &

# Follow logs
tail -f mail-notifier.log
```

## Security Notes

- Never use your regular Gmail password, only use App Password
- Keep your environment variables secure
- Don't share your Discord bot token
- Run the application on a trusted server

## Troubleshooting

### Common Errors

1. **IMAP Login Failed**: Check your Gmail App Password
2. **Discord DM Failed**: Check bot token and User ID
3. **Connection Timeout**: Check your internet connection

### Logging

The application produces detailed logs:
- `INFO`: Normal operations
- `WARN`: Warnings and reconnection attempts
- `ERROR`: Errors

## License

MIT License