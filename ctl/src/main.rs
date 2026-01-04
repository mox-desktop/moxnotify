mod notify;
use clap::{Parser, Subcommand};
use std::path::Path;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE", help = "Path to the config file")]
    config: Option<Box<Path>>,
    #[command(subcommand)]
    command: NotifyCommand,
}

#[derive(Subcommand)]
enum NotifyCommand {
    #[command(about = "Change on which output the notifications will show up")]
    Output {
        #[arg(long, group = "mode", help = "Set the output to a specific value")]
        set: Option<String>,

        #[arg(
            long,
            group = "mode",
            help = "Unset so the system chooses automatically"
        )]
        unset: bool,
    },

    #[command(about = "Focus the notification viewer")]
    Focus,

    #[command(about = "Dismiss notifications")]
    Dismiss {
        #[arg(
            short,
            long,
            help = "Dismiss all notifications",
            conflicts_with = "notification"
        )]
        all: bool,

        #[arg(short, long, help = "Dismiss a specific notification by index")]
        notification: Option<u32>,
    },

    #[command(about = "List active notifications")]
    List,

    #[command(about = "List active notifications")]
    Waiting,

    #[command(about = "Mute notifications")]
    Mute {
        #[command(subcommand)]
        action: SwitchAction,
    },

    #[command(about = "Inhibit notifications")]
    Inhibit {
        #[command(subcommand)]
        action: SwitchAction,
    },
}

#[derive(Subcommand)]
enum SwitchAction {
    On,
    Off,
    Toggle,
    State,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let event = match cli.command {
        NotifyCommand::Waiting => notify::Event::Waiting,
        NotifyCommand::Focus => notify::Event::Focus,
        NotifyCommand::List => notify::Event::List,
        NotifyCommand::Dismiss { all, notification } => {
            if all {
                notify::Event::DismissAll
            } else {
                let idx = notification.unwrap_or_default();
                notify::Event::DismissOne(idx)
            }
        }
        NotifyCommand::Mute { action } => match action {
            SwitchAction::On => notify::Event::Mute,
            SwitchAction::Off => notify::Event::Unmute,
            SwitchAction::Toggle => notify::Event::ToggleMute,
            SwitchAction::State => notify::Event::MuteState,
        },
        NotifyCommand::Inhibit { action } => match action {
            SwitchAction::On => notify::Event::Inhibit,
            SwitchAction::Off => notify::Event::Uninhibit,
            SwitchAction::Toggle => notify::Event::ToggleInhibit,
            SwitchAction::State => notify::Event::InhibitState,
        },
        NotifyCommand::Output { set, unset } => {
            if let Some(output) = set {
                notify::Event::SetOutput(Some(output))
            } else if unset {
                notify::Event::SetOutput(None)
            } else {
                unreachable!()
            }
        }
    };

    notify::emit(event).await.map_err(Into::into)
}
