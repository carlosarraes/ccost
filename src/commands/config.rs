use crate::config::Config;
use crate::cli::ConfigAction;

pub fn handle_config_action(action: ConfigAction, json_output: bool) {
    match action {
        ConfigAction::Init => match Config::default().save() {
            Ok(()) => {
                if json_output {
                    println!(
                        r#"{{"status": "success", "message": "Configuration initialized successfully"}}"#
                    );
                } else {
                    if let Ok(config_path) = Config::default_path() {
                        println!("Configuration initialized at: {}", config_path.display());
                    } else {
                        println!("Configuration initialized successfully");
                    }
                }
            }
            Err(e) => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Failed to initialize config: {}"}}"#,
                        e
                    );
                } else {
                    eprintln!("Error: Failed to initialize config: {}", e);
                }
                std::process::exit(1);
            }
        },
        ConfigAction::Show => match Config::load() {
            Ok(config) => {
                if json_output {
                    match serde_json::to_string_pretty(&config) {
                        Ok(json) => println!("{}", json),
                        Err(e) => {
                            eprintln!("Error: Failed to serialize config to JSON: {}", e);
                            std::process::exit(1);
                        }
                    }
                } else {
                    match toml::to_string_pretty(&config) {
                        Ok(toml_str) => {
                            if let Ok(config_path) = Config::default_path() {
                                println!("Configuration ({})", config_path.display());
                                println!("{}", toml_str);
                            } else {
                                println!("Configuration:");
                                println!("{}", toml_str);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: Failed to serialize config: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
            Err(e) => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Failed to load config: {}"}}"#,
                        e
                    );
                } else {
                    eprintln!("Error: Failed to load config: {}", e);
                }
                std::process::exit(1);
            }
        },
        ConfigAction::Set { key, value } => match Config::load() {
            Ok(mut config) => match config.set_value(&key, &value) {
                Ok(()) => match config.save() {
                    Ok(()) => {
                        if json_output {
                            println!(
                                r#"{{"status": "success", "message": "Configuration updated: {} = {}"}}"#,
                                key, value
                            );
                        } else {
                            println!("Configuration updated: {} = {}", key, value);
                        }
                    }
                    Err(e) => {
                        if json_output {
                            println!(
                                r#"{{"status": "error", "message": "Failed to save config: {}"}}"#,
                                e
                            );
                        } else {
                            eprintln!("Error: Failed to save config: {}", e);
                        }
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    if json_output {
                        println!(
                            r#"{{"status": "error", "message": "Invalid configuration: {}"}}"#,
                            e
                        );
                    } else {
                        eprintln!("Error: Invalid configuration: {}", e);
                    }
                    std::process::exit(1);
                }
            },
            Err(e) => {
                if json_output {
                    println!(
                        r#"{{"status": "error", "message": "Failed to load config: {}"}}"#,
                        e
                    );
                } else {
                    eprintln!("Error: Failed to load config: {}", e);
                }
                std::process::exit(1);
            }
        },
    }
}