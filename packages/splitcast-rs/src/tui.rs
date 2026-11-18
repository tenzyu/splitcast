use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};

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
    output: &mut impl Write,
    label: &str,
) -> io::Result<Option<String>> {
    write!(output, "{label}")?;
    output.flush()?;
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

pub(crate) fn run(
    mut input: impl BufRead,
    mut output: impl Write,
    mut request: impl FnMut(&str, &str) -> io::Result<String>,
) -> io::Result<()> {
    writeln!(output, "SplitCast terminal")?;
    while let Some(choice) = prompt(&mut input, &mut output, MENU)? {
        let (command, args) = match choice.trim() {
            "0" | "q" | "quit" => return Ok(()),
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
            "12" => match prompt(&mut input, &mut output, "Speech: ")? {
                Some(text) => ("say", json!({"text":text})),
                None => return Ok(()),
            },
            "13" => ("start", json!({"output":"twitch-test"})),
            "14" => match prompt(
                &mut input,
                &mut output,
                "Type LIVE to start a public Twitch broadcast: ",
            )? {
                Some(word) if word == "LIVE" => ("start", json!({"output":"twitch-live"})),
                Some(_) => continue,
                None => return Ok(()),
            },
            _ => {
                writeln!(output, "Choose a displayed number.")?;
                continue;
            }
        };
        match request(command, &args.to_string()) {
            Ok(body) => {
                let reply: Value = serde_json::from_str(&body).map_err(io::Error::other)?;
                if command == "preview" {
                    let bytes = reply["image"].as_str().map_or(0, str::len);
                    writeln!(output, "OBS preview received ({bytes} encoded bytes). Open the Web client to view it.")?;
                } else {
                    display(&mut output, &serde_json::to_string_pretty(&reply)?)?;
                }
            }
            Err(error) => display(&mut output, &error.to_string())?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_require_explicit_live_confirmation_and_leave_sessions_on_eof() {
        let mut requests = Vec::new();
        run(
            &b"14\nno\n6\n5\n13\n14\nLIVE\n14\n"[..],
            Vec::new(),
            |command, args| {
                requests.push((
                    command.to_owned(),
                    serde_json::from_str::<Value>(args).unwrap(),
                ));
                Ok("{}".into())
            },
        )
        .unwrap();
        assert_eq!(
            requests,
            [
                ("start".into(), json!({"output":"recording"})),
                ("prepare".into(), json!({"media":false})),
                ("start".into(), json!({"output":"twitch-test"})),
                ("start".into(), json!({"output":"twitch-live"})),
            ]
        );
    }
}
