mod app;
mod commands;
mod config;
mod db;
mod domain;
mod integrations;
mod services;
mod tasks;
mod telemetry;

fn main() {
    telemetry::init();
    app::run();
}
