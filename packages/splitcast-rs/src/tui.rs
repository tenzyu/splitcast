use crate::LocalClient;
use serde_json::json;
use std::io::{self, BufRead, Read, Write};
use std::sync::Mutex;
use std::thread;

const MENU: &str = "
1 Status          2 Discover devices     3 Routes
4 Prepare media   5 Prepare speech       6 Start private recording
7 Stop            8 OBS preview info    9 Speech status
10 Mute mic       11 Unmute mic         12 Speak text
13 Twitch bandwidth test                14 Twitch public broadcast
0 Quit
Choice: ";

fn prompt(
    input: &mut impl BufRead,
    output: &Mutex<(impl Write, u64)>,
    label: &str,
) -> io::Result<Option<String>> {
    {
        let mut output = output
            .lock()
            .map_err(|_| io::Error::other("terminal output lock poisoned"))?;
        write!(output.0, "{label}")?;
        output.0.flush()?;
    }
    let mut line = String::new();
    if input.take(4097).read_line(&mut line)? == 0 {
        return Ok(None);
    }
    if line.len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input exceeds 4096 bytes",
        ));
    }
    Ok(line
        .ends_with('\n')
        .then(|| line.trim_end_matches(['\r', '\n']).to_owned()))
}

fn display(output: &mut impl Write, text: &str) -> io::Result<()> {
    for character in text.chars() {
        if (character.is_control() && character != '\n')
            || matches!(character, '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            write!(output, "{}", character.escape_default())?;
        } else {
            write!(output, "{character}")?;
        }
    }
    writeln!(output)
}

fn show(
    output: &mut impl Write,
    command: &str,
    result: io::Result<serde_json::Value>,
) -> io::Result<()> {
    match result {
        Ok(reply) if command == "preview" => {
            let bytes = reply["image"].as_str().map_or(0, str::len);
            writeln!(
                output,
                "OBS preview received ({bytes} encoded bytes). Open the Web client to view it."
            )
        }
        Ok(reply) => display(output, &serde_json::to_string_pretty(&reply)?),
        Err(error) => display(output, &error.to_string()),
    }
}

fn join_command(command: thread::ScopedJoinHandle<'_, io::Result<()>>) -> io::Result<()> {
    command
        .join()
        .map_err(|_| io::Error::other("terminal command panicked"))?
}

pub(crate) fn run(
    mut input: impl BufRead,
    output: impl Write + Send,
    client: &LocalClient<'_>,
) -> io::Result<()> {
    let console = Mutex::new((output, 0u64));
    let mut output = console
        .lock()
        .map_err(|_| io::Error::other("terminal output lock poisoned"))?;
    writeln!(output.0, "SplitCast terminal")?;
    drop(output);
    thread::scope(|scope| {
        let mut normal: Option<thread::ScopedJoinHandle<'_, io::Result<()>>> = None;
        while let Some(choice) = prompt(&mut input, &console, MENU)? {
            if normal.as_ref().is_some_and(|task| task.is_finished()) {
                join_command(normal.take().unwrap())?;
            }
            let choice = choice.trim();
            if normal.is_some() && choice != "7" {
                let notice = match choice {
                    "0" | "q" | "quit" => {
                        "Quit is unavailable while a command is pending. Stop is available."
                    }
                    _ => "Command pending. Stop is available.",
                };
                writeln!(
                    console
                        .lock()
                        .map_err(|_| io::Error::other("terminal output lock poisoned"))?
                        .0,
                    "{notice}"
                )?;
                continue;
            }
            let (command, args) = match choice {
                "0" | "q" | "quit" => break,
                "1" => ("status", json!({})),
                "2" => ("devices", json!({})),
                "3" => ("routes", json!({})),
                "4" => ("prepare", json!({"media":true})),
                "5" => ("prepare", json!({"media":false})),
                "6" => ("start", json!({"output":"recording"})),
                "7" => ("stop", json!({})),
                "8" => ("preview", json!({})),
                "9" => ("tts", json!({})),
                "10" => ("mic", json!({"action":"mute"})),
                "11" => ("mic", json!({"action":"unmute"})),
                "12" => match prompt(&mut input, &console, "Speech: ")? {
                    Some(text) => ("say", json!({"text":text})),
                    None => break,
                },
                "13" => ("start", json!({"output":"twitch-test"})),
                "14" => {
                    let Some(seconds) = prompt(
                        &mut input,
                        &console,
                        "Maximum public duration in seconds (1–28800): ",
                    )?
                    else {
                        break;
                    };
                    let Ok(seconds @ 1..=28_800) = seconds.parse::<u64>() else {
                        writeln!(
                            console
                                .lock()
                                .map_err(|_| io::Error::other("terminal output lock poisoned"))?
                                .0,
                            "Enter a duration from 1 to 28800 seconds."
                        )?;
                        continue;
                    };
                    match prompt(
                        &mut input,
                        &console,
                        "Type LIVE to start a public Twitch broadcast: ",
                    )? {
                        Some(word) if word == "LIVE" => (
                            "start",
                            json!({"output":"twitch-live","duration_seconds":seconds,"confirm_public":true}),
                        ),
                        Some(_) => continue,
                        None => break,
                    }
                }
                _ => {
                    writeln!(
                        console
                            .lock()
                            .map_err(|_| io::Error::other("terminal output lock poisoned"))?
                            .0,
                        "Choose a displayed number."
                    )?;
                    continue;
                }
            };
            let generation = {
                let mut console = console
                    .lock()
                    .map_err(|_| io::Error::other("terminal output lock poisoned"))?;
                console.1 += 1;
                console.1
            };
            if command == "stop" {
                let result = client.execute(command, args);
                show(
                    &mut console
                        .lock()
                        .map_err(|_| io::Error::other("terminal output lock poisoned"))?
                        .0,
                    command,
                    result,
                )?;
                continue;
            }
            let screen = &console;
            normal = Some(scope.spawn(move || {
                let result = client.execute_if_current(command, args, || {
                    screen.lock().is_ok_and(|screen| screen.1 == generation)
                });
                let mut screen = screen
                    .lock()
                    .map_err(|_| io::Error::other("terminal output lock poisoned"))?;
                if screen.1 == generation {
                    show(&mut screen.0, command, result)?;
                }
                Ok(())
            }));
        }
        if let Some(command) = normal {
            if !command.is_finished() {
                let mut output = console
                    .lock()
                    .map_err(|_| io::Error::other("terminal output lock poisoned"))?;
                writeln!(output.0, "Waiting for the pending command before closing this client. Use Stop from another client if needed.")?;
                output.0.flush()?;
            }
            join_command(command)?;
        }
        let mut output = console
            .lock()
            .map_err(|_| io::Error::other("terminal output lock poisoned"))?;
        writeln!(output.0, "Terminal closed. The Agent keeps its session.")?;
        output.0.flush()
    })
}
