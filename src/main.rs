#[macro_use]
extern crate diesel;

mod config;
mod proof_rpc;
mod models;

use std::borrow::BorrowMut;
use crate::config::*;
use crate::proof_rpc::*;

use log::*;
use simplelog::*;
use clap::{App, AppSettings, Arg};
use std::sync::Arc;
use jsonrpc_http_server::ServerBuilder;
use jsonrpc_http_server::Server;
use jsonrpc_http_server::jsonrpc_core::IoHandler;
use crate::worker::Worker;
use uuid::{Uuid};

fn main() {
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();

    let app_m = App::new("c2proxy")
        .version("0.0.1")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            App::new("run")
                .setting(AppSettings::ArgRequiredElseHelp)
                .about("run daemon for provide service")
                .args(&[
                    Arg::new("url")
                    .long("url")
                    .env("C2PROXY_URL")
                    .default_value("127.0.0.1:8888")
                    .help("specify url for provide service api service"),
                    Arg::new("db-dsn")
                        .long("db-dsn")
                        .env("C2PROXY_DSN")
                        .default_value("task.db")
                        .help("specify sqlite path to store task")
                ]),
        )
        .get_matches();

    match app_m.subcommand() {
        Some(("run", ref sub_m)) => {
            let url: String = sub_m.value_of_t("url").unwrap_or_else(|e| e.exit());
            let db_dsn: String = sub_m.value_of_t("db-dsn").unwrap_or_else(|e| e.exit());
            let cfg = ServiceConfig::new(url, db_dsn);
            let server = run_cfg(cfg);
            server.wait();
        } // run was used
        _ => {} // Either no subcommand or one not tested for...
    }
}

fn run_cfg(cfg: ServiceConfig) ->Server {
    let db_conn = models::establish_connection(cfg.db_dsn.as_str());
    let task_pool = task_pool::TaskpoolImpl::new(db_conn);

    let mut io = IoHandler::default();
    let arc_pool = Arc::new(task_pool);
    let worker = worker::LocalWorker::new(arc_pool.clone());
    let worker_id = Uuid::new_v4();
    proof::register(io.borrow_mut(), worker_id.to_string(), arc_pool);
    worker.process_tasks();
    let server = ServerBuilder::new(io)
        .start_http(&cfg.url.parse().unwrap())
        .unwrap();

    info!("starting listening {}", cfg.url);
    server
}//run cfg
