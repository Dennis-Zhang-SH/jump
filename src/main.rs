#[deny(warnings)]
use std::process::{Command, Stdio};
use std::{
    convert::Infallible,
    fmt::Display,
    fs::File,
    io::{BufReader, Write},
    net::{AddrParseError, IpAddr},
    path::PathBuf,
};

use anyhow::{anyhow, bail, Result};
use clap::{Args, Parser, Subcommand};
use home::home_dir;
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct JumpCmd {
    #[command(subcommand)]
    opt: Opt,
}

#[derive(Debug, Subcommand)]
enum Opt {
    /// Add a server to current store, by default it will be ~/.jump/servers.json
    Add(Server),
    /// Remove a server in current store
    Rm { server_name: String },
    /// List all servers in current store
    Ls,
    /// Configurations of jump itself
    // Config(Config),
    /// Connecting to server
    Connect { server_name: String },
}

#[derive(Debug, Args, Serialize, Deserialize)]
struct Server {
    server_name: String,
    // #[arg(value_parser  = parse_ip)]
    server_address: String,
    #[arg(default_value = "22")]
    port: u32,
    #[command(subcommand)]
    connect_methods: ConnectMethods,
}

#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(about = "connect methods")]
#[command(name = "connect_methods")]
enum ConnectMethods {
    SSHKey(SSHKey),
    Password(Password),
}

impl Display for ConnectMethods {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectMethods::SSHKey(_) => write!(f, "ssh"),
            ConnectMethods::Password(_) => write!(f, "pass"),
        }
    }
}

#[derive(Debug, Args, Serialize, Deserialize)]
struct SSHKey {
    username: String,
    #[arg(value_parser = parse_ssh_path, default_value = "~/.ssh")]
    path: PathBuf,
}

#[derive(Debug, Parser, Serialize, Deserialize)]
struct Password {
    username: String,
    password: String,
}

// #[derive(Debug, Args)]
// struct Config {
//     /// Jump stroage path, by default it will be ~/.jump
//     #[arg(value_parser = parse_ssh_path, default_value = "~/.jump")]
//     path: Option<PathBuf>
// }

fn main() -> Result<()> {
    let args = JumpCmd::parse();
    match args.opt {
        Opt::Add(s) => {
            let f = open_cache_file(false)?;
            let reader = BufReader::new(f);
            let mut vs: Vec<Server> = match serde_json::from_reader(reader) {
                Ok(v) => v,
                Err(e) if e.is_eof() => vec![],
                Err(e) => bail!("Parsing json error: {}", e),
            };
            match vs.iter().find(|x| x.server_name == s.server_name) {
                Some(_) => bail!("Duplicate server name"),
                _ => {}
            }
            vs.push(s);
            let json = serde_json::to_string(&vs)?;
            let mut f = open_cache_file(true)?;
            f.write_all(json.as_bytes())?;
        }
        Opt::Rm { server_name } => {
            let f = open_cache_file(false)?;
            let reader = BufReader::new(f.try_clone()?);
            let vs: Vec<Server> = match serde_json::from_reader(reader) {
                Ok(v) => v,
                Err(e) if e.is_eof() => vec![],
                Err(e) => bail!("Parsing json error: {}", e),
            };
            let remains: Vec<Server> = vs
                .into_iter()
                .filter(|x| x.server_name != server_name)
                .collect();
            let json = serde_json::to_string(&remains)?;
            let mut f = open_cache_file(true)?;
            f.write_all(json.as_bytes())?;
        }
        Opt::Ls => {
            let f = open_cache_file(false)?;
            let reader = BufReader::new(f.try_clone()?);
            let vs: Vec<Server> = match serde_json::from_reader(reader) {
                Ok(v) => v,
                Err(e) if e.is_eof() => vec![],
                Err(e) => bail!("Parsing json error: {}", e),
            };
            vs.iter().for_each(|s| println!("{} ip: {}", s.server_name, s.server_address));
        }
        Opt::Connect { server_name } => {
            let f = open_cache_file(false)?;
            let reader = BufReader::new(f.try_clone()?);
            let vs: Vec<Server> = match serde_json::from_reader(reader) {
                Ok(v) => v,
                Err(e) => bail!("Parsing json error: {}", e),
            };
            let s = match vs.iter().find(|x| x.server_name == server_name) {
                Some(s) => s,
                _ => bail!("Server not found"),
            };
            match &s.connect_methods {
                ConnectMethods::Password(Password { username, password }) => {
                    println!("connecting to server...");
                    Command::new("sshpass")
                        .args(vec![
                            "-p",
                            password,
                            "ssh",
                            "-p",
                            &s.port.to_string(),
                            &format!("{}@{}", username, s.server_address),
                        ])
                        .stdin(Stdio::inherit())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .output()?;
                }
                ConnectMethods::SSHKey(SSHKey { username, path }) => {
                    Command::new("ssh")
                        .args(vec![
                            "-i",
                            path.to_str().ok_or(anyhow!("Invalid ssh key path"))?,
                            "-p",
                            &s.port.to_string(),
                            &format!("{}@{}", username, s.server_address),
                        ])
                        .stdin(Stdio::inherit())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .output()?;
                }
            }
        }
    }

    Ok(())
}

fn parse_ip(str: &str) -> Result<IpAddr, AddrParseError> {
    str.parse()
}

fn parse_ssh_path(str: &str) -> Result<PathBuf, Infallible> {
    str.try_into()
}

fn open_cache_file(truncate: bool) -> Result<File> {
    let jump_path = home_dir()
        .map(|mut p| {
            p.push(".jump");
            p
        })
        .unwrap();
    std::fs::create_dir_all(jump_path.clone())?;
    Ok(File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(truncate)
        .open(format!("{}/servers.json", jump_path.to_str().unwrap()))?)
}
