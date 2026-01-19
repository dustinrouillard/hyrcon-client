use clap::Parser;
use hyrcon_client::{Cli, Runtime};
use std::{
  env,
  ffi::{OsStr, OsString},
};

const PRIMARY_PREFIX: &str = "RCON_";
const ALIAS_PREFIX: &str = "HYRCON_";

fn set_env_var(key: &str, value: &OsStr) {
  // SAFETY: the key and value originate from the process environment and therefore satisfy the platform-specific requirements for environment variables.
  unsafe {
    env::set_var(key, value);
  }
}

fn mirror_env_aliases() {
  let snapshot: Vec<(OsString, OsString)> = env::vars_os().collect();

  for (key, value) in snapshot {
    let Some(key_str) = key.to_str() else {
      continue;
    };

    if let Some(suffix) = key_str.strip_prefix(PRIMARY_PREFIX) {
      let alias_key = format!("{ALIAS_PREFIX}{suffix}");
      if env::var_os(&alias_key).is_none() {
        set_env_var(&alias_key, value.as_os_str());
      }
    } else if let Some(suffix) = key_str.strip_prefix(ALIAS_PREFIX) {
      let primary_key = format!("{PRIMARY_PREFIX}{suffix}");
      if env::var_os(&primary_key).is_none() {
        set_env_var(&primary_key, value.as_os_str());
      }
    }
  }
}

#[tokio::main]
async fn main() {
  mirror_env_aliases();

  let cli = Cli::parse();
  let exit_code = Runtime::new(cli).execute().await;
  std::process::exit(exit_code);
}
