use clap::Parser;
use hyrcon_client::{Cli, Runtime};

#[tokio::main]
async fn main() {
  let cli = Cli::parse();
  let exit_code = Runtime::new(cli).execute().await;
  std::process::exit(exit_code);
}
