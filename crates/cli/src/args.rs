use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportDataFormat {
    Json,
    Csv,
}

#[derive(Debug, Parser)]
#[command(
    name = "mfa-forge",
    about = "Secure developer-first MFA token manager",
    version
)]
pub struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init,
    Agent,
    Mcp,
    Add {
        #[arg(long)]
        service: String,
        #[arg(long)]
        user: String,
        #[arg(long)]
        secret: Option<String>,
        #[arg(long, value_delimiter = ',')]
        labels: Vec<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, value_name = "PROJECT_PATH")]
        project_path: Option<String>,
        #[arg(long)]
        source: Option<String>,
    },
    Import {
        #[arg(long, value_name = "OTPAUTH_URI")]
        uri: Option<String>,
        #[arg(long, value_delimiter = ',')]
        labels: Vec<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, value_name = "PROJECT_PATH")]
        project_path: Option<String>,
        #[arg(long)]
        source: Option<String>,
    },
    ImportCsv {
        #[arg(long, value_name = "CSV_PATH")]
        path: String,
    },
    ImportBitwardenCsv {
        #[arg(long, value_name = "CSV_PATH")]
        path: String,
        #[arg(long, value_name = "ROWS")]
        rows: Option<String>,
        #[arg(long)]
        preview: bool,
    },
    List,
    History,
    Restore {
        #[arg(long, value_name = "ENTRY_ID")]
        entry_id: String,
    },
    Token {
        service: String,
        #[arg(long)]
        user: Option<String>,
    },
    Remove {
        service: String,
        #[arg(long)]
        user: Option<String>,
    },
    RotatePassword,
    Export {
        #[arg(long, value_enum, default_value_t = ExportDataFormat::Json)]
        data_format: ExportDataFormat,
        #[arg(long, value_name = "OUTPUT_PATH")]
        path: Option<String>,
    },
}
